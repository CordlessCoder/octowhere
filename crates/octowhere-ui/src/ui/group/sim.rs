//! A mesh for the host: it answers the screens' requests as the firmware's mesh would, with a
//! scripted device on the other side of each pairing, so tests, renders and the simulators can
//! walk every group screen without a radio. Its names, addresses, codes and ages are synthetic.

use heapless::Vec;

use super::view::{
    Answer, At, Done, End, GroupView, IDS, Mac, MemberView, MeshView, Name, PairingView, Phase,
    Position, Reason, RecoveryPhase, RecoveryView, RefreshPhase, RefreshView, Request, Role,
};
use crate::ui::gesture::Micros;

const SECOND: Micros = 1_000_000;
const SEARCH: Micros = 120 * SECOND;
const COMPARE: Micros = 60 * SECOND;
const STALL: Micros = 30 * SECOND;
/// A refresh's three rounds.
pub const REFRESH: Micros = 135 * SECOND;
/// How long a founding listens for the joining device.
pub const RECOVERY: Micros = 600 * SECOND;

/// What the user of the other device does with the code.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PeerUser {
    #[default]
    Accepts,
    Declines,
    ReportsMismatch,
    /// Never answers, so the code times out.
    Silent,
}

/// What the scripted other device does next, and when.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Next {
    /// Adding: it starts announcing. Joining: it picks this device.
    Appears,
    /// The keys are exchanged and the code is ready.
    Code,
    /// Its user answers the code.
    Answers,
    /// A part arrives, or is acknowledged.
    Part,
    /// The group is stored here.
    Stored,
    /// Joining: the adding device's last word arrives.
    Finished,
    /// A refresh hears a member's packet.
    RefreshHears(u8),
    /// A refresh learns a member added elsewhere.
    RefreshLearns,
    RefreshEnds,
    /// The founded group's joining device is heard.
    JoinerHeard,
    /// The founded group is stored, or its write fails.
    RecoveryStored,
    RecoveryEnds,
}

pub struct Sim {
    view: MeshView,
    pub peer: PeerUser,
    /// Every store fails: leaving, renaming and a pairing's commit.
    pub store_fails: bool,
    /// The other device never confirms that it received the last part.
    pub final_reply_lost: bool,
    /// How long after a founding's wait starts its joining device is heard, if ever.
    pub joiner_heard_after: Option<Micros>,
    /// A refresh learns of a member added elsewhere, as well as hearing two it knows.
    pub refresh_learns: bool,
    /// When a founding's wait ends.
    recovery_until: Option<Micros>,
    next: Option<(Micros, Next)>,
    changed: bool,
}

pub const PEER_MAC: Mac = [0x48, 0xa1, 0xb2, 0xc3, 0x8c, 0x91];
pub const OWN_MAC: Mac = [0x48, 0xa1, 0xb2, 0xc3, 0x7a, 0x2f];
const CODE: u32 = 482_731;
const PARTS: u8 = 2;

fn name(text: &str) -> Name {
    Name::new(text.as_bytes()).expect("a fixture name is printable")
}

/// A group of `count` members with this device at id 0, heard and placed at varied times
/// before `now`.
#[must_use]
pub fn group(count: u8, now: Micros) -> GroupView {
    const NAMES: [&str; 8] = [
        "NORTH-1",
        "Ridge_Walker07!?",
        "RIVER",
        "ABCDEFGHIJKLMNOP",
        "camp stove",
        "OW-3F02",
        "Base",
        "Ana's Watch 2",
    ];
    let now = now as At;
    let seconds = |s: i64| now - s * 1_000_000;
    let mut members = [None; IDS as usize];
    for id in 0..count.min(IDS) {
        let mac = [0x48, 0xa1, 0xb2, 0xc3, 0x11, id];
        let member = if id == 0 {
            MemberView {
                name: Name::from_mac(&OWN_MAC),
                mac: OWN_MAC,
                joined: Some(seconds(86_400 * 3)),
                heard: None,
                position: Position::At(seconds(12)),
            }
        } else {
            let pattern = usize::from(id - 1) % NAMES.len();
            MemberView {
                name: name(NAMES[pattern]),
                mac,
                joined: Some(seconds(86_400 + 3600 * i64::from(id))),
                heard: match pattern {
                    0 => Some(seconds(8)),
                    1 => Some(seconds(2_400)),
                    2 => None,
                    3 => Some(seconds(320)),
                    _ => Some(seconds(30 * i64::from(id))),
                },
                position: match pattern {
                    0 => Position::At(seconds(2_400)),
                    1 => Position::At(seconds(120)),
                    2 => Position::Never,
                    3 => Position::Unknown,
                    _ => Position::At(seconds(60 * i64::from(id))),
                },
            }
        };
        members[usize::from(id)] = Some(member);
    }
    GroupView { own: 0, members }
}

