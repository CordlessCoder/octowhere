//! The Events list's rows and an event's detail, from the 2026-10-04 hand-off. A row shows its
//! state and age, a status square, its title, two lines of explanation and where a tap leads;
//! near the list's edges the whole row shrinks a little about its centre.

use embedded_graphics::primitives::Rectangle;

use super::{
    Context,
    parts::{self, FOOTER, FOOTER_HIGH, PAIR, Symbol},
    removals,
};
use crate::{
    chrome::{self, Color},
    ui::{
        events::{Event, FAULT_RESETS, Gnss, Kind, elapsed},
        gesture::Micros,
        group::{
            layout::{Align, Face, Line, List, format, rect},
            view::{RefreshPhase, RefreshView, Thread},
        },
        text,
    },
};

/// A settled row's height, and a running operation's, which adds its time rail.
const ROW: i32 = 178;
const ONGOING_ROW: i32 = 210;
/// The list's middle row, where rows show full size, and the edge scaling's law: full size
/// within `SCALE_NEAR` of it, shrinking by up to `SCALE_DEPTH` by `SCALE_NEAR + SCALE_OVER`.
const MIDDLE: f32 = 265.5;
const SCALE_NEAR: f32 = 65.0;
const SCALE_OVER: f32 = 80.0;
const SCALE_DEPTH: f32 = 0.08;
const SECOND: Micros = 1_000_000;

#[must_use]
pub fn height(event: &Event) -> i32 {
    if event.ongoing() { ONGOING_ROW } else { ROW }
}

/// How an event reads, in its row, its toast and its detail.
pub struct Look {
    /// The state over the title, and its colour.
    pub state: &'static str,
    pub state_color: Color,
    pub title: Line,
    pub color: Color,
    pub symbol: Symbol,
    pub lines: [Line; 2],
    /// The one line a toast shows of it.
    pub toast: Line,
    /// Where a tap leads.
    pub hint: &'static str,
    /// A running operation's time so far and its whole time.
    pub rail: Option<(Micros, Micros)>,
}

fn plural(count: u32, one: &'static str, many: &'static str) -> &'static str {
    if count == 1 { one } else { many }
}

#[must_use]
pub fn look(event: &Event, context: &Context) -> Look {
    match event.kind {
        Kind::Gnss(Gnss::Recovering { attempt }) => Look {
            state: "RECOVERING",
            state_color: chrome::ORANGE,
            title: format(format_args!("GNSS RECOVERING")),
            color: chrome::ORANGE,
            symbol: Symbol::Exchange,
            lines: [
                format(format_args!("Receiver stopped responding.")),
                format(format_args!("Reset attempt {attempt} of {FAULT_RESETS}.")),
            ],
            toast: format(format_args!("Reset attempt {attempt} of {FAULT_RESETS}.")),
            hint: "VIEW RECEIVER",
            rail: None,
        },
        Kind::Gnss(Gnss::Fault { failed }) => Look {
            state: "ACTIVE",
            state_color: chrome::RED,
            title: format(format_args!("GNSS FAULT")),
            color: chrome::RED,
            symbol: Symbol::Alert,
            lines: [
                format(format_args!("Receiver is not responding.")),
                format(format_args!("{failed} resets failed. Retrying.")),
            ],
            toast: format(format_args!("{failed} resets failed. Retrying.")),
            hint: "VIEW RECEIVER",
            rail: None,
        },
        Kind::Gnss(Gnss::Responding) => Look {
            state: "RESOLVED",
            state_color: chrome::GRAY,
            title: format(format_args!("GNSS RESPONDING")),
            color: chrome::WHITE,
            symbol: Symbol::Done,
            lines: [
                format(format_args!("Receiver response restored.")),
                format(format_args!(
                    "{}",
                    if context.gnss.fix {
                        "Its position fix is back."
                    } else {
                        "Still waiting for a position fix."
                    }
                )),
            ],
            toast: format(format_args!(
                "{}",
                if context.gnss.fix {
                    "Receiver is responding. Fix is back."
                } else {
                    "Receiver is responding. No fix yet."
                }
            )),
            hint: "VIEW RECEIVER",
            rail: None,
        },
        Kind::Refresh(refresh) => refresh_look(&refresh, context.now),
        Kind::Removal(removal) => removals::look(&removal, context.own(), context.now),
        Kind::Removed { by, name, .. } => removals::removed_look(by, &name),
        Kind::Messages {
            thread,
            unread,
            newest,
            from,
        } => {
            let mail = context.mail();
            let sender = mail
                .messages
                .and_then(|messages| messages.get(newest))
                .map_or_else(
                    || format(format_args!("MEMBER {from:02}")),
                    |message| mail.sender(message).0,
                );
            let private = matches!(thread, Thread::Member(..));
            Look {
                state: if unread > 0 { "NEW" } else { "READ" },
                state_color: chrome::VIOLET,
                title: mail.name(thread),
                color: chrome::WHITE,
                symbol: Symbol::Envelope,
                lines: [
                    match unread {
                        0 => format(format_args!("No unread messages.")),
                        1 => format(format_args!("1 unread message.")),
                        unread => format(format_args!("{unread} unread messages.")),
                    },
                    if private {
                        format(format_args!("Private conversation."))
                    } else {
                        format(format_args!("Latest from {sender}."))
                    },
                ],
                toast: if private {
                    format(format_args!("New private message."))
                } else {
                    format(format_args!("New message from {sender}."))
                },
                hint: "OPEN CONVERSATION",
                rail: None,
            }
        }
    }
}

