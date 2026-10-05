//! Pairing: display-confirmed X25519 in the numeric-comparison pattern, on a channel of its own.
//!
//! The joining device announces its public key and hardware address. The adding device answers
//! with its own key and a commitment to a random nonce, the joining device sends its nonce, and
//! the adding device opens its commitment. Both then show a six-digit code of the two keys and
//! nonces. The adding device commits to its nonce before it sees the joining device's. Without
//! that, a device in the middle could try keys until the two codes agreed; with it, each attempt
//! has one chance in a million.
//!
//! Once both users confirm, the adding device sends the group in parts sealed under a key from
//! the X25519 secret, and the joining device acknowledges each. The joining device signs its own
//! record in the group and stores the group before it acknowledges the last part, and that
//! acknowledgement carries the signature. The adding device checks it, stores the new member,
//! then says it is done. The keys exchanged are Ed25519 identities, with X25519 derived from
//! them (`identity`).
//!
//! [`Pairing`] is the exchange without a radio: the caller passes it each frame heard, sends the
//! frames [`Pairing::poll`] returns, and stores the group when the phase is [`Phase::Storing`].
//! Times are microseconds on the caller's timer.

use crate::IDS;
pub use crate::identity::Identity;
use crate::identity::{SIGNATURE_LEN, dh_public};
use crate::members::{Group, MAC_LEN, Member, Name, PUBLIC_LEN, RECORD_MAX_LEN, Slot, stamp};
use crate::seal::{self, Key, SIV_LEN};
use alloc::boxed::Box;

use hkdf::Hkdf;
use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};

/// 2 added the key's generation and gone records to the group's transfer.
pub const VERSION: u8 = 2;
/// The radio's largest payload.
pub const MAX_FRAME: usize = 255;
pub const NONCE_LEN: usize = 16;

/// The joining device announces itself this often.
pub const ANNOUNCE_US: i64 = 2_000_000;
/// A frame that needs an answer is sent again this long after the last, plus up to
/// [`JITTER_US`], so two devices retrying do not keep colliding.
pub const RETRY_US: i64 = 1_000_000;
pub const JITTER_US: u32 = 250_000;
/// How long the adding device searches, and the joining device waits to be chosen.
pub const SEARCH_US: i64 = 120_000_000;
/// How long a user has to confirm the code.
pub const COMPARE_US: i64 = 60_000_000;
/// How long an exchange in progress waits for the next step before contact counts as lost.
pub const STALL_US: i64 = 30_000_000;
/// A device heard announcing stays a candidate this long after it was last heard.
pub const CANDIDATE_US: i64 = 10_000_000;
pub const CANDIDATES: usize = 4;
/// How long the adding device stays after it is done, to answer a repeated last acknowledgement.
pub const LINGER_US: i64 = 5_000_000;
/// How many times a device that ends a pairing tells the other.
const END_REPEATS: u8 = 3;

mod kind {
    pub const ANNOUNCE: u8 = 1;
    pub const OFFER: u8 = 2;
    pub const NONCE: u8 = 3;
    pub const REVEAL: u8 = 4;
    pub const SEALED: u8 = 5;
}

mod body {
    pub const ACCEPT: u8 = 1;
    pub const END: u8 = 2;
    pub const PART: u8 = 3;
    pub const ACK: u8 = 4;
    pub const DONE: u8 = 5;
}

const SESSION_LEN: usize = 8;
/// Version, kind, the joining device's key and hardware address.
const ANNOUNCE_LEN: usize = 2 + 32 + MAC_LEN;
/// Version, kind, the joining and adding devices' keys, the commitment, the adding device's
/// hardware address.
const OFFER_LEN: usize = 2 + 32 + 32 + 32 + MAC_LEN;
/// Version, kind, session and nonce: the joining device's nonce, and the adding device's reveal
/// of its own.
const NONCE_FRAME_LEN: usize = 2 + SESSION_LEN + NONCE_LEN;
/// Version, kind and session.
const SEALED_HEADER: usize = 2 + SESSION_LEN;
const PART_HEADER: usize = 3;
const PART_DATA: usize = MAX_FRAME - SEALED_HEADER - SIV_LEN - PART_HEADER;
/// The key, its generation, the joining device's id, the count, then each slot's record behind
/// its length.
const WELCOME_MAX: usize = 32 + 2 + 2 + IDS as usize * (1 + RECORD_MAX_LEN);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Role {
    Add,
    Join,
}

/// Why a device ended a pairing, as it tells the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Reason {
    /// Its user declined the code.
    Declined,
    /// Its user reported that the codes differ.
    Mismatch,
    /// Its user cancelled.
    Cancelled,
    /// Its user did not confirm the code in time.
    TimedOut,
    /// It could not store the group.
    StoreFailed,
}

impl Reason {
    fn to_byte(self) -> u8 {
        match self {
            Self::Declined => 0,
            Self::Mismatch => 1,
            Self::Cancelled => 2,
            Self::TimedOut => 3,
            Self::StoreFailed => 4,
        }
    }

    fn from_byte(byte: u8) -> Option<Self> {
        Some(match byte {
            0 => Self::Declined,
            1 => Self::Mismatch,
            2 => Self::Cancelled,
            3 => Self::TimedOut,
            4 => Self::StoreFailed,
            _ => return None,
        })
    }
}