impl Sim {
    /// A device with a radio, its own name and `group`.
    #[must_use]
    pub fn new(group: Option<GroupView>) -> Self {
        Self {
            view: MeshView {
                radio: true,
                mac: OWN_MAC,
                name: Name::from_mac(&OWN_MAC),
                group,
                ..MeshView::default()
            },
            peer: PeerUser::default(),
            store_fails: false,
            final_reply_lost: false,
            joiner_heard_after: Some(215 * SECOND),
            refresh_learns: false,
            recovery_until: None,
            next: None,
            changed: true,
        }
    }

    #[must_use]
    pub fn view(&self) -> &MeshView {
        &self.view
    }

    /// Stops the other device where it is: nothing it had scheduled happens.
    pub fn hold(&mut self) {
        self.next = None;
    }

    /// Changes the view directly, for a fixture the script does not reach.
    pub fn view_mut(&mut self) -> &mut MeshView {
        self.changed = true;
        &mut self.view
    }

    fn pairing(&mut self) -> Option<&mut PairingView> {
        self.view.pairing.as_mut()
    }

    fn set(&mut self, phase: Phase, deadline: Option<Micros>) {
        if let Some(pairing) = self.pairing() {
            pairing.phase = phase;
            pairing.deadline = deadline.map(|deadline| deadline as At);
        }
        self.changed = true;
    }

    fn end(&mut self, end: End) {
        self.set(Phase::Ended(end), None);
        self.next = None;
    }

    fn role(&self) -> Option<Role> {
        self.view.pairing.as_ref().map(|pairing| pairing.role)
    }

    fn phase(&self) -> Option<Phase> {
        self.view.pairing.as_ref().map(|pairing| pairing.phase)
    }