fn refresh_look(refresh: &RefreshView, now: Micros) -> Look {
    let heard = refresh.heard.count_ones();
    let learned = refresh.learned.count_ones();
    let heard_line = || {
        if heard == 0 {
            format(format_args!("No devices heard this time."))
        } else {
            format(format_args!(
                "{heard} {} heard",
                plural(heard, "device", "devices")
            ))
        }
    };
    let (state, title, lines, hint) = match refresh.phase {
        RefreshPhase::Listening { .. } => (
            "ONGOING",
            "REFRESHING",
            [
                format(format_args!("Listening for group devices.")),
                format(format_args!(
                    "{heard} heard / {learned} {} learned",
                    plural(learned, "member", "members")
                )),
            ],
            "VIEW REFRESH",
        ),
        RefreshPhase::Ended { .. } => (
            "COMPLETED",
            "REFRESH ENDED",
            [
                heard_line(),
                if learned == 0 {
                    format(format_args!("No new members learned."))
                } else {
                    format(format_args!(
                        "{learned} new {} learned",
                        plural(learned, "member", "members")
                    ))
                },
            ],
            "VIEW RESULTS",
        ),
        RefreshPhase::Interrupted { .. } => (
            "STOPPED",
            "REFRESH STOPPED",
            [
                format(format_args!("A pairing took the radio.")),
                heard_line(),
            ],
            "VIEW RESULTS",
        ),
    };
    let toast = if heard == 0 {
        format(format_args!("No devices heard this time."))
    } else {
        format(format_args!(
            "{heard} heard / {learned} new {} learned",
            plural(learned, "member", "members")
        ))
    };
    Look {
        state,
        state_color: chrome::GRAY,
        title: format(format_args!("{title}")),
        color: chrome::WHITE,
        symbol: Symbol::Exchange,
        lines,
        toast,
        hint,
        rail: elapsed(refresh, now),
    }
}

/// How wide a row's title and lines may run, from their column to the rule's end.
const ROW_WIDTH: f32 = 252.0;

/// `content` at `size` in `face`, cut short with an ellipsis where it runs past `width`, as a
/// long name can make it.
fn fit(context: &Context, content: &str, face: Face, size: u8, width: f32) -> Line {
    parts::fitted(
        &text::style(context.font, chrome::WHITE, u32::from(size), face.index()),
        content,
        width,
    )
}

/// An age in two digits of its largest unit: `11S`, `05M`, `01H`, `02D`.
#[must_use]
pub fn age(elapsed: Micros) -> Line {
    let seconds = elapsed / SECOND;
    match seconds {
        0..60 => format(format_args!("{seconds:02}S")),
        60..3_600 => format(format_args!("{:02}M", seconds / 60)),
        3_600..86_400 => format(format_args!("{:02}H", seconds / 3_600)),
        _ => format(format_args!("{:02}D", seconds / 86_400)),
    }
}

/// When an age shown at `elapsed` next changes, as a time after `elapsed`.
#[must_use]
pub fn age_due(elapsed: Micros) -> Micros {
    let unit = match elapsed / SECOND {
        0..60 => SECOND,
        60..3_600 => 60 * SECOND,
        3_600..86_400 => 3_600 * SECOND,
        _ => 86_400 * SECOND,
    };
    unit - elapsed % unit
}

fn clock(time: Micros) -> Line {
    let seconds = time / SECOND;
    format(format_args!("{:02}:{:02}", seconds / 60, seconds % 60))
}