/// How a pairing ended without a new member.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum End {
    /// Nothing was chosen, or nothing chose this device, before the search ended.
    NotFound,
    /// The code was not confirmed in time here.
    TimedOut,
    Declined,
    Mismatch,
    Cancelled,
    /// The other device ended it, and said why.
    Peer(Reason),
    /// The other device stopped answering partway.
    Lost,
    /// The other device's nonce did not open its commitment, or its key was unusable.
    Inauthentic,
    /// The transfer completed but did not hold a group with this device in it.
    Malformed,
    /// Adding: the group already has every id taken. Nothing was sent.
    Full,
    StoreFailed,
    /// Adding: the joining device never acknowledged the last part, so whether it stored the
    /// group is unknown. This device did not add it.
    Unconfirmed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Done {
    /// Adding: the member is stored.
    Added { id: u8, returning: bool },
    /// Joining: the group is stored. `confirmed` once the adding device said it had the last
    /// acknowledgement.
    Joined { id: u8, confirmed: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Phase {
    /// Adding: listening for devices to join. Joining: announcing itself, waiting to be chosen.
    Searching,
    /// Adding: devices are announcing, and one must be chosen.
    Found,
    /// Keys and nonces are being exchanged.
    Connecting,
    /// This user compares the codes.
    Compare {
        code: u32,
    },
    /// This user confirmed; the other has not, as far as this device knows.
    Waiting {
        code: u32,
    },
    /// Parts acknowledged, on the adding device, or received and checked, on the joining one.
    Transfer {
        done: u8,
        total: u8,
    },
    /// The caller stores [`Pairing::group`] and calls [`Pairing::stored`].
    Storing,
    /// Joining: stored, and waiting for the adding device to say it heard the last
    /// acknowledgement.
    Finishing,
    Done(Done),
    Ended(End),
}

impl Phase {
    #[must_use]
    pub fn is_final(self) -> bool {
        matches!(self, Self::Done(_) | Self::Ended(_))
    }
}

/// Where a pairing is: its [`Phase`] without the code and the parts' progress, which the
/// pairing holds once for every phase that shows them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Searching,
    Found,
    Connecting,
    Compare,
    Waiting,
    Transfer,
    Storing,
    Finishing,
    Done(Done),
    Ended(End),
}

impl Stage {
    fn is_final(self) -> bool {
        matches!(self, Self::Done(_) | Self::Ended(_))
    }
}

#[derive(Clone, Copy)]
struct Candidate {
    public: [u8; PUBLIC_LEN],
    mac: [u8; MAC_LEN],
    first: i64,
    last: i64,
}

pub struct Pairing {
    role: Role,
    me: Identity,
    public: [u8; PUBLIC_LEN],
    nonce: [u8; NONCE_LEN],
    jitter: u32,
    stage: Stage,
    /// When the phase times out.
    deadline: i64,
    /// When the next frame goes out, and how long after it the one after.
    due: Option<i64>,
    every: Option<i64>,
    ends_left: u8,
    /// What an ended pairing tells the other device.
    tell: Option<Reason>,
    candidates: [Option<Candidate>; CANDIDATES],
    peer: Option<Candidate>,
    their_nonce: [u8; NONCE_LEN],
    commitment: [u8; 32],
    session: [u8; SESSION_LEN],
    key: Option<Key>,
    code: u32,
    accepted_here: bool,
    accepted_there: bool,
    their_name: Option<Name>,
    /// Adding: the group to send, then the group with the new member in it. Joining: the group
    /// received.
    group: Option<Group>,
    new_id: u8,
    returning: bool,
    utc: Option<u32>,
    /// On the heap, for the stack's sake: a welcome for a full group is kilobytes.
    blob: Box<[u8; WELCOME_MAX]>,
    blob_len: usize,
    /// Parts acknowledged, on the adding device; received, on the joining one.
    parts_done: u8,
    parts: u8,
}

impl Pairing {
    fn new(role: Role, me: &Identity, nonce: [u8; NONCE_LEN], now: i64) -> Self {
        let jitter = u32::from_le_bytes(nonce[..4].try_into().expect("four bytes")) | 1;
        Self {
            role,
            me: me.clone(),
            public: me.public(),
            nonce,
            jitter,
            stage: Stage::Searching,
            deadline: now + SEARCH_US,
            due: None,
            every: None,
            ends_left: 0,
            tell: None,
            candidates: [None; CANDIDATES],
            peer: None,
            their_nonce: [0; NONCE_LEN],
            commitment: [0; 32],
            session: [0; SESSION_LEN],
            key: None,
            code: 0,
            accepted_here: false,
            accepted_there: false,
            their_name: None,
            group: None,
            new_id: 0,
            returning: false,
            utc: None,
            blob: Box::new([0; WELCOME_MAX]),
            blob_len: 0,
            parts_done: 0,
            parts: 0,
        }
    }

    /// Starts announcing this device to be added to a group. `nonce` must be random.
    #[must_use]
    pub fn join(me: &Identity, nonce: [u8; NONCE_LEN], now: i64) -> Self {
        let mut pairing = Self::new(Role::Join, me, nonce, now);
        pairing.due = Some(now);
        pairing.every = Some(ANNOUNCE_US);
        pairing
    }

    /// Starts listening for a device to add to `group`, at UTC `utc` if known, which dates the
    /// new member's record. A device in no group passes one [`Group::found`] gave it. `nonce` must be
    /// random. A full group ends at once, with nothing sent.
    #[must_use]
    pub fn add(
        me: &Identity,
        group: Group,
        nonce: [u8; NONCE_LEN],
        now: i64,
        utc: Option<u32>,
    ) -> Self {
        let mut pairing = Self::new(Role::Add, me, nonce, now);
        pairing.utc = utc;
        if group.is_full() {
            pairing.stage = Stage::Ended(End::Full);
            pairing.deadline = now;
        }
        pairing.group = Some(group);
        pairing
    }

    #[must_use]
    pub fn role(&self) -> Role {
        self.role
    }

    #[must_use]
    pub fn phase(&self) -> Phase {
        match self.stage {
            Stage::Searching => Phase::Searching,
            Stage::Found => Phase::Found,
            Stage::Connecting => Phase::Connecting,
            Stage::Compare => Phase::Compare { code: self.code },
            Stage::Waiting => Phase::Waiting { code: self.code },
            Stage::Transfer => Phase::Transfer {
                done: self.parts_done,
                total: self.parts,
            },
            Stage::Storing => Phase::Storing,
            Stage::Finishing => Phase::Finishing,
            Stage::Done(done) => Phase::Done(done),
            Stage::Ended(end) => Phase::Ended(end),
        }
    }

    /// When the phase times out, while it can.
    #[must_use]
    pub fn deadline(&self) -> Option<i64> {
        (!self.stage.is_final() && self.stage != Stage::Storing).then_some(self.deadline)
    }

    /// Adding: the hardware addresses of the devices announcing, in the order first heard.
    pub fn candidates(&self) -> impl Iterator<Item = [u8; MAC_LEN]> + '_ {
        self.candidates.iter().flatten().map(|c| c.mac)
    }

    /// The other device's hardware address, once one is chosen or has chosen this device.
    #[must_use]
    pub fn peer(&self) -> Option<[u8; MAC_LEN]> {
        self.peer.map(|peer| peer.mac)
    }

    /// The other device's name, once it has sent it: the joining device's comes with its
    /// confirmation, the adding device's with the group.
    #[must_use]
    pub fn peer_name(&self) -> Option<Name> {
        match self.role {
            Role::Add => self.their_name,
            Role::Join => {
                let group = self.group.as_ref()?;
                let id = group.by_public(&self.peer?.public)?;
                group.member(id).map(|member| member.name)
            }
        }
    }

    /// The group to store while the phase is [`Phase::Storing`], and the group as it ends.
    #[must_use]
    pub fn group(&self) -> Option<&Group> {
        self.group.as_ref()
    }

    /// Takes the group out of a pairing that is over.
    pub fn take_group(&mut self) -> Option<Group> {
        self.group.take()
    }

    /// Adding: whether the joining device's user has confirmed the code.
    #[must_use]
    pub fn peer_accepted(&self) -> bool {
        self.accepted_there
    }

    /// Whether the pairing has nothing left to send or wait for.
    #[must_use]
    pub fn is_over(&self, now: i64) -> bool {
        self.stage.is_final() && self.due.is_none() && now >= self.deadline
    }

    /// When [`Pairing::poll`] next has something to do.
    #[must_use]
    pub fn wake_at(&self) -> i64 {
        let deadline = if self.stage == Stage::Storing {
            i64::MAX
        } else {
            self.deadline
        };
        self.due.map_or(deadline, |due| due.min(deadline))
    }

    fn jitter(&mut self) -> i64 {
        self.jitter ^= self.jitter << 13;
        self.jitter ^= self.jitter >> 17;
        self.jitter ^= self.jitter << 5;
        i64::from(self.jitter % JITTER_US)
    }

    fn send_now(&mut self, now: i64, every: Option<i64>) {
        self.due = Some(now);
        self.every = every;
    }

    fn end(&mut self, end: End, tell: Option<Reason>, now: i64) {
        self.stage = Stage::Ended(end);
        self.tell = tell.filter(|_| self.key.is_some());
        self.ends_left = 0;
        self.due = None;
        if self.tell.is_some() {
            self.ends_left = END_REPEATS;
            self.send_now(now, Some(RETRY_US));
        }
        self.deadline = now;
    }

    /// Adding: picks the candidate at `index` in [`Pairing::candidates`].
    pub fn choose(&mut self, index: usize, now: i64) {
        if self.role != Role::Add || self.stage != Stage::Found {
            return;
        }
        let Some(candidate) = self.candidates.iter().flatten().nth(index).copied() else {
            return;
        };
        self.peer = Some(candidate);
        self.commitment = commitment(&self.nonce, &self.public, &candidate.public);
        self.session = session(&candidate.public, &self.public);
        self.stage = Stage::Connecting;
        self.deadline = now + STALL_US;
        self.send_now(now, Some(RETRY_US));
    }

    /// This user confirmed that the codes match.
    pub fn accept(&mut self, now: i64) {
        let Stage::Compare = self.stage else {
            return;
        };
        self.accepted_here = true;
        self.stage = Stage::Waiting;
        match self.role {
            Role::Join => self.send_now(now, Some(RETRY_US)),
            Role::Add if self.accepted_there => self.start_transfer(now),
            Role::Add => {}
        }
    }

    /// This user declined the code, or reported that the codes differ.
    pub fn reject(&mut self, mismatch: bool, now: i64) {
        if matches!(self.stage, Stage::Compare | Stage::Waiting) {
            let (end, reason) = if mismatch {
                (End::Mismatch, Reason::Mismatch)
            } else {
                (End::Declined, Reason::Declined)
            };
            self.end(end, Some(reason), now);
        }
    }

    /// This user cancelled. Once the group is being stored, it is too late to.
    pub fn cancel(&mut self, now: i64) {
        if !self.stage.is_final() && !matches!(self.stage, Stage::Storing | Stage::Finishing) {
            self.end(End::Cancelled, Some(Reason::Cancelled), now);
        }
    }

    /// Takes the outcome of storing [`Pairing::group`].
    pub fn stored(&mut self, ok: bool, now: i64) {
        if self.stage != Stage::Storing {
            return;
        }
        match (self.role, ok) {
            (Role::Join, true) => {
                self.stage = Stage::Finishing;
                self.deadline = now + STALL_US;
                self.send_now(now, Some(RETRY_US));
            }
            (Role::Join, false) => self.end(End::StoreFailed, Some(Reason::StoreFailed), now),
            (Role::Add, ok) => {
                self.stage = if ok {
                    Stage::Done(Done::Added {
                        id: self.new_id,
                        returning: self.returning,
                    })
                } else {
                    Stage::Ended(End::StoreFailed)
                };
                // The joining device has the group either way; telling it so lets it stop.
                self.deadline = now + LINGER_US;
                self.send_now(now, None);
            }
        }
    }

    /// Runs the timers and returns the frame to send now, if one is due.
    pub fn poll(&mut self, now: i64, out: &mut [u8; MAX_FRAME]) -> Option<usize> {
        self.time_out(now);
        let due = self.due?;
        if now < due {
            return None;
        }
        let len = self.frame(out);
        self.due = match self.every {
            Some(every) if len.is_some() => Some(now + every + self.jitter()),
            _ => None,
        };
        if matches!(self.stage, Stage::Ended(_)) && self.ends_left > 0 {
            self.ends_left -= 1;
            if self.ends_left == 0 {
                self.due = None;
            }
        }
        len
    }

    fn time_out(&mut self, now: i64) {
        if self.role == Role::Add && matches!(self.stage, Stage::Searching | Stage::Found) {
            for slot in &mut self.candidates {
                if slot.is_some_and(|c| now - c.last > CANDIDATE_US) {
                    *slot = None;
                }
            }
            self.stage = if self.candidates.iter().any(Option::is_some) {
                Stage::Found
            } else {
                Stage::Searching
            };
        }
        if now < self.deadline || self.stage == Stage::Storing {
            return;
        }
        match self.stage {
            Stage::Searching | Stage::Found => self.end(End::NotFound, None, now),
            Stage::Connecting => self.end(End::Lost, None, now),
            Stage::Compare | Stage::Waiting => {
                self.end(End::TimedOut, Some(Reason::TimedOut), now);
            }
            Stage::Transfer if self.role == Role::Add && self.parts_done + 1 == self.parts => {
                self.end(End::Unconfirmed, None, now);
            }
            Stage::Transfer => self.end(End::Lost, None, now),
            Stage::Finishing => {
                self.stage = Stage::Done(Done::Joined {
                    id: self.new_id,
                    confirmed: false,
                });
                self.due = None;
            }
            Stage::Storing | Stage::Done(_) | Stage::Ended(_) => {}
        }
    }

    /// Writes the frame the phase sends.
    fn frame(&mut self, out: &mut [u8; MAX_FRAME]) -> Option<usize> {
        out[0] = VERSION;
        match (self.role, self.stage) {
            (Role::Join, Stage::Searching) => {
                out[1] = kind::ANNOUNCE;
                out[2..34].copy_from_slice(&self.public);
                out[34..40].copy_from_slice(&self.me.mac);
                Some(ANNOUNCE_LEN)
            }
            (Role::Add, Stage::Connecting) => {
                let peer = self.peer?;
                out[1] = kind::OFFER;
                out[2..34].copy_from_slice(&peer.public);
                out[34..66].copy_from_slice(&self.public);
                out[66..98].copy_from_slice(&self.commitment);
                out[98..104].copy_from_slice(&self.me.mac);
                Some(OFFER_LEN)
            }
            (Role::Join, Stage::Connecting) => {
                out[1] = kind::NONCE;
                out[2..10].copy_from_slice(&self.session);
                out[10..26].copy_from_slice(&self.nonce);
                Some(NONCE_FRAME_LEN)
            }
            (Role::Add, Stage::Compare | Stage::Waiting) => {
                out[1] = kind::REVEAL;
                out[2..10].copy_from_slice(&self.session);
                out[10..26].copy_from_slice(&self.nonce);
                Some(NONCE_FRAME_LEN)
            }
            (Role::Join, Stage::Waiting) => {
                let name = self.me.name;
                let name = name.as_bytes();
                let mut body = [0; 1 + 16];
                body[0] = body::ACCEPT;
                body[1..1 + name.len()].copy_from_slice(name);
                self.seal(out, &body[..1 + name.len()])
            }
            (Role::Add, Stage::Transfer) => {
                let index = self.parts_done;
                let start = usize::from(index) * PART_DATA;
                let end = (start + PART_DATA).min(self.blob_len);
                let mut body = [0; PART_HEADER + PART_DATA];
                body[0] = body::PART;
                body[1] = index;
                body[2] = self.parts;
                body[3..3 + end - start].copy_from_slice(&self.blob[start..end]);
                self.seal(out, &body[..3 + end - start])
            }
            (Role::Join, Stage::Transfer | Stage::Finishing) => {
                let mut ack = [0; 2 + SIGNATURE_LEN];
                ack[..2].copy_from_slice(&[body::ACK, self.parts_done.wrapping_sub(1)]);
                // The last part's carries this device's signature of its own record.
                match self
                    .group
                    .as_ref()
                    .filter(|_| self.parts_done == self.parts)
                {
                    Some(group) => {
                        ack[2..].copy_from_slice(&group.me().signature);
                        self.seal(out, &ack)
                    }
                    None => self.seal(out, &ack[..2]),
                }
            }
            (Role::Add, Stage::Done(_) | Stage::Ended(End::StoreFailed)) => {
                self.seal(out, &[body::DONE])
            }
            (_, Stage::Ended(_)) => {
                let reason = self.tell?;
                self.seal(out, &[body::END, reason.to_byte()])
            }
            _ => None,
        }
    }

    fn seal(&self, out: &mut [u8; MAX_FRAME], body: &[u8]) -> Option<usize> {
        let key = self.key.as_ref()?;
        out[1] = kind::SEALED;
        out[2..SEALED_HEADER].copy_from_slice(&self.session);
        let start = SEALED_HEADER + SIV_LEN;
        out[start..start + body.len()].copy_from_slice(body);
        Some(SEALED_HEADER + seal::seal(key, &mut out[SEALED_HEADER..], body.len()))
    }

    /// Takes a frame heard on the pairing channel.
    pub fn receive(&mut self, frame: &mut [u8], now: i64) {
        if frame.len() < 2 || frame[0] != VERSION {
            return;
        }
        match (self.role, frame[1]) {
            (Role::Add, kind::ANNOUNCE) if frame.len() == ANNOUNCE_LEN => {
                self.announced(frame, now)
            }
            (Role::Join, kind::OFFER) if frame.len() == OFFER_LEN => self.offered(frame, now),
            (Role::Add, kind::NONCE) if frame.len() == NONCE_FRAME_LEN => {
                self.nonce_heard(frame, now)
            }
            (Role::Join, kind::REVEAL) if frame.len() == NONCE_FRAME_LEN => {
                self.revealed(frame, now)
            }
            (_, kind::SEALED) => self.sealed(frame, now),
            _ => {}
        }
    }

    fn announced(&mut self, frame: &[u8], now: i64) {
        if !matches!(self.stage, Stage::Searching | Stage::Found) {
            return;
        }
        let public: [u8; PUBLIC_LEN] = frame[2..34].try_into().expect("32 bytes");
        let mac: [u8; MAC_LEN] = frame[34..40].try_into().expect("6 bytes");
        // One with this device's MAC would be given this device's id, and its record.
        if public == self.public || mac == self.me.mac {
            return;
        }
        if let Some(known) = self
            .candidates
            .iter_mut()
            .flatten()
            .find(|c| c.public == public && c.mac == mac)
        {
            known.last = now;
        } else {
            let slot = match self.candidates.iter().position(Option::is_none) {
                Some(free) => free,
                None => (0..CANDIDATES)
                    .min_by_key(|&i| self.candidates[i].map_or(i64::MIN, |c| c.last))
                    .expect("four slots"),
            };
            self.candidates[slot] = Some(Candidate {
                public,
                mac,
                first: now,
                last: now,
            });
            self.candidates
                .sort_unstable_by_key(|c| c.map_or(i64::MAX, |c| c.first));
        }
        self.stage = Stage::Found;
    }

    fn offered(&mut self, frame: &[u8], now: i64) {
        if self.stage != Stage::Searching || frame[2..34] != self.public {
            return;
        }
        let public: [u8; PUBLIC_LEN] = frame[34..66].try_into().expect("32 bytes");
        self.peer = Some(Candidate {
            public,
            mac: frame[98..104].try_into().expect("6 bytes"),
            first: now,
            last: now,
        });
        self.commitment = frame[66..98].try_into().expect("32 bytes");
        self.session = session(&self.public, &public);
        self.stage = Stage::Connecting;
        self.deadline = now + STALL_US;
        self.send_now(now, Some(RETRY_US));
    }

    fn nonce_heard(&mut self, frame: &[u8], now: i64) {
        if frame[2..10] != self.session {
            return;
        }
        match self.stage {
            Stage::Connecting => {
                let peer = self.peer.expect("chosen before connecting");
                self.their_nonce = frame[10..26].try_into().expect("16 bytes");
                if !self.derive(&peer.public, &peer.public, &self.public.clone()) {
                    self.end(End::Inauthentic, None, now);
                    return;
                }
                self.stage = Stage::Compare;
                self.deadline = now + COMPARE_US;
                self.send_now(now, None);
            }
            // The reveal was lost.
            Stage::Compare | Stage::Waiting => self.send_now(now, None),
            _ => {}
        }
    }

    fn revealed(&mut self, frame: &[u8], now: i64) {
        if self.stage != Stage::Connecting || frame[2..10] != self.session {
            return;
        }
        let peer = self.peer.expect("offered before connecting");
        let theirs: [u8; NONCE_LEN] = frame[10..26].try_into().expect("16 bytes");
        if commitment(&theirs, &peer.public, &self.public) != self.commitment {
            self.end(End::Inauthentic, None, now);
            return;
        }
        self.their_nonce = theirs;
        if !self.derive(&peer.public, &self.public.clone(), &peer.public) {
            self.end(End::Inauthentic, None, now);
            return;
        }
        self.stage = Stage::Compare;
        self.deadline = now + COMPARE_US;
        self.due = None;
    }

    /// Works out the code and the session key from the other device's key, the joining and
    /// adding devices' keys, and both nonces. Returns false for a key no honest device has.
    fn derive(&mut self, theirs: &[u8; 32], joining: &[u8; 32], adding: &[u8; 32]) -> bool {
        let Some(theirs) = dh_public(theirs) else {
            return false;
        };
        let shared = self.me.dh().diffie_hellman(&theirs);
        if !shared.was_contributory() {
            return false;
        }
        let (nonce_j, nonce_a) = match self.role {
            Role::Join => (&self.nonce, &self.their_nonce),
            Role::Add => (&self.their_nonce, &self.nonce),
        };
        let transcript: [u8; 32] = Sha256::new()
            .chain_update(b"octowhere pairing transcript")
            .chain_update(joining)
            .chain_update(adding)
            .chain_update(nonce_j)
            .chain_update(nonce_a)
            .finalize()
            .into();
        let code: [u8; 32] = Sha256::new()
            .chain_update(b"octowhere pairing code")
            .chain_update(transcript)
            .finalize()
            .into();
        self.code = u32::from_be_bytes(code[..4].try_into().expect("four bytes")) % 1_000_000;
        let mut key = [0; 32];
        Hkdf::<Sha256>::new(Some(&transcript), shared.as_bytes())
            .expand(b"octowhere pairing key", &mut key)
            .expect("32 bytes is a valid length");
        self.key = Some(Key::new(key));
        true
    }

    fn sealed(&mut self, frame: &mut [u8], now: i64) {
        if frame.len() <= SEALED_HEADER || frame[2..SEALED_HEADER] != self.session {
            return;
        }
        let Some(key) = &self.key else { return };
        let Ok(body) = seal::open(key, &mut frame[SEALED_HEADER..]) else {
            return;
        };
        let mut copy = [0; MAX_FRAME];
        copy[..body.len()].copy_from_slice(body);
        let body = &copy[..body.len()];
        let Some((&what, rest)) = body.split_first() else {
            return;
        };
        match (self.role, what) {
            (_, body::END)
                if !self.stage.is_final()
                    && !matches!(self.stage, Stage::Storing | Stage::Finishing) =>
            {
                if let Some(reason) = rest.first().copied().and_then(Reason::from_byte) {
                    self.end(End::Peer(reason), None, now);
                }
            }
            (Role::Add, body::ACCEPT) => {
                if self.accepted_there {
                    return;
                }
                self.accepted_there = true;
                self.their_name = Name::new(rest);
                if matches!(self.stage, Stage::Waiting) && self.accepted_here {
                    self.start_transfer(now);
                }
            }
            (Role::Add, body::ACK) => self.acknowledged(rest, now),
            (Role::Join, body::PART) => self.part(rest, now),
            (Role::Join, body::DONE) => {
                if matches!(self.stage, Stage::Finishing | Stage::Done(_)) {
                    self.stage = Stage::Done(Done::Joined {
                        id: self.new_id,
                        confirmed: true,
                    });
                    self.due = None;
                    self.deadline = now;
                }
            }
            _ => {}
        }
    }

    fn start_transfer(&mut self, now: i64) {
        let peer = self.peer.expect("chosen before the transfer");
        let group = self.group.as_mut().expect("an adding device has a group");
        let Some(id) = group.id_for(&peer.public) else {
            self.end(End::Full, None, now);
            return;
        };
        self.returning = group.by_public(&peer.public).is_some();
        self.new_id = id;
        // The joining device signs this, and only this, once it has the group.
        group.enrol(
            id,
            Member {
                public: peer.public,
                joined: stamp(self.utc),
                changed: stamp(self.utc),
                mac: peer.mac,
                name: self.their_name.unwrap_or_else(|| Name::from_mac(&peer.mac)),
                signature: [0; SIGNATURE_LEN],
            },
        );
        self.blob_len = welcome(group, id, &mut self.blob);
        self.parts = self.blob_len.div_ceil(PART_DATA) as u8;
        self.parts_done = 0;
        self.stage = Stage::Transfer;
        self.deadline = now + STALL_US;
        self.send_now(now, Some(RETRY_US));
    }

    fn acknowledged(&mut self, rest: &[u8], now: i64) {
        let Some(&index) = rest.first() else { return };
        match self.stage {
            Stage::Transfer if index == self.parts_done => {
                if self.parts_done + 1 == self.parts {
                    let signed = rest
                        .get(1..)
                        .and_then(|signature| signature.try_into().ok())
                        .is_some_and(|signature| self.signed_by_joiner(signature));
                    if !signed {
                        self.end(End::Inauthentic, Some(Reason::StoreFailed), now);
                        return;
                    }
                }
                self.parts_done += 1;
                if self.parts_done == self.parts {
                    self.stage = Stage::Storing;
                    self.due = None;
                } else {
                    self.stage = Stage::Transfer;
                    self.deadline = now + STALL_US;
                    self.send_now(now, Some(RETRY_US));
                }
            }
            // The joining device missed the done.
            Stage::Done(_) | Stage::Ended(End::StoreFailed) if index + 1 == self.parts => {
                self.send_now(now, None);
            }
            _ => {}
        }
    }

    /// Puts the joining device's `signature` on its record, if it signed the record this device
    /// made of it.
    fn signed_by_joiner(&mut self, signature: [u8; SIGNATURE_LEN]) -> bool {
        let id = self.new_id;
        let Some(group) = &mut self.group else {
            return false;
        };
        let Some(mut record) = group.member(id).copied() else {
            return false;
        };
        record.signature = signature;
        if !record.verify(id) {
            return false;
        }
        group.enrol(id, record);
        true
    }

    fn part(&mut self, rest: &[u8], now: i64) {
        let [index, total, data @ ..] = rest else {
            return;
        };
        let (index, total) = (*index, *total);
        match self.stage {
            Stage::Waiting | Stage::Transfer if index == self.parts_done => {
                if total == 0
                    || (index > 0 && total != self.parts)
                    || usize::from(total) * PART_DATA > WELCOME_MAX + PART_DATA
                    || self.blob_len + data.len() > WELCOME_MAX
                    || (index + 1 < total && data.len() != PART_DATA)
                {
                    return;
                }
                self.blob[self.blob_len..self.blob_len + data.len()].copy_from_slice(data);
                self.blob_len += data.len();
                self.parts_done += 1;
                self.parts = total;
                self.deadline = now + STALL_US;
                if self.parts_done < total {
                    self.stage = Stage::Transfer;
                    self.send_now(now, None);
                    return;
                }
                match read_welcome(&self.blob[..self.blob_len]) {
                    Some(mut group)
                        if group.me().public == self.public
                            && group.me().mac == self.me.mac
                            && group.me().name == self.me.name =>
                    {
                        group.sign_own(&self.me);
                        self.new_id = group.own();
                        self.group = Some(group);
                        self.stage = Stage::Storing;
                        self.due = None;
                    }
                    _ => self.end(End::Malformed, Some(Reason::StoreFailed), now),
                }
            }
            // The acknowledgement was lost.
            Stage::Transfer if index < self.parts_done => {
                self.send_now(now, None);
            }
            _ => {}
        }
    }
}

