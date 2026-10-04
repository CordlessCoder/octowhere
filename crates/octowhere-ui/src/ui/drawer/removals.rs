//! A removal's screens in the drawer, from the 2026-10-04 hand-off: a request as it stands,
//! before and after this device switches, its details, the two confirmations of declining it,
//! two requests that compete, and the notice that this device was removed. Every screen names
//! its request by its new key, since a removed member's id can be taken again.

use embedded_graphics::{
    prelude::{Point, Size},
    primitives::Rectangle,
};

use super::{
    age_due,
    parts::{self, FOOTER, PAIR, Symbol},
    rows::{Look, age},
};
use crate::{
    chrome::{self, Color, FontdueRenderer},
    ui::{
        gesture::Micros,
        group::{
            layout::{Face, Line, List, format, rect},
            view::{Decline, Name, RemovalStage, RemovalView},
        },
        slide::{Slide, Track},
        text::style,
    },
};

const SECOND: Micros = 1_000_000;
const MINUTE: Micros = 60 * SECOND;
const HOUR: Micros = 60 * MINUTE;
const DAY: Micros = 24 * HOUR;
/// The elapsed rail under this device's own request's countdown.
const RAIL: Rectangle = rect(118, 348, 348, 352);
/// The rivals' rows: their tops, and the height down to the rule under each.
const RIVAL_TOPS: [i32; 2] = [134, 260];
const RIVAL_ROW: i32 = 103;
/// The confirmations' slide: its track, the handle at rest and the box it is carried to.
const TRACK: Rectangle = rect(82, 348, 384, 396);
const HANDLE: Rectangle = rect(82, 348, 142, 396);
const TARGET: Rectangle = rect(324, 348, 384, 396);
pub const SLIDE: Track = Track {
    grab: rect(78, 342, 146, 402),
    travel: 242,
};

/// What the removal screens are drawn for: this device's id in its group, the time, and the
/// font titles are measured in.
pub struct Show<'a> {
    pub own: u8,
    pub now: Micros,
    pub font: &'a FontdueRenderer<'static, Color>,
}

/// How wide a sentence may run, centred and down the reading column.
const CENTRED: f32 = 320.0;
const COLUMN: f32 = 290.0;

impl Show<'_> {
    /// `with`, a sentence naming a member, where it fits `width` in Sans at `size`, and
    /// `without`, which names nobody, where a long name runs it wider.
    fn naming(&self, with: Line, without: &str, size: u8, width: f32) -> Line {
        if measure(self.font, Face::Sans, size, &with) <= width {
            with
        } else {
            format(format_args!("{without}"))
        }
    }
}

fn measure(font: &FontdueRenderer<'static, Color>, face: Face, size: u8, text: &str) -> f32 {
    style(font, chrome::WHITE, u32::from(size), face.index()).advance(text)
}

/// `text` at `size` in `face`, cut short with an ellipsis where it runs past `width`.
fn fitted(
    font: &FontdueRenderer<'static, Color>,
    text: &str,
    face: Face,
    size: u8,
    width: f32,
) -> Line {
    parts::fitted(
        &style(font, chrome::WHITE, u32::from(size), face.index()),
        text,
        width,
    )
}

/// `text` in capitals, as the faces without lower case set it.
#[must_use]
pub fn upper(text: &str) -> Line {
    let mut line = Line::new();
    for c in text.chars() {
        if line.push(c.to_ascii_uppercase()).is_err() {
            break;
        }
    }
    line
}

/// A fingerprint's first four bytes in hex, as the screens abbreviate it.
fn short(bytes: &[u8; 8]) -> Line {
    format(format_args!(
        "{:02X}{:02X}{:02X}{:02X}",
        bytes[0], bytes[1], bytes[2], bytes[3]
    ))
}

fn full(bytes: &[u8; 8]) -> Line {
    let mut line = Line::new();
    for byte in bytes {
        _ = line.push_str(&format(format_args!("{byte:02X}")));
    }
    line
}

