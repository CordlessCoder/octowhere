//! The crowded group `crowded-screens-bench` stands in for the one two boards cannot make: the
//! 2026-10-05 hand-off's sectors fixture, as `sectors` with `RUNS` in
//! `crates/octowhere-ui/examples/render/members.rs` builds it for the host, and a private message
//! to this device from each of its members. It has no board dependency, so `host-tests` checks
//! it against the stage.

use octowhere_ui::ui::{
    compass::CompassView,
    group::view::{
        Carriage, GroupView, IDS, MemberView, MeshView, MessageView, MessagesView, Name, Position,
    },
};

/// The position the bench injects as this device's fix, which the members are placed around:
/// 53.3498 N, 6.2603 W, in degrees × 10⁷.
pub const CENTRE: (i32, i32) = (533_498_000, -62_603_000);

/// The members besides this device.
pub const COUNT: usize = IDS as usize - 1;

const SECOND: i64 = 1_000_000;

/// Each member's name, its position's offset from [`CENTRE`] in degrees × 10⁷, and how many
/// seconds old that position is and since it was last heard, in the fixture's order. Kestrel is
/// 150 m away; the others are 400 m away in four runs. The offsets are the render's spherical
/// `at` for each bearing and distance, rounded as it rounds them.
const MEMBERS: [(&str, (i32, i32), i64, i64); COUNT] = [
    ("Kestrel", (9_866, 15_413), 23, 7),         // 043°
    ("Lough", (29_467, 34_568), 11, 60),         // 035°
    ("Basecamp", (27_823, 38_199), 189, 60),     // 039.3°
    ("Kestrel", (26_021, 41_612), 367, 60),      // 043.7°
    ("Lough", (24_070, 44_787), 545, 60),        // 048°
    ("Basecamp", (21_981, 47_706), 723, 60),     // 052.3°
    ("Kestrel", (19_766, 50_352), 901, 60),      // 056.7°
    ("Lough", (17_439, 52_710), 1_080, 60),      // 061°
    ("Basecamp", (-16_889, 53_207), 23, 60),     // 118°
    ("Kestrel", (-19_668, 50_457), 79, 60),      // 123.1°
    ("Lough", (-22_289, 47_300), 136, 60),       // 128.3°
    ("Basecamp", (-24_730, 43_763), 193, 60),    // 133.4°
    ("Kestrel", (-26_972, 39_873), 249, 60),     // 138.6°
    ("Lough", (-28_997, 35_662), 306, 60),       // 143.7°
    ("Basecamp", (-30_789, 31_164), 363, 60),    // 148.9°
    ("Kestrel", (-32_332, 26_416), 420, 60),     // 154°
    ("Lough", (-28_348, -37_099), 120, 60),      // 218°
    ("Basecamp", (-26_734, -40_321), 257, 60),   // 222°
    ("Kestrel", (-24_990, -43_347), 394, 60),    // 226°
    ("Lough", (-23_124, -46_162), 531, 60),      // 230°
    ("Basecamp", (-21_145, -48_752), 668, 60),   // 234°
    ("Kestrel", (-19_064, -51_104), 805, 60),    // 238°
    ("Lough", (-16_889, -53_207), 942, 60),      // 242°
    ("Basecamp", (-14_633, -55_051), 1_080, 60), // 246°
    ("Kestrel", (20_632, -49_367), 11, 60),      // 305°
    ("Lough", (23_122, -46_167), 39, 60),        // 310°
    ("Basecamp", (25_436, -42_615), 67, 60),     // 315°
    ("Kestrel", (27_556, -38_739), 95, 60),      // 320°
    ("Lough", (29_467, -34_568), 123, 60),       // 325°
    ("Basecamp", (31_153, -30_134), 151, 60),    // 330°
    ("Kestrel", (32_602, -25_470), 180, 60),     // 335°
];

/// What member `index` sends this device. A conversation's row shows up to two lines of it.
const TEXTS: [&str; 4] = [
    "At the north gate. Holding here until the others arrive.",
    "Ridge path is clear; moving down to the river trail now.",
    "Low on water. Heading back to the car park in ten minutes.",
    "Signal is poor up here. Will check in again from the summit.",
];

/// The id of the first member's message; the rest follow it up to `u32::MAX`. The node numbers
/// its own up from 1, and would have to wrap to reach them.
const FIRST_MESSAGE: u32 = u32::MAX - COUNT as u32 + 1;
/// How far apart the members sent their messages, the last member's newest.
const SPACING: i64 = 40 * SECOND;