fn session(joining: &[u8; 32], adding: &[u8; 32]) -> [u8; SESSION_LEN] {
    let digest = Sha256::new()
        .chain_update(b"octowhere pairing session")
        .chain_update(joining)
        .chain_update(adding)
        .finalize();
    digest[..SESSION_LEN].try_into().expect("eight bytes")
}

fn commitment(nonce: &[u8; NONCE_LEN], adding: &[u8; 32], joining: &[u8; 32]) -> [u8; 32] {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(nonce).expect("any length is valid");
    mac.update(b"octowhere pairing commitment");
    mac.update(adding);
    mac.update(joining);
    mac.finalize().into_bytes().into()
}

fn welcome(group: &Group, id: u8, out: &mut [u8; WELCOME_MAX]) -> usize {
    out[..32].copy_from_slice(group.key().bytes());
    out[32..34].copy_from_slice(&group.generation().to_le_bytes());
    out[34] = id;
    let mut count = 0;
    let mut len = 36;
    for slot_id in 0..IDS {
        let Some(slot) = group.slot(slot_id) else {
            continue;
        };
        let mut record = [0; RECORD_MAX_LEN];
        let n = slot.encode(slot_id, &mut record);
        out[len + 1..len + 1 + n].copy_from_slice(&record[..n]);
        out[len] = n as u8;
        len += 1 + n;
        count += 1;
    }
    out[35] = count;
    len
}