/// The time left until `at` as two fields, whole `unit`s and sixtieths of one, rounded up, and
/// when that next changes.
fn left(at: i64, now: Micros, unit: Micros) -> (Line, Micros) {
    let left = (at - now as i64).max(0) as Micros;
    let small = unit / 60;
    let count = left.div_ceil(small);
    let text = format(format_args!("{:02}:{:02}", count / 60, count % 60));
    let step = match left % small {
        0 => small,
        part => part,
    };
    (text, now + step)
}

/// Whether member `own`'s device can still decline `removal` at `now`.
#[must_use]
pub fn declinable(removal: &RemovalView, own: u8, now: Micros) -> bool {
    removal.remover != own
        && match removal.stage {
            RemovalStage::Pending { .. } => true,
            RemovalStage::Switched {
                decline: Decline::Until(until),
                ..
            } => now as i64 <= until,
            _ => false,
        }
}

/// Why a switched removal cannot be declined.
fn why_not(removal: &RemovalView, own: u8) -> &'static str {
    match removal.stage {
        _ if removal.remover == own => "You cannot decline your own request.",
        RemovalStage::Switched {
            decline: Decline::Later,
            ..
        } => "A later group change is under way.",
        _ => "The time to decline has ended.",
    }
}

/// How a removal's event reads in its row and its toast.
#[must_use]
pub fn look(removal: &RemovalView, own: u8, now: Micros) -> Look {
    let removed = removal.removed_name.as_str();
    let by = format(format_args!(
        "Requested by {} [{:02}].",
        removal.remover_name.as_str(),
        removal.remover
    ));
    let mine = removal.remover == own;
    let (state, lines, toast) = match removal.stage {
        RemovalStage::Pending { switch, .. } => (
            if mine { "REMOVING" } else { "REQUEST" },
            [
                if mine {
                    format(format_args!("Your request."))
                } else {
                    by
                },
                match switch {
                    Some(at) => format(format_args!("Switch in {}.", left(at, now, MINUTE).0)),
                    None => format(format_args!("Switch time unavailable.")),
                },
            ],
            format(format_args!(
                "{} asks to remove {removed}.",
                removal.remover_name.as_str()
            )),
        ),
        RemovalStage::Switched { .. } => (
            "SWITCHED",
            [
                format(format_args!("{removed} was removed.")),
                if declinable(removal, own, now) {
                    format(format_args!("Decline available."))
                } else {
                    format(format_args!("Decline unavailable."))
                },
            ],
            format(format_args!("{removed} was removed from this group.")),
        ),
        RemovalStage::Declined { .. } => (
            "DECLINED",
            [
                format(format_args!("This device kept {removed}.")),
                format(format_args!("Others may have switched.")),
            ],
            format(format_args!("This device kept {removed}.")),
        ),
        RemovalStage::Lost => (
            "RIVAL",
            [by, format(format_args!("Another removal won."))],
            format(format_args!("Two removals compete.")),
        ),
    };
    Look {
        state,
        state_color: chrome::ORANGE,
        title: format(format_args!(
            "REMOVE {}",
            upper(removal.removed_name.as_str())
        )),
        color: chrome::ORANGE,
        symbol: Symbol::Remove,
        lines,
        toast,
        hint: "VIEW REQUEST",
        rail: None,
    }
}

/// How the notice that this device was removed reads in its row and its toast.
#[must_use]
pub fn removed_look(by: u8, name: &Name) -> Look {
    Look {
        state: "GROUP CHANGE",
        state_color: chrome::ORANGE,
        title: format(format_args!("REMOVED")),
        color: chrome::ORANGE,
        symbol: Symbol::Remove,
        lines: [
            format(format_args!("Removed by {} [{by:02}].", name.as_str())),
            format(format_args!("This device stays in the old group.")),
        ],
        toast: format(format_args!("You were removed by {}.", name.as_str())),
        hint: "VIEW NOTICE",
        rail: None,
    }
}

/// What a tap on a removal's detail asks for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Act {
    Details,
    Decline,
    ViewGroup,
}