/// The time an event's row shows by its state: a running operation's time left, or how long
/// ago it last changed. Notes in `list` when it next changes.
fn row_time(event: &Event, look: &Look, now: Micros, list: &mut List) -> Line {
    if let Some((done, whole)) = look.rail {
        list.changes_at(now + SECOND - done % SECOND);
        return format(format_args!("{} LEFT", clock(whole - done)));
    }
    let elapsed = now.saturating_sub(event.at);
    list.changes_at(now + age_due(elapsed));
    if matches!(event.kind, Kind::Refresh(_)) && elapsed < 10 * SECOND {
        return format(format_args!("JUST ENDED"));
    }
    format(format_args!("{} AGO", age(elapsed)))
}

/// Maps a row's coordinates to the screen, shrunk about its centre by where it lies.
pub(super) struct Scaled {
    middle: f32,
    scale: f32,
}

impl Scaled {
    pub(super) fn new(top: i32, height: i32) -> Self {
        let middle = top as f32 + height as f32 / 2.0;
        let q = (((middle - MIDDLE).abs() - SCALE_NEAR) / SCALE_OVER).clamp(0.0, 1.0);
        let q = q * q * (3.0 - 2.0 * q);
        Self {
            middle,
            scale: 1.0 - SCALE_DEPTH * q,
        }
    }

    pub(super) fn x(&self, x: f32) -> f32 {
        parts::CENTRE as f32 + (x - parts::CENTRE as f32) * self.scale
    }

    pub(super) fn y(&self, y: f32) -> f32 {
        self.middle + (y - self.middle) * self.scale
    }

    pub(super) fn point(&self, x: i32, y: i32) -> (i32, i32) {
        (
            libm::roundf(self.x(x as f32)) as i32,
            libm::roundf(self.y(y as f32)) as i32,
        )
    }

    pub(super) fn size(&self, size: u8) -> u8 {
        libm::roundf(f32::from(size) * self.scale).max(1.0) as u8
    }

    pub(super) fn rect(&self, area: Rectangle) -> Rectangle {
        let (x0, y0) = self.point(area.top_left.x, area.top_left.y);
        let corner = area.top_left + area.size;
        let (x1, y1) = self.point(corner.x, corner.y);
        rect(x0, y0, x1.max(x0 + 1), y1.max(y0 + 1))
    }

    /// A point given in quarter pixels.
    pub(super) fn quarter(&self, (x, y): (i16, i16)) -> (i16, i16) {
        (
            libm::roundf(self.x(f32::from(x) / 4.0) * 4.0) as i16,
            libm::roundf(self.y(f32::from(y) / 4.0) * 4.0) as i16,
        )
    }

    pub(super) fn text(
        &self,
        list: &mut List,
        content: &str,
        at: (i32, i32),
        face: Face,
        size: u8,
        color: Color,
    ) {
        let (x, y) = self.point(at.0, at.1);
        list.text(parts::text(content, x, y, face, self.size(size), color));
    }

    /// A path through points in quarter pixels from `origin`, in whole pixels.
    pub(super) fn path(
        &self,
        list: &mut List,
        origin: (i32, i32),
        points: &[(i16, i16)],
        width: u8,
        color: Color,
    ) {
        let mut placed: heapless::Vec<(i16, i16), 6> = heapless::Vec::new();
        for &(x, y) in points.iter().take(6) {
            let point = (origin.0 as i16 * 4 + x, origin.1 as i16 * 4 + y);
            _ = placed.push(self.quarter(point));
        }
        let width = libm::roundf(f32::from(width) * self.scale).max(1.0) as u8;
        list.path(&placed, width, color);
    }
}

/// A status square with its symbol, its corner at `corner`.
fn square(
    list: &mut List,
    scaled: &Scaled,
    corner: (i32, i32),
    side: i32,
    symbol: Symbol,
    color: Color,
) {
    let area = scaled.rect(rect(corner.0, corner.1, corner.0 + side, corner.1 + side));
    list.outline(area, color);
    let k = side as f32 / 24.0;
    let parts::Parts {
        boxes,
        paths,
        width,
    } = symbol.parts();
    for &[x0, y0, x1, y1] in boxes {
        let at = |v: i16| libm::roundf(f32::from(v) / 4.0 * k) as i32;
        list.fill(
            scaled.rect(rect(
                corner.0 + at(x0),
                corner.1 + at(y0),
                corner.0 + at(x1),
                corner.1 + at(y1),
            )),
            color,
        );
    }
    for path in paths {
        let points: heapless::Vec<(i16, i16), 6> = path
            .iter()
            .map(|&(x, y)| {
                (
                    libm::roundf(f32::from(x) * k) as i16,
                    libm::roundf(f32::from(y) * k) as i16,
                )
            })
            .collect();
        scaled.path(list, corner, &points, width, color);
    }
}

