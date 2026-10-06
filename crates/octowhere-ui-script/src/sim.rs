//! A mesh for the host: it answers the screens' requests as the firmware's mesh would, with a
//! scripted device on the other side of each pairing, so tests, renders and the simulators can
//! walk every group screen without a radio. Its names, addresses, codes and ages are synthetic.

use heapless::Vec;
use octowhere_ui::ui::{
    gesture::Micros,
    group::view::{
        Answer, At, Carriage, Decline, Done, End, GroupView, IDS, Mac, MemberView, MeshView,
        MessageView, MessagesView, Name, PairingView, Phase, Position, Reason, RecoveryPhase,
        RecoveryView, Refusal, Refused, RemovalStage, RemovalView, Request, Role, Thread,
        Unremovable,
    },
};

const SECOND: Micros = 1_000_000;
const SEARCH: Micros = 120 * SECOND;
const COMPARE: Micros = 60 * SECOND;
const STALL: Micros = 30 * SECOND;
/// How long a founding listens for the joining device.
const RECOVERY: Micros = 600 * SECOND;

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
    /// When a founding's wait ends.
    recovery_until: Option<Micros>,
    next: Option<(Micros, Next)>,
    changed: bool,
    messages: alloc::boxed::Box<MessagesView>,
    /// The number the next message shown takes.
    next_message: u32,
    /// What happens to a message next, and when: it goes further, or its destination replies.
    carried: Vec<(Micros, u32, Carried), 16>,
    messages_changed: bool,
    /// The member the last switch removed, to put back if it is declined.
    removed: Option<(u8, MemberView)>,
}

/// A message as [`Sim::arrive`] adds it: from `from` to `to`, sent `ago` before now.
#[derive(Clone, Copy, Debug)]
pub struct Arrival<'t> {
    pub from: u8,
    pub to: Option<u8>,
    pub text: &'t str,
    pub ago: Micros,
    pub carriage: Carriage,
    pub unread: bool,
}

/// What happens to a message sent from here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Carried {
    Goes(Carriage),
    /// The member it went to privately answers.
    Answered,
}

/// How long after it is queued a message goes out, is heard relayed, and for a private one is
/// acknowledged, and its destination answers.
const SENT_AFTER: Micros = 2 * SECOND;
const RELAYED_AFTER: Micros = 6 * SECOND;
const DELIVERED_AFTER: Micros = 10 * SECOND;
const ANSWERED_AFTER: Micros = 20 * SECOND;
/// How long after a removal starts its switch comes, as for a group of eight, and how long after
/// the switch it can be declined.
pub const SWITCH_AFTER: Micros = 8 * 60 * SECOND;
const DECLINE_FOR: Micros = 86_400 * SECOND;

const PEER_MAC: Mac = [0x48, 0xa1, 0xb2, 0xc3, 0x8c, 0x91];
const OWN_MAC: Mac = [0x48, 0xa1, 0xb2, 0xc3, 0x7a, 0x2f];
const CODE: u32 = 482_731;
const PARTS: u8 = 2;

/// A synthetic fingerprint for the device with address `mac`.
fn device(mac: &Mac) -> [u8; 8] {
    let mut device = [0x5a; 8];
    for (i, byte) in mac.iter().enumerate() {
        device[i] ^= byte.rotate_left(i as u32 + 1);
        device[7 - i] = device[7 - i].wrapping_mul(31).wrapping_add(*byte);
    }
    device
}

fn name(text: &str) -> Name {
    Name::new(text.as_bytes()).expect("a fixture name is printable")
}

/// Dublin, where this device stands, in degrees × 10⁷.
const DUBLIN: (i32, i32) = (533_498_000, -62_603_000);