    /// Takes a request from the screens, as the firmware's mesh takes a command.
    pub fn request(&mut self, request: Request, now: Micros) {
        self.changed = true;
        let active = self.phase().is_some_and(|phase| !phase.is_final());
        match request {
            Request::Add | Request::Join if !active => {
                let role = if request == Request::Add {
                    Role::Add
                } else {
                    Role::Join
                };
                self.view.recovery = None;
                if let Some(refresh) = &mut self.view.refresh
                    && refresh.is_listening()
                {
                    refresh.phase = RefreshPhase::Interrupted { at: now as At };
                }
                self.view.sessions += 1;
                let group = self
                    .view
                    .group
                    .as_ref()
                    .map(|group| (group.own, group.count() as u8));
                self.view.pairing = Some(PairingView {
                    session: self.view.sessions,
                    role,
                    phase: Phase::Searching,
                    deadline: Some((now + SEARCH) as At),
                    candidates: Vec::new(),
                    peer: None,
                    peer_name: None,
                    group,
                    refused: None,
                });
                if role == Role::Add && self.view.group.as_ref().is_some_and(GroupView::is_full) {
                    self.end(End::Full);
                } else {
                    self.next = Some((now + 2 * SECOND, Next::Appears));
                }
            }
            Request::Choose(mac) if self.phase() == Some(Phase::Found) => {
                if let Some(pairing) = self.pairing() {
                    pairing.peer = Some(mac);
                }
                self.set(Phase::Connecting, Some(now + STALL));
                self.next = Some((now + SECOND / 2, Next::Code));
            }
            Request::Accept => {
                if let Some(Phase::Compare { code }) = self.phase() {
                    self.set(Phase::Waiting { code }, Some(now + COMPARE));
                }
            }
            Request::Decline | Request::Mismatch | Request::Cancel if active => {
                let end = match request {
                    Request::Decline => End::Declined,
                    Request::Mismatch => End::Mismatch,
                    _ => End::Cancelled,
                };
                if !matches!(self.phase(), Some(Phase::Storing | Phase::Finishing)) {
                    self.end(end);
                }
            }
            Request::Leave => {
                let ok = !self.store_fails;
                if ok {
                    self.view.group = None;
                    self.view.refresh = None;
                    self.view.recovery = None;
                    self.next = None;
                }
                self.answer(Answer::Left(ok));
            }
            Request::Refresh
                if self.view.radio
                    && self.view.group.is_some()
                    && !active
                    && !self
                        .view
                        .refresh
                        .is_some_and(|refresh| refresh.is_listening()) =>
            {
                let session = self.view.refresh.map_or(1, |refresh| refresh.session + 1);
                self.view.refresh = Some(RefreshView {
                    session,
                    phase: RefreshPhase::Listening {
                        until: (now + REFRESH) as At,
                    },
                    heard: 0,
                    learned: 0,
                });
                self.next = Some((now + 20 * SECOND, Next::RefreshHears(1)));
            }
            Request::Rename(name) => {
                let ok = !self.store_fails;
                if ok {
                    self.view.name = name;
                    if let Some(group) = &mut self.view.group {
                        let own = usize::from(group.own);
                        if let Some(member) = &mut group.members[own] {
                            member.name = name;
                        }
                    }
                }
                self.answer(Answer::Renamed(ok));
            }
            _ => {}
        }
    }

    fn answer(&mut self, answer: Answer) {
        self.view.answered += 1;
        self.view.answer = Some(answer);
    }

    /// Moves the other device on to `now`. Returns whether the view changed since the last
    /// call.
    pub fn step(&mut self, now: Micros) -> bool {
        if let Some(deadline) = self
            .view
            .pairing
            .as_ref()
            .filter(|pairing| !pairing.phase.is_final())
            .and_then(|pairing| pairing.deadline)
            && now as At >= deadline
        {
            if self.phase() == Some(Phase::Finishing) {
                // Stored here, and the other device's last word never came.
                let id = self.view.group.as_ref().map_or(0, |group| group.own);
                self.set(
                    Phase::Done(Done::Joined {
                        id,
                        confirmed: false,
                    }),
                    None,
                );
                return core::mem::take(&mut self.changed);
            }
            let end = match self.phase() {
                Some(Phase::Searching | Phase::Found) => End::NotFound,
                Some(Phase::Compare { .. } | Phase::Waiting { .. }) => End::TimedOut,
                Some(Phase::Transfer { done, total })
                    if self.role() == Some(Role::Add) && done + 1 == total =>
                {
                    End::Unconfirmed
                }
                _ => End::Lost,
            };
            self.end(end);
            if end == End::Unconfirmed && self.view.group.is_none() {
                self.wait_for_joiner(now);
            }
        }
        while let Some((at, next)) = self.next
            && now >= at
        {
            self.next = None;
            self.changed = true;
            self.advance(next, at);
        }
        core::mem::take(&mut self.changed)
    }