/// A small arrow pointing right, whose tip is at (`x`, `y`) in pixels.
fn arrow(list: &mut List, scaled: &Scaled, x: i32, y: i32, color: Color) {
    scaled.path(list, (x, y), &[(-40, 2), (0, 2)], 4, color);
    scaled.path(list, (x, y), &[(-14, -14), (0, 2), (-14, 18)], 4, color);
}

/// An event's row, its top at `top` on the screen.
pub fn row(list: &mut List, event: &Event, top: i32, height: i32, context: &Context) {
    let look = look(event, context);
    let scaled = Scaled::new(top, height);
    let time = row_time(event, &look, context.now, list);
    scaled.text(
        list,
        look.state,
        (118, top + 3),
        Face::Mono,
        11,
        look.state_color,
    );
    scaled.text(list, &time, (329, top + 3), Face::Mono, 11, chrome::GRAY);
    if event.unread {
        list.fill(scaled.rect(rect(70, top + 33, 76, top + 39)), chrome::WHITE);
    }
    square(list, &scaled, (82, top + 24), 24, look.symbol, look.color);
    let title = fit(context, &look.title, Face::Kh, 23, ROW_WIDTH);
    scaled.text(list, &title, (118, top + 25), Face::Kh, 23, look.color);
    for (i, line) in look.lines.iter().enumerate() {
        scaled.text(
            list,
            &fit(context, line, Face::Sans, 16, ROW_WIDTH),
            (118, top + 62 + 23 * i as i32),
            Face::Sans,
            16,
            chrome::WHITE,
        );
    }
    let hint_top = match look.rail {
        Some((done, whole)) => {
            let rail = rect(118, top + 113, 354, top + 117);
            list.fill(scaled.rect(rail), chrome::TRACK);
            let filled = (236 * done / whole.max(1)) as i32;
            if filled > 0 {
                list.fill(
                    scaled.rect(rect(118, top + 113, 118 + filled, top + 117)),
                    chrome::WHITE,
                );
            }
            let caption = format(format_args!("ELAPSED {} / {}", clock(done), clock(whole)));
            scaled.text(
                list,
                &caption,
                (118, top + 127),
                Face::Mono,
                11,
                chrome::GRAY,
            );
            top + 151
        }
        None => top + 119,
    };
    scaled.text(
        list,
        look.hint,
        (118, hint_top),
        Face::Mono,
        12,
        chrome::WHITE,
    );
    arrow(list, &scaled, 354, hint_top + 4, chrome::GRAY);
    let rule = top + height - 19;
    list.fill(scaled.rect(rect(82, rule, 384, rule + 1)), chrome::TRACK);
}

/// Where a toast shows over the faces, and while a keyboard shows.
pub const TOAST: Rectangle = rect(98, 312, 368, 412);
pub const TOAST_COMPACT: Rectangle = rect(140, 22, 326, 65);

/// A toast telling of an event: its state and age, its symbol and title, the line that says
/// what happened, and where a tap leads. A compact toast, while a keyboard shows, keeps only the
/// symbol, the title and where a tap leads.
pub fn toast(list: &mut List, event: &Event, context: &Context, compact: bool) {
    let look = look(event, context);
    let flat = Scaled {
        middle: 0.0,
        scale: 1.0,
    };
    if compact {
        list.boxed(TOAST_COMPACT, look.color, chrome::BLACK);
        square(list, &flat, (151, 31), 18, look.symbol, look.color);
        let title = fit(context, &look.title, Face::Kh, 16, 138.0);
        list.text(parts::text(&title, 180, 32, Face::Kh, 16, look.color));
        list.text(parts::text(
            look.hint,
            180,
            53,
            Face::Mono,
            10,
            chrome::GRAY,
        ));
        return;
    }
    if let Kind::Messages { .. } = event.kind {
        message_toast(list, &look);
        return;
    }
    list.boxed(TOAST, look.color, chrome::BLACK);
    let elapsed = context.now.saturating_sub(event.at);
    let time = if elapsed < 10 * SECOND {
        format(format_args!("NOW"))
    } else {
        format(format_args!("{} AGO", age(elapsed)))
    };
    list.changes_at(context.now + age_due(elapsed));
    list.text(parts::text(
        look.state,
        112,
        323,
        Face::Mono,
        10,
        chrome::GRAY,
    ));
    list.text(parts::text(&time, 354, 323, Face::Mono, 10, chrome::GRAY).align(Align::Right));
    square(list, &flat, (112, 343), 22, look.symbol, look.color);
    let title = fit(context, &look.title, Face::Kh, 21, 212.0);
    list.text(parts::text(&title, 146, 345, Face::Kh, 21, look.color));
    list.text(parts::text(
        &fit(context, &look.toast, Face::Sans, 14, 244.0),
        112,
        376,
        Face::Sans,
        14,
        chrome::WHITE,
    ));
    list.text(parts::text(
        look.hint,
        112,
        394,
        Face::Mono,
        10,
        chrome::GRAY,
    ));
}