/// What a removal's detail offers, and where.
#[must_use]
pub fn buttons(removal: &RemovalView, own: u8, now: Micros) -> [(Rectangle, Act); 2] {
    match removal.stage {
        RemovalStage::Pending { .. } if removal.remover != own => {
            [(PAIR[0], Act::Details), (PAIR[1], Act::Decline)]
        }
        RemovalStage::Switched { .. } if declinable(removal, own, now) => {
            [(PAIR[0], Act::ViewGroup), (PAIR[1], Act::Decline)]
        }
        _ => [(FOOTER, Act::ViewGroup); 2],
    }
}

fn centred(list: &mut List, text: &str, top: i32, face: Face, size: u8, color: Color) {
    list.text(parts::centred(text, parts::CENTRE, top, face, size, color));
}

/// Lines of prose down the reading column from `top`, at `size` rather than the drawer's.
fn lines(list: &mut List, lines: &[&str], top: i32, size: u8) {
    for (i, line) in lines.iter().enumerate() {
        list.text(parts::text(
            line,
            parts::PROSE_X,
            top + i as i32 * parts::PROSE_PITCH,
            Face::Sans,
            size,
            chrome::WHITE,
        ));
    }
}

/// The member removed, large and orange, and under it `under`. A long name takes a smaller
/// size before it is cut short.
pub fn target(list: &mut List, name: &Name, under: &str, font: &FontdueRenderer<'static, Color>) {
    let name = upper(name.as_str());
    let (size, name) = if measure(font, Face::Kh, 35, &name) <= CENTRED {
        (35, name)
    } else {
        (28, fitted(font, &name, Face::Kh, 28, CENTRED))
    };
    centred(list, &name, 132, Face::Kh, size, chrome::ORANGE);
    centred(list, under, 178, Face::Mono, 11, chrome::GRAY);
}

/// The member's id and its device, as the screens that remove it caption it.
#[must_use]
pub fn device_caption(id: u8, device: &[u8; 8]) -> Line {
    format(format_args!("[{id:02}] / DEVICE {}", short(device)))
}

fn requested_by(removal: &RemovalView, own: u8) -> Line {
    if removal.remover == own {
        return format(format_args!("[{:02}] / YOUR REQUEST", removal.removed));
    }
    format(format_args!(
        "[{:02}] / REQUESTED BY {} [{:02}]",
        removal.removed,
        upper(removal.remover_name.as_str()),
        removal.remover
    ))
}

/// The time left to `at` as the screens that count down to a switch show it, noting in `list`
/// when it next changes.
#[must_use]
pub fn switch_in(at: i64, now: Micros, list: &mut List) -> Line {
    let (text, due) = left(at, now, MINUTE);
    list.changes_at(due);
    text
}