    fn advance(&mut self, next: Next, now: Micros) {
        let role = self.role();
        match (next, role) {
            (Next::Appears, Some(Role::Add)) => {
                if let Some(pairing) = self.pairing() {
                    _ = pairing.candidates.push(PEER_MAC);
                }
                let deadline = self.view.pairing.as_ref().and_then(|p| p.deadline);
                self.set(Phase::Found, deadline.map(|deadline| deadline as Micros));
            }
            (Next::Appears, Some(Role::Join)) => {
                if let Some(pairing) = self.pairing() {
                    pairing.peer = Some(PEER_MAC);
                }
                self.set(Phase::Connecting, Some(now + STALL));
                self.next = Some((now + SECOND / 2, Next::Code));
            }
            (Next::Code, _) => {
                self.set(Phase::Compare { code: CODE }, Some(now + COMPARE));
                if self.peer != PeerUser::Silent {
                    self.next = Some((now + 3 * SECOND, Next::Answers));
                }
            }
            (Next::Answers, Some(role)) => match self.peer {
                PeerUser::Declines => self.end(End::Peer(Reason::Declined)),
                PeerUser::ReportsMismatch => self.end(End::Peer(Reason::Mismatch)),
                PeerUser::Silent => {}
                PeerUser::Accepts => {
                    if role == Role::Add
                        && let Some(pairing) = self.pairing()
                    {
                        pairing.peer_name = Some(name("Ana's Watch 2"));
                    }
                    match self.phase() {
                        // The other user first: the transfer waits for this one.
                        Some(Phase::Compare { .. }) => {
                            self.next = Some((now + SECOND / 2, Next::Answers))
                        }
                        _ => {
                            self.set(
                                Phase::Transfer {
                                    done: 0,
                                    total: PARTS,
                                },
                                Some(now + STALL),
                            );
                            self.next = Some((now + SECOND / 2, Next::Part));
                        }
                    }
                }
            },
            (Next::Part, Some(role)) => {
                let Some(Phase::Transfer { done, total }) = self.phase() else {
                    return;
                };
                let last = done + 1 == total;
                if last && role == Role::Add && self.final_reply_lost {
                    // The last acknowledgement never comes; the deadline ends it.
                    return;
                }
                if last {
                    self.set(Phase::Storing, None);
                    self.next = Some((now + SECOND / 10, Next::Stored));
                } else {
                    self.set(
                        Phase::Transfer {
                            done: done + 1,
                            total,
                        },
                        Some(now + STALL),
                    );
                    self.next = Some((now + SECOND / 2, Next::Part));
                }
            }
            (Next::Stored, Some(role)) => {
                if self.store_fails {
                    return self.end(End::StoreFailed);
                }
                match role {
                    Role::Add => {
                        let group = self.view.group.get_or_insert_with(|| group(1, now));
                        let id = (0..IDS)
                            .find(|&id| group.members[usize::from(id)].is_none())
                            .unwrap_or(0);
                        group.members[usize::from(id)] = Some(MemberView {
                            name: name("Ana's Watch 2"),
                            mac: PEER_MAC,
                            joined: Some(now as At),
                            heard: None,
                            position: Position::Never,
                        });
                        let count = group.count() as u8;
                        if let Some(pairing) = self.pairing() {
                            pairing.group = Some((0, count));
                        }
                        self.set(
                            Phase::Done(Done::Added {
                                id,
                                returning: false,
                            }),
                            None,
                        );
                    }
                    Role::Join => {
                        let mut joined = group(3, now);
                        joined.own = 2;
                        let me = MemberView {
                            name: self.view.name,
                            mac: self.view.mac,
                            joined: Some(now as At),
                            heard: None,
                            position: Position::Never,
                        };
                        joined.members[2] = Some(me);
                        if let Some(pairing) = self.pairing() {
                            pairing.group = Some((2, joined.count() as u8));
                        }
                        self.view.group = Some(joined);
                        self.set(Phase::Finishing, Some(now + STALL));
                        if !self.final_reply_lost {
                            self.next = Some((now + SECOND / 5, Next::Finished));
                        }
                    }
                }
            }
            (Next::Finished, _) => self.set(
                Phase::Done(Done::Joined {
                    id: 2,
                    confirmed: true,
                }),
                None,
            ),
            (Next::RefreshHears(id), _) => {
                if let Some(member) = self
                    .view
                    .group
                    .as_mut()
                    .and_then(|group| group.members[usize::from(id)].as_mut())
                {
                    member.heard = Some(now as At);
                    if let Some(refresh) = &mut self.view.refresh {
                        refresh.heard |= 1 << id;
                    }
                }
                self.next = Some(match id {
                    1 => (now + 30 * SECOND, Next::RefreshHears(2)),
                    _ if self.refresh_learns => (now + 20 * SECOND, Next::RefreshLearns),
                    _ => (self.refresh_ends(), Next::RefreshEnds),
                });
            }
            (Next::RefreshLearns, _) => {
                if let Some(group) = &mut self.view.group
                    && let Some(id) = (0..IDS).find(|&id| group.members[usize::from(id)].is_none())
                {
                    group.members[usize::from(id)] = Some(MemberView {
                        name: name("Fell Runner"),
                        mac: [0x48, 0xa1, 0xb2, 0xc3, 0x22, id],
                        joined: Some(now as At - 3_600 * SECOND as At),
                        heard: None,
                        position: Position::Never,
                    });
                    if let Some(refresh) = &mut self.view.refresh {
                        refresh.learned |= 1 << id;
                    }
                }
                self.next = Some((self.refresh_ends(), Next::RefreshEnds));
            }
            (Next::RefreshEnds, _) => {
                if let Some(refresh) = &mut self.view.refresh
                    && let RefreshPhase::Listening { until } = refresh.phase
                {
                    refresh.phase = RefreshPhase::Ended { at: until };
                }
            }
            (Next::JoinerHeard, _) => {
                self.set_recovery(RecoveryPhase::Storing);
                self.next = Some((now + SECOND / 20, Next::RecoveryStored));
            }
            (Next::RecoveryStored, _) => {
                let until = self.recovery_ends();
                if self.store_fails {
                    self.set_recovery(RecoveryPhase::SaveFailed { until: until as At });
                    self.next = Some((until, Next::RecoveryEnds));
                    return;
                }
                let mut group = group(1, now);
                group.members[1] = Some(MemberView {
                    name: name("Ana's Watch 2"),
                    mac: PEER_MAC,
                    joined: Some(now as At),
                    heard: Some(now as At),
                    position: Position::Never,
                });
                self.view.group = Some(group);
                self.set_recovery(RecoveryPhase::Stored);
            }
            (Next::RecoveryEnds, _) => {
                let phase = match self.view.recovery.map(|recovery| recovery.phase) {
                    Some(RecoveryPhase::Listening { .. }) => RecoveryPhase::Expired,
                    _ => RecoveryPhase::NotStored,
                };
                self.set_recovery(phase);
            }
            (_, None) => {}
        }
    }

    fn refresh_ends(&self) -> Micros {
        match self.view.refresh.map(|refresh| refresh.phase) {
            Some(RefreshPhase::Listening { until }) => until as Micros,
            _ => 0,
        }
    }

    fn recovery_ends(&self) -> Micros {
        self.recovery_until.unwrap_or(0)
    }

    /// Listens for the joining device of a founding whose last acknowledgement never came.
    fn wait_for_joiner(&mut self, now: Micros) {
        let until = now + RECOVERY;
        self.recovery_until = Some(until);
        self.view.recovery = Some(RecoveryView {
            session: self.view.sessions,
            phase: RecoveryPhase::Listening { until: until as At },
            peer_name: self
                .view
                .pairing
                .as_ref()
                .and_then(|pairing| pairing.peer_name),
            peer: PEER_MAC,
            count: 2,
        });
        self.next = Some(match self.joiner_heard_after {
            Some(after) if after < RECOVERY => (now + after, Next::JoinerHeard),
            _ => (until, Next::RecoveryEnds),
        });
    }

    fn set_recovery(&mut self, phase: RecoveryPhase) {
        if let Some(recovery) = &mut self.view.recovery {
            recovery.phase = phase;
        }
        self.changed = true;
    }
}