/// A message's arrival: an envelope, whose conversation, and for a private one no more, so that
/// its words do not show over the face.
fn message_toast(list: &mut List, look: &Look) {
    list.boxed(TOAST, chrome::WHITE, chrome::BLACK);
    list.path(
        &[
            (446, 1306),
            (522, 1306),
            (522, 1382),
            (446, 1382),
            (446, 1306),
        ],
        4,
        chrome::WHITE,
    );
    list.path(&[(446, 1306), (484, 1340), (522, 1306)], 4, chrome::WHITE);
    list.text(parts::text(
        &format(format_args!("MESSAGE / {}", look.title)),
        143,
        327,
        Face::Kh,
        15,
        chrome::WHITE,
    ));
    list.text(parts::text(
        &look.toast,
        111,
        357,
        Face::Sans,
        16,
        chrome::WHITE,
    ));
    list.text(parts::text(
        "TAP TO OPEN CONVERSATION",
        111,
        387,
        Face::Mono,
        11,
        chrome::GRAY,
    ));
}

/// Where an event's detail puts VIEW MEMBERS, if it has it, and DISMISS. A removal's detail
/// places its own.
#[must_use]
pub fn detail_buttons(event: &Event) -> (Option<Rectangle>, Option<Rectangle>) {
    match event.kind {
        Kind::Refresh(_) => (Some(PAIR[0]), Some(PAIR[1])),
        Kind::Gnss(_) if event.protected().is_some() => (None, Some(FOOTER_HIGH)),
        Kind::Gnss(_) | Kind::Messages { .. } => (None, Some(FOOTER)),
        Kind::Removal(_) | Kind::Removed { .. } => (None, None),
    }
}