/// A removal's detail as it stands: before this device switches, after it with the time left to
/// decline, or past that; this device's own request with its countdown.
pub fn detail(list: &mut List, removal: &RemovalView, show: &Show) {
    let own = show.own;
    let now = show.now;
    let removed = removal.removed_name.as_str();
    parts::back(list);
    match removal.stage {
        RemovalStage::Pending { since, switch } if removal.remover == own => {
            parts::title(list, "REMOVING", show.font);
            parts::meta(list, "YOUR REQUEST");
            target(
                list,
                &removal.removed_name,
                &device_caption(removal.removed, &removal.device),
                show.font,
            );
            centred(list, "SWITCH IN", 208, Face::Mono, 11, chrome::GRAY);
            match switch {
                Some(at) => {
                    let text = switch_in(at, now, list);
                    centred(list, &text, 230, Face::Kh, 43, chrome::WHITE);
                    list.fill(RAIL, chrome::TRACK);
                    let whole = (at - since).max(1);
                    let done = (now as i64 - since).clamp(0, whole);
                    let filled = (i64::from(RAIL.size.width) * done / whole) as u32;
                    if filled > 0 {
                        list.fill(
                            Rectangle::new(RAIL.top_left, Size::new(filled, RAIL.size.height)),
                            chrome::WHITE,
                        );
                    }
                }
                None => centred(list, "UNAVAILABLE", 236, Face::Kh, 30, chrome::GRAY),
            }
            centred(
                list,
                &show.naming(
                    format(format_args!("{removed} remains in the group until then.")),
                    "The member remains in the group until then.",
                    16,
                    CENTRED,
                ),
                287,
                Face::Sans,
                16,
                chrome::WHITE,
            );
            centred(
                list,
                "Sharing the group change.",
                316,
                Face::Sans,
                15,
                chrome::GRAY,
            );
            centred(
                list,
                why_not(removal, own),
                370,
                Face::Sans,
                14,
                chrome::GRAY,
            );
            parts::button(list, FOOTER, "VIEW GROUP", true);
        }
        RemovalStage::Pending { switch, .. } => {
            parts::title(list, "GROUP CHANGE", show.font);
            parts::meta(list, "REMOVAL REQUEST");
            target(
                list,
                &removal.removed_name,
                &requested_by(removal, show.own),
                show.font,
            );
            let scheduled = match switch {
                Some(at) => format(format_args!(
                    "Scheduled switch in {}.",
                    switch_in(at, now, list)
                )),
                None => format(format_args!("Switch time unavailable.")),
            };
            centred(list, &scheduled, 214, Face::Sans, 18, chrome::WHITE);
            lines(
                list,
                &["If you do nothing, this device", "switches with the group."],
                257,
                17,
            );
            centred(
                list,
                &show.naming(
                    format(format_args!("{removed} is still a member until then.")),
                    "Still a member until the switch.",
                    15,
                    CENTRED,
                ),
                323,
                Face::Sans,
                15,
                chrome::GRAY,
            );
            parts::button(list, PAIR[0], "DETAILS", true);
            parts::consequential(list, PAIR[1], "DECLINE");
        }
        RemovalStage::Switched { at, decline } => {
            parts::title(list, "GROUP CHANGE", show.font);
            let since = (now as i64 - at).max(0) as Micros;
            let meta = if since >= DAY {
                format(format_args!("SWITCHED OVER A DAY AGO"))
            } else {
                list.changes_at(now + age_due(since));
                format(format_args!("SWITCHED {} AGO", age(since)))
            };
            parts::meta(list, &meta);
            target(
                list,
                &removal.removed_name,
                &requested_by(removal, show.own),
                show.font,
            );
            centred(
                list,
                &show.naming(
                    format(format_args!("{removed} was removed from this group.")),
                    "The member was removed from this group.",
                    17,
                    CENTRED,
                ),
                214,
                Face::Sans,
                17,
                chrome::WHITE,
            );
            match decline {
                Decline::Until(until) if declinable(removal, own, now) => {
                    let (text, due) = left(until, now, HOUR);
                    // And once more as the time runs out.
                    list.changes_at(due.min(until.max(0) as Micros + 1));
                    centred(list, "DECLINE AVAILABLE", 258, Face::Mono, 11, chrome::GRAY);
                    centred(
                        list,
                        &format(format_args!("{text} LEFT")),
                        281,
                        Face::Kh,
                        32,
                        chrome::WHITE,
                    );
                    centred(
                        list,
                        "Return to the previous group if declined.",
                        333,
                        Face::Sans,
                        14,
                        chrome::GRAY,
                    );
                    parts::button(list, PAIR[0], "VIEW GROUP", true);
                    parts::consequential(list, PAIR[1], "DECLINE");
                }
                _ => {
                    centred(list, "DECLINE UNAVAILABLE", 267, Face::Kh, 20, chrome::GRAY);
                    centred(
                        list,
                        why_not(removal, own),
                        312,
                        Face::Sans,
                        17,
                        chrome::GRAY,
                    );
                    parts::button(list, FOOTER, "VIEW GROUP", true);
                }
            }
        }
        RemovalStage::Declined { .. } => {
            parts::title(list, "GROUP CHANGE", show.font);
            parts::meta(list, "DECLINED HERE");
            target(
                list,
                &removal.removed_name,
                &requested_by(removal, show.own),
                show.font,
            );
            centred(
                list,
                &show.naming(
                    format(format_args!("This device kept {removed}.")),
                    "This device kept the member.",
                    17,
                    CENTRED,
                ),
                214,
                Face::Sans,
                17,
                chrome::WHITE,
            );
            lines(
                list,
                &[
                    "Other devices may have switched.",
                    "Declining did not cancel",
                    "their group change.",
                ],
                257,
                17,
            );
            parts::button(list, FOOTER, "VIEW GROUP", true);
        }
        RemovalStage::Lost => {
            parts::title(list, "GROUP CHANGE", show.font);
            parts::meta(list, "RIVAL REQUEST");
            target(
                list,
                &removal.removed_name,
                &requested_by(removal, show.own),
                show.font,
            );
            centred(
                list,
                "Another removal won over this one.",
                214,
                Face::Sans,
                17,
                chrome::WHITE,
            );
            centred(
                list,
                "This request has no effect.",
                258,
                Face::Sans,
                17,
                chrome::GRAY,
            );
            parts::button(list, FOOTER, "VIEW GROUP", true);
        }
    }
}