/// The ids the members take, in the fixture's order: every id but this device's.
pub fn ids(own: u8) -> impl Iterator<Item = u8> {
    (0..IDS).filter(move |&id| id != own)
}

fn name(index: usize) -> Name {
    Name::new(MEMBERS[index].0.as_bytes()).expect("a fixture name is printable")
}

/// Member `index`'s device fingerprint, the render's for the same member.
fn device(index: usize) -> [u8; 8] {
    match index {
        0 => [0x9c, 0x2a, 0x7f, 0x10, 0xe5, 0x31, 0xa8, 1],
        _ => [0x9c, 0x2a, 0x7f, 0x10, 0, 0, 0, index as u8 + 1],
    }
}

fn member(index: usize, now: i64) -> MemberView {
    let (label, (north, east), age, heard) = MEMBERS[index];
    let ago = |seconds: i64| now - seconds * SECOND;
    MemberView {
        name: name(index),
        mac: [0x48, 0xa1, 0xb2, 0xc3, 0x8c, label.len() as u8],
        device: device(index),
        joined: Some(ago(86_400)),
        heard: Some(ago(heard)),
        position: Position::At(ago(age)),
        coordinates: Some((CENTRE.0 + north, CENTRE.1 + east)),
    }
}

/// Puts the members at every id but this device's, with their positions' ages and when they
/// were heard counted back from `now`, on the stage's clock. In no group, this device is member
/// 0 of a group made here.
pub fn place(view: &mut MeshView, now: i64) {
    // A constant is copied into place; the same group written inline was built on the stack
    // first, about 3 KB of it.
    const ALONE: GroupView = GroupView {
        own: 0,
        members: [None; IDS as usize],
    };
    let (own_name, mac) = (view.name, view.mac);
    let group = match &mut view.group {
        Some(group) => group,
        none => none.insert(ALONE),
    };
    let own = &mut group.members[usize::from(group.own)];
    if own.is_none() {
        *own = Some(MemberView {
            name: own_name,
            mac,
            device: [0; 8],
            joined: None,
            heard: None,
            position: Position::Never,
            coordinates: None,
        });
    }
    for (index, id) in ids(group.own).enumerate() {
        group.members[usize::from(id)] = Some(member(index, now));
    }
}

/// Which member sent message `id`, if a member here did.
#[must_use]
pub fn sender(id: u32) -> Option<usize> {
    id.checked_sub(FIRST_MESSAGE).map(|index| index as usize)
}

/// Puts a private message from each member to this device, member `own`, among `messages`, in
/// place of any put there before. Each is unread unless its member's bit in `read` is set, and
/// the last member's was sent at `newest`, on the stage's clock.
pub fn add_messages(messages: &mut MessagesView, own: u8, read: u32, newest: i64) {
    messages.retain(|message| sender(message.id).is_none());
    for (index, from) in ids(own).enumerate() {
        let sent = newest - (COUNT - 1 - index) as i64 * SPACING;
        let text = TEXTS[index % TEXTS.len()];
        let mut message = MessageView::new(
            FIRST_MESSAGE + index as u32,
            sent,
            from,
            Some(own),
            text.as_bytes(),
        );
        message.seq = 1;
        message.carriage = Carriage::Received;
        message.unread = read & 1 << index == 0;
        message.set_peer(device(index), name(index));
        messages.push(message);
    }
}

/// How many conversations `messages` hold, seen from member `own`: the rows the Messages root
/// lists.
#[must_use]
pub fn conversations(messages: &MessagesView, own: u8) -> usize {
    messages
        .iter()
        .enumerate()
        .filter(|&(i, message)| {
            let thread = message.thread(own);
            messages
                .iter()
                .take(i)
                .all(|earlier| earlier.thread(own) != thread)
        })
        .count()
}

/// The compass as the motion task publishes it, calibrated, level and undisturbed, turned
/// clockwise at `rate` degrees a second for `elapsed` microseconds from `from` tenths of a degree.
#[must_use]
pub fn turned(from: u16, rate: i32, elapsed: u64) -> CompassView {
    let turned = i64::from(rate) * 10 * elapsed as i64 / 1_000_000;
    CompassView {
        live: true,
        calibration_percent: 100,
        heading_decidegrees: Some((i64::from(from) + turned).rem_euclid(3_600) as u16),
        pitch_deg: 0,
        roll_deg: 0,
        disturbed: false,
    }
}