/// An event's detail: what it is, what the device knows of it now, and DISMISS, which waits for
/// it to settle. A removal's shows the request as it stands instead.
pub fn detail(list: &mut List, event: &Event, context: &Context) {
    let (category, state) = match event.kind {
        Kind::Removal(removal) => return removals::detail(list, &removal, &context.show()),
        Kind::Removed { by, name, .. } => {
            return removals::removed(list, by, &name, &context.show());
        }
        Kind::Gnss(Gnss::Recovering { .. }) => ("GNSS", "EVENT / RECOVERING"),
        Kind::Gnss(Gnss::Fault { .. }) => ("GNSS", "EVENT / ACTIVE"),
        Kind::Gnss(Gnss::Responding) => ("GNSS", "EVENT / RESOLVED"),
        Kind::Refresh(refresh) => (
            "REFRESH",
            match refresh.phase {
                RefreshPhase::Listening { .. } => "EVENT / ONGOING",
                RefreshPhase::Ended { .. } => "EVENT / COMPLETED",
                RefreshPhase::Interrupted { .. } => "EVENT / STOPPED",
            },
        ),
        Kind::Messages { .. } => ("MESSAGES", "EVENT / NEW"),
    };
    let look = look(event, context);
    let now = context.now;
    parts::back(list);
    parts::title(list, category, context.font);
    parts::meta(list, state);
    list.text(parts::centred(
        &look.title,
        parts::CENTRE,
        145,
        Face::Kh,
        29,
        look.color,
    ));
    let ago = |at: Option<Micros>, suffix: &str| match at {
        Some(at) => format(format_args!("{} {suffix}", age(now.saturating_sub(at)))),
        None => format(format_args!("NEVER")),
    };
    match event.kind {
        Kind::Gnss(phase) => {
            let health = &context.gnss.health;
            let (prose, middle): ([&str; 2], (&str, Line)) = match phase {
                Gnss::Recovering { attempt } => (
                    ["Receiver stopped responding.", "A reset is under way."],
                    (
                        "RESET ATTEMPT",
                        format(format_args!("{attempt:02} / {FAULT_RESETS:02}")),
                    ),
                ),
                Gnss::Fault { failed } => (
                    [
                        "Receiver is not responding.",
                        "Automatic retries are continuing.",
                    ],
                    (
                        "FAILED RESETS",
                        if failed <= FAULT_RESETS {
                            format(format_args!("{failed:02} / {FAULT_RESETS:02}"))
                        } else {
                            format(format_args!("{failed:02}, RETRYING"))
                        },
                    ),
                ),
                Gnss::Responding => (
                    [
                        "Receiver response restored.",
                        if context.gnss.fix {
                            "Its position fix is back."
                        } else {
                            "Still waiting for a position fix."
                        },
                    ],
                    (
                        "SATELLITE FIX",
                        format(format_args!(
                            "{}",
                            if context.gnss.fix { "VALID" } else { "WAITING" }
                        )),
                    ),
                ),
            };
            parts::prose(list, &prose, 199);
            let response = ago(health.last_response, "AGO");
            let fix = ago(health.last_fix, "OLD");
            parts::figures(
                list,
                &[
                    ("LAST RESPONSE", &response),
                    (middle.0, &middle.1),
                    ("LAST FIX", &fix),
                ],
            );
            if event.protected().is_some() {
                parts::button(list, FOOTER_HIGH, "DISMISS", false);
                parts::reason(list, "Available when resolved.");
            } else {
                parts::button(list, FOOTER, "DISMISS", true);
            }
            if let Some(at) = health.last_response.or(health.last_fix) {
                list.changes_at(now + age_due(now.saturating_sub(at)));
            }
        }
        // Opening a messages event opens its conversation instead; this stands in only should
        // a detail ever be asked of one.
        Kind::Messages { .. } => {
            parts::prose(list, &[&look.lines[0], &look.lines[1]], 199);
            parts::button(list, FOOTER, "DISMISS", true);
        }
        // Drawn by their own screens above.
        Kind::Removal(_) | Kind::Removed { .. } => {}
        Kind::Refresh(refresh) => {
            let heard = refresh.heard.count_ones();
            let learned = refresh.learned.count_ones();
            let heard_line = if heard == 0 {
                format(format_args!("No devices were heard."))
            } else {
                format(format_args!(
                    "{heard} {} heard.",
                    plural(heard, "device was", "devices were")
                ))
            };
            let learned_line = if learned == 0 {
                format(format_args!("No new members were learned."))
            } else {
                format(format_args!(
                    "{learned} new {} learned.",
                    plural(learned, "member was", "members were")
                ))
            };
            let (prose, first): ([Line; 2], (&str, Line)) = match refresh.phase {
                RefreshPhase::Listening { .. } => {
                    let (done, whole) = look.rail.unwrap_or((0, 0));
                    list.changes_at(now + SECOND - done % SECOND);
                    (
                        [
                            format(format_args!("Listening for group devices.")),
                            format(format_args!("{heard} heard so far.")),
                        ],
                        ("TIME LEFT", clock(whole - done)),
                    )
                }
                RefreshPhase::Ended { at } => {
                    let at = at.max(0) as Micros;
                    list.changes_at(now + age_due(now.saturating_sub(at)));
                    (
                        [heard_line, learned_line],
                        ("COMPLETED", ago(Some(at), "AGO")),
                    )
                }
                RefreshPhase::Interrupted { at } => {
                    let at = at.max(0) as Micros;
                    list.changes_at(now + age_due(now.saturating_sub(at)));
                    (
                        [
                            format(format_args!("A pairing took the radio.")),
                            heard_line,
                        ],
                        ("STOPPED", ago(Some(at), "AGO")),
                    )
                }
            };
            parts::prose(list, &[&prose[0], &prose[1]], 199);
            let heard = format(format_args!("{heard:02}"));
            let learned = format(format_args!("{learned:02}"));
            parts::figures(
                list,
                &[
                    (first.0, &first.1),
                    ("DEVICES HEARD", &heard),
                    ("MEMBERS LEARNED", &learned),
                ],
            );
            parts::button(list, PAIR[0], "VIEW MEMBERS", true);
            let finished = event.protected().is_none();
            parts::button(list, PAIR[1], "DISMISS", finished);
            if !finished {
                parts::reason(list, "Available when finished.");
            }
        }
    }
}