/// A request in full: whom it removes, by which device, who asked, its key and its switch.
pub fn details(list: &mut List, removal: &RemovalView, show: &Show) {
    let now = show.now;
    parts::back(list);
    parts::title(list, "DETAILS", show.font);
    parts::meta(list, "REMOVAL REQUEST");
    target(
        list,
        &removal.removed_name,
        &requested_by(removal, show.own),
        show.font,
    );
    parts::prose(
        list,
        &[
            "A request is named by its key.",
            "Names and ids can be reused.",
        ],
        214,
    );
    let device = full(&removal.device);
    let key = full(&removal.key);
    let switch = match removal.stage {
        RemovalStage::Pending {
            switch: Some(at), ..
        } => format(format_args!("IN {}", switch_in(at, now, list))),
        RemovalStage::Pending { switch: None, .. } => format(format_args!("UNAVAILABLE")),
        RemovalStage::Switched { at, .. } => {
            let since = (now as i64 - at).max(0) as Micros;
            list.changes_at(now + age_due(since));
            format(format_args!("{} AGO", age(since)))
        }
        RemovalStage::Declined { .. } => format(format_args!("DECLINED HERE")),
        RemovalStage::Lost => format(format_args!("NONE, IT LOST")),
    };
    parts::figures(
        list,
        &[
            ("DEVICE", &device),
            ("REQUEST KEY", &key),
            ("SWITCH", &switch),
        ],
    );
}

/// Declining, before this device switches or after: what it costs, and the slide.
pub fn confirm(list: &mut List, removal: &RemovalView, slide: &Slide, show: &Show) {
    parts::back(list);
    parts::title(list, "DECLINE", show.font);
    let meta = format(format_args!(
        "{} / REQUESTED BY {} [{:02}]",
        upper(removal.removed_name.as_str()),
        upper(removal.remover_name.as_str()),
        removal.remover
    ));
    parts::meta(list, &fitted(show.font, &meta, Face::Mono, 12, CENTRED));
    let keep = show.naming(
        format(format_args!(
            "This device will keep {}.",
            removal.removed_name.as_str()
        )),
        "This device will keep the member.",
        16,
        COLUMN,
    );
    let (heading, prose): (&str, [&str; 4]) = match removal.stage {
        RemovalStage::Switched { .. } => (
            "RETURN TO THE OLD GROUP",
            [
                "Positions, messages and changes",
                "learned since the switch are lost.",
                "Earlier records can return from",
                "devices still in the old group.",
            ],
        ),
        _ => (
            "STAY IN THE OLD GROUP",
            [
                &keep,
                "Other devices may switch away.",
                "Declining here does not cancel",
                "their group change.",
            ],
        ),
    };
    centred(list, heading, 139, Face::Kh, 21, chrome::ORANGE);
    lines(list, &prose, 191, 16);
    slider(list, "SLIDE TO DECLINE", slide, show.now);
}