/// Where member `id` stands: spread round Dublin at bearings a golden angle apart, further out
/// each, so that their bearings and distances differ.
fn around(id: u8) -> (i32, i32) {
    let metres = 180.0 * f32::from(id);
    let (sin, cos) = libm::sincosf((137.5 * f32::from(id)).to_radians());
    let north = metres * cos / 111_320.0;
    let east = metres * sin / (111_320.0 * libm::cosf(53.35_f32.to_radians()));
    (
        DUBLIN.0 + (north * 1e7) as i32,
        DUBLIN.1 + (east * 1e7) as i32,
    )
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
                device: device(&OWN_MAC),
                joined: Some(seconds(86_400 * 3)),
                heard: None,
                position: Position::At(seconds(12)),
                coordinates: Some(DUBLIN),
            }
        } else {
            let pattern = usize::from(id - 1) % NAMES.len();
            MemberView {
                name: name(NAMES[pattern]),
                mac,
                device: device(&mac),
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
                coordinates: (pattern != 2).then(|| around(id)),
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
            recovery_until: None,
            next: None,
            changed: true,
            messages: MessagesView::boxed(),
            next_message: 1,
            carried: Vec::new(),
            messages_changed: true,
            removed: None,
        }
    }

    /// The removal of member `removed` that member `remover` asks for at `now`, with its switch
    /// `switch_in` later, as the mesh shows it.
    fn removal(&self, remover: u8, removed: u8, switch_in: Micros, now: Micros) -> RemovalView {
        let member = |id: u8| {
            self.view
                .group
                .as_ref()
                .and_then(|group| group.member(id))
                .copied()
        };
        let name =
            |id: u8| member(id).map_or_else(|| Name::from_mac(&[0, 0, 0, 0, 0, id]), |m| m.name);
        let mut key = [0x4b; 8];
        key[0] = remover;
        key[1] = removed;
        key[2..].copy_from_slice(&now.to_be_bytes()[2..]);
        RemovalView {
            key,
            remover,
            remover_name: name(remover),
            removed,
            removed_name: name(removed),
            device: member(removed).map_or([0; 8], |member| member.device),
            stage: RemovalStage::Pending {
                since: now as At,
                switch: Some((now + switch_in) as At),
            },
        }
    }

    /// Member `remover` asks to remove member `removed`, its switch `switch_in` from `now`.
    /// Another request under way becomes its rival and loses.
    pub fn request_removal(&mut self, remover: u8, removed: u8, switch_in: Micros, now: Micros) {
        let removal = self.removal(remover, removed, switch_in, now);
        if let Some(mut lost) = self
            .view
            .removals
            .current
            .filter(|current| matches!(current.stage, RemovalStage::Pending { .. }))
        {
            lost.stage = RemovalStage::Lost;
            self.view.removals.rival = Some(lost);
        }
        self.view.removals.current = Some(removal);
        self.changed = true;
    }

    /// A rival of the removal under way that lost to it: member `remover` asked to remove
    /// member `removed`.
    pub fn losing_rival(&mut self, remover: u8, removed: u8, now: Micros) {
        let mut rival = self.removal(remover, removed, 0, now);
        rival.stage = RemovalStage::Lost;
        self.view.removals.rival = Some(rival);
        self.changed = true;
    }

    /// Member `by` removed this device, and its notice arrives at `now`.
    pub fn removed_by(&mut self, by: u8, now: Micros) {
        let name = self
            .view
            .group
            .as_ref()
            .and_then(|group| group.member(by))
            .map_or_else(
                || Name::from_mac(&[0, 0, 0, 0, 0, by]),
                |member| member.name,
            );
        self.view.removals.removed_by = Some((by, name, now as At));
        self.changed = true;
    }

    /// Switches to the removal under way at `now`: its member goes, and another member's can be
    /// declined for a day.
    fn switch(&mut self, now: Micros) {
        let own = self.view.group.as_ref().map_or(0, |group| group.own);
        let Some(current) = &mut self.view.removals.current else {
            return;
        };
        let decline = if current.remover == own {
            Decline::Own
        } else {
            Decline::Until((now + DECLINE_FOR) as At)
        };
        current.stage = RemovalStage::Switched {
            at: now as At,
            decline,
        };
        let removed = current.removed;
        if let Some(group) = &mut self.view.group
            && let Some(member) = group.members[usize::from(removed)].take()
        {
            self.removed = Some((removed, member));
        }
        self.changed = true;
    }

    #[must_use]
    pub fn messages(&self) -> &MessagesView {
        &self.messages
    }

    /// Whether the messages changed since the last call.
    pub fn messages_changed(&mut self) -> bool {
        core::mem::take(&mut self.messages_changed)
    }

    /// Adds a message as though it had come, at `now`.
    pub fn arrive(&mut self, arrival: Arrival, now: Micros) -> u32 {
        let id = self.next_message;
        self.next_message += 1;
        let Arrival {
            from,
            to,
            text,
            ago,
            carriage,
            unread,
        } = arrival;
        let mut message = MessageView::new(id, now as At - ago as At, from, to, text.as_bytes());
        message.seq = id;
        message.carriage = carriage;
        message.unread = unread;
        // The other member, as the node names it: the sender, or the recipient of this
        // device's own.
        if let Some(group) = &self.view.group {
            let peer = if from == group.own { to } else { Some(from) };
            if let Some(member) = peer.and_then(|id| group.member(id)) {
                message.set_peer(member.device, member.name);
            }
        }
        self.messages.push(message);
        self.messages_changed = true;
        id
    }

    /// The design's conversations, with the members at ids 1 and 2: the group's and one with
    /// each, some unread, at `now`.
    pub fn conversations(&mut self, now: Micros) {
        let own = self.view.group.as_ref().map_or(0, |group| group.own);
        let minute = 60 * SECOND;
        self.arrive(
            Arrival {
                from: 2,
                to: None,
                text: "Meet at the bridge.",
                ago: minute,
                carriage: Carriage::Received,
                unread: true,
            },
            now,
        );
        self.arrive(
            Arrival {
                from: own,
                to: None,
                text: "I will bring the spare cells.",
                ago: 2 * minute,
                carriage: Carriage::Relayed,
                unread: false,
            },
            now,
        );
        self.arrive(
            Arrival {
                from: 1,
                to: None,
                text: "North path is clear.",
                ago: 4 * minute,
                carriage: Carriage::Received,
                unread: false,
            },
            now,
        );
        self.arrive(
            Arrival {
                from: 1,
                to: Some(own),
                text: "Take the north path. I will wait at the turn.",
                ago: 2 * minute,
                carriage: Carriage::Received,
                unread: true,
            },
            now,
        );
        self.arrive(
            Arrival {
                from: own,
                to: Some(1),
                text: "On my way.",
                ago: 3 * minute,
                carriage: Carriage::Delivered,
                unread: false,
            },
            now,
        );
        self.arrive(
            Arrival {
                from: 1,
                to: Some(own),
                text: "See you there.",
                ago: 8 * minute,
                carriage: Carriage::Received,
                unread: false,
            },
            now,
        );
        self.arrive(
            Arrival {
                from: own,
                to: Some(2),
                text: "On my way.",
                ago: 8 * minute,
                carriage: Carriage::Delivered,
                unread: false,
            },
            now,
        );
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
        let removing = self
            .view
            .removals
            .current
            .filter(|current| matches!(current.stage, RemovalStage::Pending { .. }));
        match request {
            // As the node does, adding waits for a removal's switch.
            Request::Add if let Some(removing) = removing => {
                self.view.sessions += 1;
                self.view.pairing = None;
                self.view.refusal = Some(Refusal {
                    session: self.view.sessions,
                    role: Role::Add,
                    refused: Refused::Removing { key: removing.key },
                });
            }
            Request::Add | Request::Join if !active => {
                let role = if request == Request::Add {
                    Role::Add
                } else {
                    Role::Join
                };
                self.view.recovery = None;
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
                    self.view.recovery = None;
                    self.view.removals = Default::default();
                    self.next = None;
                }
                self.answer(Answer::Left(ok));
            }
            Request::Send { to, text } => {
                let to = match to {
                    Thread::Group => None,
                    Thread::Member(id, device) => {
                        let holds = self.view.group.as_ref().is_some_and(|group| {
                            group
                                .member(id)
                                .is_some_and(|member| member.device == device)
                        });
                        if !holds {
                            return;
                        }
                        Some(id)
                    }
                };
                let own = self.view.group.as_ref().map_or(0, |group| group.own);
                let id = self.arrive(
                    Arrival {
                        from: own,
                        to,
                        text: text.as_str(),
                        ago: 0,
                        carriage: Carriage::Queued,
                        unread: false,
                    },
                    now,
                );
                let mut later = [
                    Some((SENT_AFTER, Carried::Goes(Carriage::Sent))),
                    Some((RELAYED_AFTER, Carried::Goes(Carriage::Relayed))),
                    to.map(|_| (DELIVERED_AFTER, Carried::Goes(Carriage::Delivered))),
                    to.map(|_| (ANSWERED_AFTER, Carried::Answered)),
                ];
                for (after, carried) in later.iter_mut().flatten() {
                    _ = self.carried.push((now + *after, id, *carried));
                }
            }
            Request::Read(id) => {
                if let Some(message) = self.messages.get_mut(id) {
                    message.unread = false;
                    self.messages_changed = true;
                }
            }
            Request::Remove { id, device } => {
                let own = self.view.group.as_ref().map_or(0, |group| group.own);
                let member = self
                    .view
                    .group
                    .as_ref()
                    .and_then(|group| group.member(id))
                    .filter(|member| id != own && member.device == device);
                let underway = self
                    .view
                    .removals
                    .current
                    .filter(|current| matches!(current.stage, RemovalStage::Pending { .. }));
                let started = if member.is_none() {
                    Err(Unremovable::Changed)
                } else if let Some(underway) = underway {
                    Err(Unremovable::Underway { key: underway.key })
                } else if self.store_fails {
                    Err(Unremovable::Unsaved)
                } else {
                    Ok(())
                };
                if started.is_ok() {
                    self.view.removals.current = Some(self.removal(own, id, SWITCH_AFTER, now));
                }
                self.answer(Answer::Removing(started));
            }
            Request::Keep { key } => {
                let own = self.view.group.as_ref().map_or(0, |group| group.own);
                if let Some(current) = &mut self.view.removals.current
                    && current.key == key
                    && current.remover != own
                {
                    match current.stage {
                        RemovalStage::Pending { .. } => {
                            current.stage = RemovalStage::Declined { at: now as At };
                        }
                        RemovalStage::Switched {
                            decline: Decline::Until(until),
                            ..
                        } if now as At <= until => {
                            current.stage = RemovalStage::Declined { at: now as At };
                            if let (Some((id, member)), Some(group)) =
                                (self.removed.take(), &mut self.view.group)
                            {
                                group.members[usize::from(id)] = Some(member);
                            }
                        }
                        _ => {}
                    }
                }
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
        match self.view.removals.current.map(|current| current.stage) {
            Some(RemovalStage::Pending {
                switch: Some(switch),
                ..
            }) if now as At >= switch => self.switch(now),
            Some(RemovalStage::Switched {
                at,
                decline: Decline::Until(until),
            }) if now as At > until => {
                if let Some(current) = &mut self.view.removals.current {
                    current.stage = RemovalStage::Switched {
                        at,
                        decline: Decline::Expired,
                    };
                }
                self.removed = None;
                self.changed = true;
            }
            _ => {}
        }
        while let Some((at, next)) = self.next
            && now >= at
        {
            self.next = None;
            self.changed = true;
            self.advance(next, at);
        }
        while let Some(at) = self.carried.iter().position(|&(due, ..)| now >= due) {
            let (_, id, carried) = self.carried.remove(at);
            let Some(message) = self.messages.get(id).copied() else {
                continue;
            };
            match carried {
                Carried::Goes(carriage) => {
                    if let Some(message) = self.messages.get_mut(id) {
                        message.carriage = carriage;
                    }
                }
                Carried::Answered => {
                    if let Some(to) = message.to {
                        self.arrive(
                            Arrival {
                                from: to,
                                to: Some(message.from),
                                text: "Got it.",
                                ago: 0,
                                carriage: Carriage::Received,
                                unread: true,
                            },
                            now,
                        );
                    }
                }
            }
            self.messages_changed = true;
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
                            device: device(&PEER_MAC),
                            joined: Some(now as At),
                            heard: None,
                            position: Position::Never,
                            coordinates: None,
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
                            device: device(&self.view.mac),
                            joined: Some(now as At),
                            heard: None,
                            position: Position::Never,
                            coordinates: None,
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
                    device: device(&PEER_MAC),
                    joined: Some(now as At),
                    heard: Some(now as At),
                    position: Position::Never,
                    coordinates: None,
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