/// Reads what [`welcome`] wrote.
fn read_welcome(blob: &[u8]) -> Option<Group> {
    let key = Key::new(blob.get(..32)?.try_into().ok()?);
    let generation = u16::from_le_bytes(blob.get(32..34)?.try_into().ok()?);
    let own = *blob.get(34)?;
    let count = *blob.get(35)?;
    let mut slots = [None; IDS as usize];
    let mut rest = blob.get(36..)?;
    for _ in 0..count {
        let (&n, after) = rest.split_first()?;
        let (record, after) = after.split_at_checked(usize::from(n))?;
        let (id, slot) = Slot::decode(record)?;
        slots[usize::from(id)] = Some(slot);
        rest = after;
    }
    if !rest.is_empty() {
        return None;
    }
    Group::restore(key, generation, own, slots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::members::{GONE_LEN, Gone, tests::member};

    /// The welcome as [`welcome`] wrote it before slots had one encoding.
    fn welcome_as_first_written(group: &Group, id: u8) -> alloc::vec::Vec<u8> {
        let mut out = alloc::vec![0; 36];
        out[..32].copy_from_slice(group.key().bytes());
        out[32..34].copy_from_slice(&group.generation().to_le_bytes());
        out[34] = id;
        let mut count = 0;
        for slot_id in 0..IDS {
            let record = match group.slot(slot_id) {
                Some(Slot::Member(member)) => {
                    let mut record = [0; RECORD_MAX_LEN];
                    let n = member.encode(slot_id, &mut record);
                    record[..n].to_vec()
                }
                Some(Slot::Gone(gone)) => {
                    let mut record = [0; GONE_LEN];
                    gone.encode(slot_id, &mut record);
                    record.to_vec()
                }
                None => continue,
            };
            out.push(record.len() as u8);
            out.extend_from_slice(&record);
            count += 1;
        }
        out[35] = count;
        out
    }

    #[test]
    fn a_welcome_keeps_its_bytes() {
        let mut slots = [None; IDS as usize];
        slots[0] = Some(Slot::Member(member(1, 100)));
        slots[2] = Some(Slot::Gone(Gone::unsigned(member(2, 0).public, 200)));
        slots[31] = Some(Slot::Member(member(3, 300)));
        let group = Group::restore(Key::new([6; 32]), 9, 0, slots).unwrap();
        let mut blob = [0; WELCOME_MAX];
        let len = welcome(&group, 31, &mut blob);
        assert_eq!(blob[..len], welcome_as_first_written(&group, 31));
    }

    #[test]
    fn a_transfer_keeps_the_generation_and_gone_records() {
        let mut slots = [None; IDS as usize];
        slots[0] = Some(Slot::Member(member(1, 100)));
        slots[2] = Some(Slot::Gone(Gone::unsigned(member(2, 0).public, 200)));
        slots[31] = Some(Slot::Member(member(3, 300)));
        let group = Group::restore(Key::new([6; 32]), 9, 0, slots).unwrap();
        let mut blob = [0; WELCOME_MAX];
        let len = welcome(&group, 31, &mut blob);
        let theirs = read_welcome(&blob[..len]).unwrap();
        assert_eq!(theirs.own(), 31);
        assert_eq!(theirs.generation(), 9);
        assert!(theirs.key() == group.key());
        assert_eq!(theirs.digest(), group.digest());
        assert!(read_welcome(&blob[..len - 1]).is_none());
    }

    const MS: i64 = 1_000;
    const UTC: u32 = 1_790_000_000;

    fn identity(n: u8) -> Identity {
        let mac = [0x10, 0, 0, 0, 0x1c, n];
        Identity::new([n; 32], mac, Name::from_mac(&mac))
    }

    /// A group founded by `adder`, with members at the other `ids`.
    fn group(adder: &Identity, ids: &[u8]) -> Group {
        let mut group = Group::found(Key::new([9; 32]), adder, Some(UTC - 100));
        for &id in ids {
            group.enrol(id, member(100 + id, UTC - 50));
        }
        group
    }

    /// Who sent a frame.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum From {
        Adder,
        Joiner,
    }

    /// Runs both devices for up to `for_us`, delivering every frame `keep` lets through, after
    /// `act` has acted for the users. Answers the store as `store` says.
    fn run(
        adder: &mut Pairing,
        joiner: &mut Pairing,
        for_us: i64,
        mut keep: impl FnMut(From, Phase, &[u8]) -> bool,
        mut act: impl FnMut(i64, &mut Pairing, &mut Pairing),
        store: (bool, bool),
    ) -> i64 {
        let mut out = [0; MAX_FRAME];
        let mut now = 0;
        while now < for_us {
            act(now, adder, joiner);
            for (from, store) in [(From::Adder, store.0), (From::Joiner, store.1)] {
                let (sender, receiver) = match from {
                    From::Adder => (&mut *adder, &mut *joiner),
                    From::Joiner => (&mut *joiner, &mut *adder),
                };
                if sender.phase() == Phase::Storing {
                    sender.stored(store, now);
                }
                if let Some(len) = sender.poll(now, &mut out)
                    && keep(from, sender.phase(), &out[..len])
                {
                    receiver.receive(&mut out[..len], now);
                }
            }
            if adder.is_over(now) && joiner.is_over(now) {
                return now;
            }
            now += 10 * MS;
        }
        now
    }

    /// Chooses the first device found and confirms each code once it shows.
    fn users(now: i64, adder: &mut Pairing, joiner: &mut Pairing) {
        if adder.phase() == Phase::Found {
            adder.choose(0, now);
        }
        for side in [adder, joiner] {
            if let Phase::Compare { .. } = side.phase() {
                side.accept(now);
            }
        }
    }

    fn code(pairing: &Pairing) -> Option<u32> {
        match pairing.phase() {
            Phase::Compare { code } | Phase::Waiting { code } => Some(code),
            _ => None,
        }
    }

    #[test]
    fn a_device_joins_an_existing_group() {
        let (a, j) = (identity(1), identity(2));
        let mut adder = Pairing::add(&a, group(&a, &[1, 3]), [1; 16], 0, Some(UTC));
        let mut joiner = Pairing::join(&j, [2; 16], 0);
        let mut codes = None;
        run(
            &mut adder,
            &mut joiner,
            60_000 * MS,
            |_, _, _| true,
            |now, adder, joiner| {
                if let (Some(x), Some(y)) = (code(adder), code(joiner)) {
                    codes = Some((x, y));
                }
                users(now, adder, joiner);
            },
            (true, true),
        );
        let (x, y) = codes.expect("both showed a code");
        assert_eq!(x, y);
        assert!(x < 1_000_000);
        assert_eq!(
            adder.phase(),
            Phase::Done(Done::Added {
                id: 2,
                returning: false
            })
        );
        assert_eq!(
            joiner.phase(),
            Phase::Done(Done::Joined {
                id: 2,
                confirmed: true
            })
        );
        assert_eq!(joiner.peer(), Some(a.mac));
        assert_eq!(adder.peer(), Some(j.mac));
        assert_eq!(joiner.peer_name(), Some(a.name));
        assert_eq!(adder.peer_name(), Some(j.name));
        let (theirs, ours) = (adder.group().unwrap(), joiner.group().unwrap());
        assert_eq!(ours.key().bytes(), theirs.key().bytes());
        assert_eq!(ours.own(), 2);
        assert_eq!(ours.count(), 4);
        for (id, member) in theirs.members() {
            assert_eq!(ours.member(id), Some(member));
        }
        let me = ours.me();
        assert_eq!(
            (me.public, me.mac, me.name, me.joined),
            (j.public(), j.mac, j.name, UTC)
        );
    }

    #[test]
    fn a_device_in_no_group_founds_one() {
        let (a, j) = (identity(1), identity(2));
        let founded = Group::found(Key::new([4; 32]), &a, Some(UTC - 100));
        let mut adder = Pairing::add(&a, founded, [1; 16], 0, Some(UTC));
        let mut joiner = Pairing::join(&j, [2; 16], 0);
        run(
            &mut adder,
            &mut joiner,
            60_000 * MS,
            |_, _, _| true,
            users,
            (true, true),
        );
        assert_eq!(
            adder.phase(),
            Phase::Done(Done::Added {
                id: 1,
                returning: false
            })
        );
        assert_eq!(
            joiner.group().unwrap().member(0).unwrap().public,
            a.public()
        );
    }

    #[test]
    fn it_survives_losing_frames() {
        let (a, j) = (identity(1), identity(2));
        let mut adder = Pairing::add(
            &a,
            group(&a, &[1, 2, 3, 4, 5, 6, 7, 8, 9]),
            [1; 16],
            0,
            Some(UTC),
        );
        let mut joiner = Pairing::join(&j, [2; 16], 0);
        let mut n = 0;
        let lose_every_third = |_: From, _: Phase, _: &[u8]| {
            n += 1;
            n % 3 != 0
        };
        run(
            &mut adder,
            &mut joiner,
            120_000 * MS,
            lose_every_third,
            users,
            (true, true),
        );
        assert_eq!(
            adder.phase(),
            Phase::Done(Done::Added {
                id: 10,
                returning: false
            })
        );
        assert!(matches!(
            joiner.phase(),
            Phase::Done(Done::Joined { id: 10, .. })
        ));
    }

    #[test]
    fn a_full_group_is_spread_over_parts() {
        let (a, j) = (identity(1), identity(2));
        let ids: [u8; 30] = core::array::from_fn(|i| i as u8 + 1);
        let mut adder = Pairing::add(&a, group(&a, &ids), [1; 16], 0, Some(UTC));
        let mut joiner = Pairing::join(&j, [2; 16], 0);
        let mut most = 0;
        run(
            &mut adder,
            &mut joiner,
            60_000 * MS,
            |_, _, _| true,
            |now, adder, joiner| {
                if let Phase::Transfer { total, .. } = joiner.phase() {
                    most = most.max(total);
                }
                users(now, adder, joiner);
            },
            (true, true),
        );
        assert!(most > 1);
        assert_eq!(joiner.group().unwrap().count(), 32);
        assert_eq!(
            adder.phase(),
            Phase::Done(Done::Added {
                id: 31,
                returning: false
            })
        );
    }

    #[test]
    fn a_device_in_the_middle_shows_each_side_a_different_code() {
        let (a, j, m) = (identity(1), identity(2), identity(3));
        let mut adder = Pairing::add(&a, group(&a, &[]), [1; 16], 0, Some(UTC));
        let mut joiner = Pairing::join(&j, [2; 16], 0);
        // The device in the middle adds the joining device, and joins the adding one.
        let mut m_add = Pairing::add(&m, group(&m, &[]), [3; 16], 0, Some(UTC));
        let mut m_join = Pairing::join(&m, [4; 16], 0);
        let mut out = [0; MAX_FRAME];
        for step in 0..3_000 {
            let now = step * 10 * MS;
            if adder.phase() == Phase::Found {
                adder.choose(0, now);
            }
            if m_add.phase() == Phase::Found {
                m_add.choose(0, now);
            }
            if let Some(len) = joiner.poll(now, &mut out) {
                m_add.receive(&mut out[..len], now);
            }
            if let Some(len) = m_add.poll(now, &mut out) {
                joiner.receive(&mut out[..len], now);
            }
            if let Some(len) = m_join.poll(now, &mut out) {
                adder.receive(&mut out[..len], now);
            }
            if let Some(len) = adder.poll(now, &mut out) {
                m_join.receive(&mut out[..len], now);
            }
        }
        let (x, y) = (code(&adder).unwrap(), code(&joiner).unwrap());
        assert_ne!(x, y);
    }

    #[test]
    fn a_tampered_reveal_reaching_the_joining_device_is_refused() {
        let (a, j) = (identity(1), identity(2));
        let mut adder = Pairing::add(&a, group(&a, &[]), [1; 16], 0, Some(UTC));
        let mut joiner = Pairing::join(&j, [2; 16], 0);
        let mut out = [0; MAX_FRAME];
        for step in 0..1_000 {
            let now = step * 10 * MS;
            if adder.phase() == Phase::Found {
                adder.choose(0, now);
            }
            if let Some(len) = joiner.poll(now, &mut out) {
                adder.receive(&mut out[..len], now);
            }
            if let Some(len) = adder.poll(now, &mut out) {
                if out[1] == kind::REVEAL {
                    out[20] ^= 1;
                }
                joiner.receive(&mut out[..len], now);
            }
        }
        assert_eq!(joiner.phase(), Phase::Ended(End::Inauthentic));
    }

    #[test]
    fn a_rejection_reaches_the_other_device_with_its_reason() {
        for (mismatch, here, there) in [
            (true, End::Mismatch, End::Peer(Reason::Mismatch)),
            (false, End::Declined, End::Peer(Reason::Declined)),
        ] {
            let (a, j) = (identity(1), identity(2));
            let mut adder = Pairing::add(&a, group(&a, &[]), [1; 16], 0, Some(UTC));
            let mut joiner = Pairing::join(&j, [2; 16], 0);
            run(
                &mut adder,
                &mut joiner,
                30_000 * MS,
                |_, _, _| true,
                |now, adder, joiner| {
                    if adder.phase() == Phase::Found {
                        adder.choose(0, now);
                    }
                    if let Phase::Compare { .. } = adder.phase() {
                        adder.accept(now);
                    }
                    if let Phase::Compare { .. } = joiner.phase() {
                        joiner.reject(mismatch, now);
                    }
                },
                (true, true),
            );
            assert_eq!(joiner.phase(), Phase::Ended(here));
            assert_eq!(adder.phase(), Phase::Ended(there));
        }
    }

    #[test]
    fn nothing_found_ends_the_search() {
        let a = identity(1);
        let mut adder = Pairing::add(&a, group(&a, &[]), [1; 16], 0, Some(UTC));
        let mut out = [0; MAX_FRAME];
        assert_eq!(adder.poll(SEARCH_US - 1, &mut out), None);
        assert_eq!(adder.phase(), Phase::Searching);
        adder.poll(SEARCH_US, &mut out);
        assert_eq!(adder.phase(), Phase::Ended(End::NotFound));
        assert!(adder.is_over(SEARCH_US));
    }

    #[test]
    fn a_code_left_unconfirmed_times_out() {
        let (a, j) = (identity(1), identity(2));
        let mut adder = Pairing::add(&a, group(&a, &[]), [1; 16], 0, Some(UTC));
        let mut joiner = Pairing::join(&j, [2; 16], 0);
        let ended = run(
            &mut adder,
            &mut joiner,
            120_000 * MS,
            |_, _, _| true,
            |now, adder, _| {
                if adder.phase() == Phase::Found {
                    adder.choose(0, now);
                }
            },
            (true, true),
        );
        assert!(ended > COMPARE_US);
        for side in [&adder, &joiner] {
            assert!(matches!(
                side.phase(),
                Phase::Ended(End::TimedOut | End::Peer(Reason::TimedOut))
            ));
        }
    }

    #[test]
    fn a_full_group_sends_nothing() {
        let a = identity(1);
        let ids: [u8; 31] = core::array::from_fn(|i| i as u8 + 1);
        let mut adder = Pairing::add(&a, group(&a, &ids), [1; 16], 0, Some(UTC));
        assert_eq!(adder.phase(), Phase::Ended(End::Full));
        assert_eq!(adder.poll(0, &mut [0; MAX_FRAME]), None);
        assert!(adder.is_over(0));
    }

    #[test]
    fn the_joining_device_signs_its_own_record() {
        let (a, j) = (identity(1), identity(2));
        let mut adder = Pairing::add(&a, group(&a, &[1]), [1; 16], 0, Some(UTC));
        let mut joiner = Pairing::join(&j, [2; 16], 0);
        run(
            &mut adder,
            &mut joiner,
            60_000 * MS,
            |_, _, _| true,
            users,
            (true, true),
        );
        let Phase::Done(Done::Added { id, .. }) = adder.phase() else {
            panic!("added: {:?}", adder.phase());
        };
        let theirs = adder.group().unwrap().member(id).unwrap();
        assert_eq!(theirs.public, j.public());
        assert!(theirs.verify(id));
        assert_eq!(joiner.group().unwrap().member(id), Some(theirs));
    }

    #[test]
    fn a_device_with_a_known_mac_and_a_new_key_is_a_new_member() {
        let (a, j) = (identity(1), identity(2));
        let mut old = group(&a, &[1, 2]);
        old.enrol(
            5,
            Member {
                mac: j.mac,
                ..member(55, UTC - 999)
            },
        );
        let mut adder = Pairing::add(&a, old, [1; 16], 0, Some(UTC));
        let mut joiner = Pairing::join(&j, [2; 16], 0);
        run(
            &mut adder,
            &mut joiner,
            60_000 * MS,
            |_, _, _| true,
            users,
            (true, true),
        );
        assert_eq!(
            adder.phase(),
            Phase::Done(Done::Added {
                id: 3,
                returning: false
            })
        );
    }

    #[test]
    fn a_returning_device_gets_its_old_id() {
        let (a, j) = (identity(1), identity(2));
        let mut old = group(&a, &[1, 2]);
        old.enrol(
            5,
            Member {
                public: j.public(),
                mac: j.mac,
                ..member(55, UTC - 999)
            },
        );
        let mut adder = Pairing::add(&a, old, [1; 16], 0, Some(UTC));
        let mut joiner = Pairing::join(&j, [2; 16], 0);
        run(
            &mut adder,
            &mut joiner,
            60_000 * MS,
            |_, _, _| true,
            users,
            (true, true),
        );
        assert_eq!(
            adder.phase(),
            Phase::Done(Done::Added {
                id: 5,
                returning: true
            })
        );
        let renewed = adder.group().unwrap().member(5).unwrap();
        assert_eq!((renewed.public, renewed.joined), (j.public(), UTC));
    }

    #[test]
    fn a_lost_last_acknowledgement_leaves_each_side_saying_so() {
        let (a, j) = (identity(1), identity(2));
        let mut adder = Pairing::add(&a, group(&a, &[]), [1; 16], 0, Some(UTC));
        let mut joiner = Pairing::join(&j, [2; 16], 0);
        run(
            &mut adder,
            &mut joiner,
            120_000 * MS,
            // Once the joining device has stored the group, nothing it sends arrives.
            |from, phase, _| !(from == From::Joiner && phase == Phase::Finishing),
            users,
            (true, true),
        );
        assert_eq!(adder.phase(), Phase::Ended(End::Unconfirmed));
        assert_eq!(
            joiner.phase(),
            Phase::Done(Done::Joined {
                id: 1,
                confirmed: false
            })
        );
    }

    #[test]
    fn a_store_that_fails_is_reported_to_the_other_device() {
        let (a, j) = (identity(1), identity(2));
        let mut adder = Pairing::add(&a, group(&a, &[]), [1; 16], 0, Some(UTC));
        let mut joiner = Pairing::join(&j, [2; 16], 0);
        run(
            &mut adder,
            &mut joiner,
            60_000 * MS,
            |_, _, _| true,
            users,
            (true, false),
        );
        assert_eq!(joiner.phase(), Phase::Ended(End::StoreFailed));
        assert_eq!(adder.phase(), Phase::Ended(End::Peer(Reason::StoreFailed)));
    }

    #[test]
    fn a_cancel_during_the_transfer_reaches_the_other_device() {
        let (a, j) = (identity(1), identity(2));
        let ids: [u8; 20] = core::array::from_fn(|i| i as u8 + 1);
        let mut adder = Pairing::add(&a, group(&a, &ids), [1; 16], 0, Some(UTC));
        let mut joiner = Pairing::join(&j, [2; 16], 0);
        run(
            &mut adder,
            &mut joiner,
            60_000 * MS,
            |_, _, _| true,
            |now, adder, joiner| {
                if let Phase::Transfer { done: 2, .. } = joiner.phase() {
                    joiner.cancel(now);
                }
                users(now, adder, joiner);
            },
            (true, true),
        );
        assert_eq!(joiner.phase(), Phase::Ended(End::Cancelled));
        assert_eq!(adder.phase(), Phase::Ended(End::Peer(Reason::Cancelled)));
    }

    #[test]
    fn a_device_announcing_the_adders_own_mac_is_not_listed() {
        let a = identity(1);
        let mut adder = Pairing::add(&a, group(&a, &[]), [1; 16], 0, Some(UTC));
        let mut out = [0; MAX_FRAME];
        let mut twin = identity(2);
        twin.mac = a.mac;
        let mut joiner = Pairing::join(&twin, [2; 16], 0);
        let len = joiner.poll(0, &mut out).unwrap();
        adder.receive(&mut out[..len], 0);
        assert_eq!(adder.candidates().count(), 0);
        assert_eq!(adder.phase(), Phase::Searching);
    }

    #[test]
    fn the_adding_device_lists_every_device_announcing() {
        let a = identity(1);
        let mut adder = Pairing::add(&a, group(&a, &[]), [1; 16], 0, Some(UTC));
        let mut out = [0; MAX_FRAME];
        for (n, at) in [(2, 0), (3, 500 * MS)] {
            let mut joiner = Pairing::join(&identity(n), [n; 16], at);
            let len = joiner.poll(at, &mut out).unwrap();
            adder.receive(&mut out[..len], at);
        }
        assert_eq!(adder.phase(), Phase::Found);
        let macs: [[u8; 6]; 2] = {
            let mut found = adder.candidates();
            [found.next().unwrap(), found.next().unwrap()]
        };
        assert_eq!(macs, [identity(2).mac, identity(3).mac]);
        adder.poll(CANDIDATE_US + 1, &mut out);
        assert_eq!(
            adder.candidates().count(),
            1,
            "the first stopped announcing"
        );
        adder.poll(CANDIDATE_US + 500 * MS + 1, &mut out);
        assert_eq!(adder.phase(), Phase::Searching);
    }
}