/// The slide's prompt over its track, the box at its end, the handle, and CANCEL under it.
pub fn slider(list: &mut List, prompt: &str, slide: &Slide, now: Micros) {
    centred(list, prompt, 320, Face::Kh, 16, chrome::ORANGE);
    list.outline(TRACK, chrome::GRAY);
    list.outline(TARGET, chrome::ORANGE);
    let travel = slide.travel(now, &SLIDE);
    let handle = Rectangle::new(HANDLE.top_left + Point::new(travel, 0), HANDLE.size);
    list.fill(handle, chrome::ORANGE);
    let (x, y) = (handle.top_left.x as i16 * 4, handle.top_left.y as i16 * 4);
    list.path(
        &[(x + 96, y + 56), (x + 136, y + 96), (x + 96, y + 136)],
        8,
        chrome::BLACK,
    );
    parts::button(list, FOOTER, "CANCEL", true);
}

/// Two requests that compete, the winner first, with the one selected marked.
pub fn rivals(list: &mut List, requests: [&RemovalView; 2], selected: usize, show: &Show) {
    parts::back(list);
    parts::title(list, "TWO REQUESTS", show.font);
    parts::meta(list, "REVIEW EACH REMOVAL");
    for (i, (removal, top)) in requests.into_iter().zip(RIVAL_TOPS).enumerate() {
        if i == selected {
            list.fill(rect(75, top, 77, top + RIVAL_ROW), chrome::WHITE);
        }
        let state = if removal.stage == RemovalStage::Lost {
            "RIVAL REQUEST"
        } else {
            "CURRENT WINNER"
        };
        list.text(parts::text(state, 88, top, Face::Mono, 11, chrome::GRAY));
        let title = format(format_args!(
            "REMOVE {} [{:02}]",
            upper(removal.removed_name.as_str()),
            removal.removed
        ));
        list.text(parts::text(
            &fitted(show.font, &title, Face::Kh, 23, COLUMN),
            88,
            top + 25,
            Face::Kh,
            23,
            chrome::ORANGE,
        ));
        list.text(parts::text(
            &format(format_args!(
                "REQUESTED BY {} [{:02}]",
                upper(removal.remover_name.as_str()),
                removal.remover
            )),
            88,
            top + 64,
            Face::Mono,
            12,
            chrome::VIOLET,
        ));
        list.fill(
            rect(82, top + RIVAL_ROW, 384, top + RIVAL_ROW + 1),
            chrome::TRACK,
        );
    }
    parts::button(list, FOOTER, "VIEW REQUEST", true);
}

/// The rival row `y` falls on.
#[must_use]
pub fn rival_at(y: i32) -> Option<usize> {
    RIVAL_TOPS
        .iter()
        .position(|&top| (top..top + RIVAL_ROW).contains(&y))
}

/// Another member removed this device: who, that it stays in its old group, and that leaving is
/// its user's choice.
pub fn removed(list: &mut List, by: u8, name: &Name, show: &Show) {
    parts::back(list);
    parts::title(list, "REMOVED", show.font);
    parts::meta(list, "GROUP CHANGE");
    list.outline(rect(215, 130, 251, 166), chrome::ORANGE);
    list.fill(rect(223, 147, 243, 150), chrome::ORANGE);
    let by = show.naming(
        format(format_args!(
            "You were removed by {} [{by:02}].",
            name.as_str()
        )),
        &format(format_args!("You were removed by member [{by:02}].")),
        17,
        COLUMN,
    );
    lines(
        list,
        &[
            &by,
            "This device stays in the old group.",
            "New group updates are unavailable.",
        ],
        198,
        17,
    );
    centred(
        list,
        "Leaving is your choice.",
        303,
        Face::Sans,
        17,
        chrome::GRAY,
    );
    parts::button(list, PAIR[0], "CLOSE", true);
    parts::consequential(list, PAIR[1], "LEAVE GROUP");
}
