//! The keyboard: every printable ASCII character over four views, a draft edited at a caret, and
//! CANCEL, DELETE and SAVE for a name, or REVIEW for a message. A key acts when the finger lifts
//! inside the key it came down on. Its geometry is the pairing hand-off's `KEYBOARD-VIEWS.json`,
//! and a message's field and buttons the 2026-10-04 hand-off's.

use embedded_graphics::{prelude::Point, primitives::Rectangle};
use heapless::Vec;

use super::{
    layout::{Align, Face, List, Text, Vertical, format, rect},
    view::{NAME_LEN, Name, TEXT_MAX, Text as Message},
};
use crate::{
    chrome::{self, Color, FontdueRenderer},
    ui::{
        gesture::GestureEvent,
        text::{style, wrap},
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Upper,
    Lower,
    Numbers,
    Symbols,
}

impl Mode {
    fn caption(self) -> &'static str {
        match self {
            Self::Upper => "ABC",
            Self::Lower => "abc",
            Self::Numbers => "123",
            Self::Symbols => "SYMBOLS",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Action {
    Char(u8),
    Delete,
    Cancel,
    /// SAVE for a name, REVIEW for a message.
    Save,
    Mode(Mode),
    /// Shows the current case, and switches to the other.
    Case,
}

/// What the draft is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Field {
    /// A name, from the saved one.
    Name(Name),
    Message,
}

impl Field {
    fn capacity(self) -> usize {
        match self {
            Self::Name(_) => NAME_LEN,
            Self::Message => TEXT_MAX,
        }
    }

    /// Where a tap moves the caret.
    fn area(self) -> Rectangle {
        match self {
            Self::Name(_) => FIELD,
            Self::Message => MESSAGE_FIELD,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Key {
    rect: Rectangle,
    label: &'static str,
    action: Action,
    size: u8,
}

const PRINTABLE: &str = " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~";
const ROWS: [i32; 3] = [160, 215, 270];
const KEY_WIDTH: i32 = 39;
const KEY_HEIGHT: i32 = 53;
/// The name field, which a tap moves the caret in.
const FIELD: Rectangle = rect(78, 75, 388, 106);
/// A message's field: two lines that follow the caret.
const MESSAGE_FIELD: Rectangle = rect(78, 64, 388, 107);
const MESSAGE_LEFT: i32 = 88;
const MESSAGE_WIDTH: f32 = 290.0;
const MESSAGE_TOPS: [i32; 2] = [70, 89];
const MESSAGE_SIZE: u8 = 14;
const NAME_TOP: i32 = 82;
const CARET_TOP: i32 = 102;
const CENTRE: i32 = 233;
const KEYS: usize = 36;

fn label(c: u8) -> &'static str {
    let at = usize::from(c - b' ');
    &PRINTABLE[at..=at]
}

fn row(keys: &mut Vec<Key, KEYS>, chars: &str, left: i32, top: i32) {
    for (i, c) in chars.bytes().enumerate() {
        let x = left + KEY_WIDTH * i as i32;
        _ = keys.push(Key {
            rect: rect(x, top, x + KEY_WIDTH, top + KEY_HEIGHT),
            label: label(c),
            action: Action::Char(c),
            size: 23,
        });
    }
}

fn keys(mode: Mode, field: Field) -> Vec<Key, KEYS> {
    let mut keys = Vec::new();
    let key = |rect, label, action, size| Key {
        rect,
        label,
        action,
        size,
    };
    let [first, second, third] = ROWS;
    match mode {
        Mode::Upper | Mode::Lower => {
            let upper = mode == Mode::Upper;
            let (top, middle, bottom) = if upper {
                ("QWERTYUIOP", "ASDFGHJKL", "ZXCVBNM")
            } else {
                ("qwertyuiop", "asdfghjkl", "zxcvbnm")
            };
            row(&mut keys, top, 38, first);
            // The design's half-pixel offset, rounded.
            row(&mut keys, middle, 58, second);
            _ = keys.push(key(
                rect(38, third, 77, third + KEY_HEIGHT),
                if upper { "A" } else { "a" },
                Action::Case,
                15,
            ));
            row(&mut keys, bottom, 77, third);
            _ = keys.push(key(
                rect(350, third, 428, third + KEY_HEIGHT),
                "DEL",
                Action::Delete,
                14,
            ));
        }
        Mode::Numbers => {
            row(&mut keys, "0123456789", 38, first);
            row(&mut keys, "!\"#$%&'()*", 38, second);
            row(&mut keys, "+,-./:;<=>", 38, third);
        }
        Mode::Symbols => {
            row(&mut keys, "?@[\\]^_`{|", 38, first);
            row(&mut keys, "}~", 194, second);
        }
    }
    match field {
        Field::Name(_) => {
            _ = keys.push(key(rect(84, 112, 182, 150), "CANCEL", Action::Cancel, 15));
            _ = keys.push(key(rect(184, 112, 282, 150), "DELETE", Action::Delete, 14));
            _ = keys.push(key(rect(284, 112, 382, 150), "SAVE", Action::Save, 18));
        }
        Field::Message => {
            _ = keys.push(key(rect(86, 114, 180, 148), "CANCEL", Action::Cancel, 12));
            _ = keys.push(key(rect(186, 114, 280, 148), "DELETE", Action::Delete, 12));
            _ = keys.push(key(rect(286, 114, 380, 148), "REVIEW", Action::Save, 12));
        }
    }
    let (left, right) = match mode {
        Mode::Upper | Mode::Lower => (
            ("123", Action::Mode(Mode::Numbers), 15),
            ("+ / #", Action::Mode(Mode::Symbols), 13),
        ),
        Mode::Numbers => (
            ("ABC", Action::Mode(Mode::Upper), 15),
            ("+ / #", Action::Mode(Mode::Symbols), 13),
        ),
        Mode::Symbols => (
            ("ABC", Action::Mode(Mode::Upper), 15),
            ("123", Action::Mode(Mode::Numbers), 13),
        ),
    };
    _ = keys.push(key(rect(88, 327, 153, 380), left.0, left.1, left.2));
    _ = keys.push(key(
        rect(155, 327, 309, 380),
        "SPACE",
        Action::Char(b' '),
        17,
    ));
    _ = keys.push(key(rect(311, 327, 378, 380), right.0, right.1, right.2));
    keys
}

/// Where to tap to type `text` on a message keyboard showing `mode`, switching modes as it goes,
/// for scripts and tests. A character no key types is left out.
#[must_use]
pub fn taps(text: &str, mut mode: Mode) -> heapless::Vec<Point, 512> {
    let find = |mode: Mode, action: Action| {
        keys(mode, Field::Message)
            .into_iter()
            .find(|key| key.action == action)
            .map(|key| key.rect.center())
    };
    let mut taps = heapless::Vec::new();
    for c in text.bytes() {
        let Some(target) = [Mode::Lower, Mode::Upper, Mode::Numbers, Mode::Symbols]
            .into_iter()
            .find(|&each| find(each, Action::Char(c)).is_some())
        else {
            continue;
        };
        if find(mode, Action::Char(c)).is_none() {
            while mode != target {
                let (action, next) = match (mode, target) {
                    (Mode::Upper | Mode::Lower, Mode::Numbers | Mode::Symbols) => {
                        (Action::Mode(target), target)
                    }
                    (Mode::Numbers, Mode::Symbols) | (Mode::Symbols, Mode::Numbers) => {
                        (Action::Mode(target), target)
                    }
                    (Mode::Numbers | Mode::Symbols, _) => (Action::Mode(Mode::Upper), Mode::Upper),
                    (Mode::Upper, _) => (Action::Case, Mode::Lower),
                    (Mode::Lower, _) => (Action::Case, Mode::Upper),
                };
                if let Some(point) = find(mode, action) {
                    _ = taps.push(point);
                }
                mode = next;
            }
        }
        if let Some(point) = find(mode, Action::Char(c)) {
            _ = taps.push(point);
        }
    }
    taps
}

/// What the line under the keys says instead of the count.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Note {
    Full,
    Blank,
    Saving,
}

/// What a gesture on the keyboard asks of the screen around it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    Stay,
    Cancel,
    /// Store this name. The keyboard waits for the answer until told.
    Save(Name),
    /// SAVE with the saved name unchanged, which needs no write.
    Unchanged,
    /// REVIEW: this message, to read through before it is sent.
    Review(Message),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Keyboard {
    field: Field,
    draft: Vec<u8, TEXT_MAX>,
    /// The boundary the next character goes in at, 0 to the draft's length.
    cursor: usize,
    mode: Mode,
    /// The key a finger came down on, in this mode, and whether the finger is still inside it.
    held: Option<(usize, bool)>,
    note: Option<Note>,
}

impl Keyboard {
    /// Edits `name`, with the caret at its end.
    #[must_use]
    pub fn new(name: Name) -> Self {
        let draft = Vec::from_slice(name.as_bytes()).unwrap_or_default();
        Self {
            field: Field::Name(name),
            cursor: draft.len(),
            draft,
            mode: Mode::Upper,
            held: None,
            note: None,
        }
    }

    /// An empty message, in lower case.
    #[must_use]
    pub fn message() -> Self {
        Self {
            field: Field::Message,
            draft: Vec::new(),
            cursor: 0,
            mode: Mode::Lower,
            held: None,
            note: None,
        }
    }

    /// Shows that the draft is being saved, and holds the keys until [`Keyboard::resume`].
    pub fn saving(&mut self) {
        self.note = Some(Note::Saving);
        self.held = None;
    }

    pub fn resume(&mut self) {
        self.note = None;
    }

    #[must_use]
    fn is_saving(&self) -> bool {
        self.note == Some(Note::Saving)
    }

    pub fn handle(
        &mut self,
        event: &GestureEvent,
        font: &FontdueRenderer<'static, Color>,
    ) -> Outcome {
        if self.is_saving() {
            return Outcome::Stay;
        }
        let keys = keys(self.mode, self.field);
        let under = |point: Point| keys.iter().position(|key| key.rect.contains(point));
        match *event {
            GestureEvent::Down(point) => {
                self.held = under(point).map(|index| (index, true));
            }
            GestureEvent::DragStart(drag) | GestureEvent::DragMove(drag) => {
                if let Some((index, inside)) = &mut self.held {
                    *inside = keys[*index].rect.contains(drag.current);
                }
            }
            GestureEvent::DragEnd(drag) => {
                if let Some((index, _)) = self.held.take()
                    && keys[index].rect.contains(drag.current)
                {
                    return self.act(keys[index].action);
                }
            }
            GestureEvent::Tap(point) => {
                if let Some((index, _)) = self.held.take()
                    && keys[index].rect.contains(point)
                {
                    return self.act(keys[index].action);
                }
                if self.field.area().contains(point) {
                    self.cursor = self.boundary_at(point, font);
                }
            }
            GestureEvent::None => {}
        }
        Outcome::Stay
    }

    fn act(&mut self, action: Action) -> Outcome {
        self.note = None;
        match action {
            Action::Char(c) => {
                if self.draft.len() < self.field.capacity()
                    && self.draft.insert(self.cursor, c).is_ok()
                {
                    self.cursor += 1;
                } else {
                    self.note = Some(Note::Full);
                }
            }
            Action::Delete => {
                if self.cursor > 0 {
                    self.draft.remove(self.cursor - 1);
                    self.cursor -= 1;
                }
            }
            Action::Cancel => return Outcome::Cancel,
            Action::Save => {
                let end = self
                    .draft
                    .iter()
                    .rposition(|&c| c != b' ')
                    .map_or(0, |last| last + 1);
                return match self.field {
                    Field::Name(original) => match Name::new(&self.draft[..end]) {
                        None => {
                            self.note = Some(Note::Blank);
                            Outcome::Stay
                        }
                        Some(name) if name == original => Outcome::Unchanged,
                        Some(name) => Outcome::Save(name),
                    },
                    // REVIEW is unavailable while the draft has nothing but spaces.
                    Field::Message => {
                        Message::new(&self.draft[..end]).map_or(Outcome::Stay, Outcome::Review)
                    }
                };
            }
            Action::Mode(mode) => self.mode = mode,
            Action::Case => {
                self.mode = if self.mode == Mode::Upper {
                    Mode::Lower
                } else {
                    Mode::Upper
                };
            }
        }
        Outcome::Stay
    }

    fn draft(&self) -> &str {
        core::str::from_utf8(&self.draft).unwrap_or_default()
    }

    /// Whether REVIEW has something to show.
    fn reviewable(&self) -> bool {
        self.draft.iter().any(|&c| c != b' ')
    }

    fn name_style(font: &FontdueRenderer<'static, Color>) -> FontdueRenderer<'static, Color> {
        style(font, chrome::WHITE, 18, Face::Mono.index())
    }

    /// Where the draft's pen starts, so that it sits centred on the field.
    fn pen_left(&self, font: &FontdueRenderer<'static, Color>) -> i32 {
        let width = Self::name_style(font).advance(self.draft());
        libm::roundf(CENTRE as f32 - width / 2.0) as i32
    }

    /// The column of the boundary before character `index`.
    fn boundary_x(&self, index: usize, font: &FontdueRenderer<'static, Color>) -> i32 {
        let before = Self::name_style(font).advance(&self.draft()[..index]);
        self.pen_left(font) + libm::roundf(before) as i32
    }

    /// The boundary nearest `point`.
    fn boundary_at(&self, point: Point, font: &FontdueRenderer<'static, Color>) -> usize {
        match self.field {
            Field::Name(_) => (0..=self.draft.len())
                .min_by_key(|&index| (self.boundary_x(index, font) - point.x).abs())
                .unwrap_or(0),
            Field::Message => {
                let style = Self::message_style(font);
                let lines = self.message_lines(&style);
                let first = self.first_line(&lines);
                let row = ((point.y - MESSAGE_FIELD.top_left.y) / 22).clamp(0, 1) as usize;
                let Some(line) = lines.get(first + row) else {
                    return self.draft.len();
                };
                (line.start..=line.end)
                    .min_by_key(|&index| {
                        let x =
                            MESSAGE_LEFT as f32 + style.advance(&self.draft()[line.start..index]);
                        (libm::roundf(x) as i32 - point.x).abs()
                    })
                    .unwrap_or(line.end)
            }
        }
    }

    fn message_style(font: &FontdueRenderer<'static, Color>) -> FontdueRenderer<'static, Color> {
        style(
            font,
            chrome::WHITE,
            u32::from(MESSAGE_SIZE),
            Face::Mono.index(),
        )
    }

    /// The draft's lines, as wrapped in its field. An empty draft has one, empty.
    fn message_lines(
        &self,
        style: &FontdueRenderer<'static, Color>,
    ) -> heapless::Vec<core::ops::Range<usize>, { crate::ui::text::LINES }> {
        let mut lines = wrap(style, self.draft(), MESSAGE_WIDTH);
        if lines.is_empty() {
            _ = lines.push(0..0);
        }
        lines
    }

    /// The line the caret is on: the first that ends at or past it.
    fn caret_line(&self, lines: &[core::ops::Range<usize>]) -> usize {
        lines
            .iter()
            .position(|line| line.end >= self.cursor)
            .unwrap_or(lines.len() - 1)
    }

    /// The first of the two lines the field shows, which keep the caret's in view.
    fn first_line(&self, lines: &[core::ops::Range<usize>]) -> usize {
        self.caret_line(lines).saturating_sub(1)
    }

    /// Draws the keyboard, under `title` for a message.
    pub fn draw(&self, font: &FontdueRenderer<'static, Color>, list: &mut List, title: &str) {
        match self.field {
            Field::Name(_) => self.draw_name(font, list),
            Field::Message => self.draw_message(font, list, title),
        }
        self.draw_keys(list);
    }

    fn draw_message(&self, font: &FontdueRenderer<'static, Color>, list: &mut List, title: &str) {
        list.centred(title, CENTRE, 28, Face::Title, 26, chrome::WHITE);
        list.outline(MESSAGE_FIELD, chrome::GRAY);
        let style = Self::message_style(font);
        let lines = self.message_lines(&style);
        let first = self.first_line(&lines);
        let caret = self.caret_line(&lines);
        for (row, line) in lines.iter().enumerate().skip(first).take(2) {
            let top = MESSAGE_TOPS[row - first];
            let baseline = top + super::super::text::cap(&style);
            let text = &self.draft()[line.clone()];
            if !text.is_empty() {
                list.text(
                    Text::new(text, Face::Mono, MESSAGE_SIZE, chrome::WHITE)
                        .at(MESSAGE_LEFT, top)
                        .align(Align::Pen)
                        .vertical(Vertical::Cap),
                );
            }
            if row == caret {
                let before = &self.draft()[line.start..self.cursor.clamp(line.start, line.end)];
                let x = MESSAGE_LEFT + libm::roundf(style.advance(before)) as i32;
                list.fill(rect(x, baseline + 1, x + 8, baseline + 3), chrome::WHITE);
            }
        }
        let status = match self.note {
            Some(Note::Full) => format(format_args!("{TEXT_MAX}/{TEXT_MAX} / MESSAGE IS FULL")),
            _ => format(format_args!(
                "{:03}/{TEXT_MAX} / REVIEW BEFORE SEND",
                self.draft.len()
            )),
        };
        list.centred(&status, CENTRE, 436, Face::Mono, 10, chrome::GRAY);
        list.centred(
            self.mode.caption(),
            CENTRE,
            400,
            Face::Mono,
            12,
            chrome::VIOLET,
        );
    }

    fn draw_name(&self, font: &FontdueRenderer<'static, Color>, list: &mut List) {
        list.centred("MY NAME", CENTRE, 30, Face::Title, 27, chrome::WHITE);
        list.outline(FIELD, chrome::GRAY);
        if self.draft.is_empty() {
            list.centred("ENTER NAME", CENTRE, NAME_TOP, Face::Mono, 18, chrome::GRAY);
        } else {
            list.text(
                Text::new(self.draft(), Face::Mono, 18, chrome::WHITE)
                    .at(self.pen_left(font), NAME_TOP)
                    .align(Align::Pen)
                    .vertical(Vertical::Cap),
            );
        }
        let caret = if self.draft.is_empty() {
            CENTRE - 1
        } else {
            self.boundary_x(self.cursor, font)
        };
        list.fill(
            rect(caret, CARET_TOP, caret + 2, CARET_TOP + 3),
            chrome::VIOLET,
        );
        list.centred(
            self.mode.caption(),
            CENTRE,
            400,
            Face::Mono,
            12,
            chrome::VIOLET,
        );
        let status = match self.note {
            Some(Note::Full) => format(format_args!("{NAME_LEN}/{NAME_LEN} / NAME IS FULL")),
            Some(Note::Blank) => format(format_args!("A NAME NEEDS A CHARACTER")),
            Some(Note::Saving) => format(format_args!("SAVING")),
            None => format(format_args!(
                "{:02}/{NAME_LEN} / TAP NAME",
                self.draft.len()
            )),
        };
        list.centred(&status, CENTRE, 435, Face::Mono, 11, chrome::GRAY);
    }

    fn draw_keys(&self, list: &mut List) {
        let message = self.field == Field::Message;
        for (index, key) in keys(self.mode, self.field).iter().enumerate() {
            let pressed = self.held == Some((index, true));
            let color = match key.action {
                Action::Case => chrome::VIOLET,
                Action::Save if message && !self.reviewable() => chrome::DISABLED,
                Action::Save if message => chrome::LIME,
                Action::Char(_) | Action::Save => chrome::WHITE,
                Action::Delete | Action::Cancel | Action::Mode(_) => chrome::GRAY,
            };
            let outline = if pressed || key.action == Action::Case {
                chrome::VIOLET
            } else if message && key.action == Action::Save {
                color
            } else {
                chrome::GRAY
            };
            let (fill, ink) = if pressed {
                (chrome::VIOLET, chrome::BLACK)
            } else {
                (chrome::BLACK, color)
            };
            list.boxed(key.rect.offset(-2), outline, fill);
            let Rectangle { top_left, size } = key.rect;
            list.text(
                Text::new(key.label, Face::Mono, key.size, ink)
                    .at(
                        top_left.x + size.width as i32 / 2,
                        top_left.y + size.height as i32 / 2,
                    )
                    .vertical(Vertical::Middle)
                    .on(fill),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_printable_character_has_a_key() {
        let mut found = [false; 95];
        for mode in [Mode::Upper, Mode::Lower, Mode::Numbers, Mode::Symbols] {
            for key in keys(mode, Field::Message) {
                if let Action::Char(c) = key.action {
                    found[usize::from(c - b' ')] = true;
                }
            }
        }
        assert!(found.iter().all(|&found| found));
    }

    #[test]
    fn no_two_keys_overlap() {
        for mode in [Mode::Upper, Mode::Lower, Mode::Numbers, Mode::Symbols] {
            let keys = keys(mode, Field::Message);
            for (i, a) in keys.iter().enumerate() {
                for b in &keys[i + 1..] {
                    assert!(
                        a.rect.intersection(&b.rect).is_zero_sized(),
                        "{} and {} overlap",
                        a.label,
                        b.label
                    );
                }
            }
        }
    }

    /// Presses the key labelled `label`, or the case key for `Aa`.
    fn press(keyboard: &mut Keyboard, label: &str) -> Outcome {
        let key = keys(keyboard.mode, keyboard.field)
            .into_iter()
            .find(|key| match key.action {
                Action::Case => label == "Aa",
                _ => key.label == label,
            })
            .expect("a key with that label");
        let font = crate::chrome::FontdueRenderer::new(
            crate::chrome::FontdueRendererCtx::new_rc(),
            20,
            chrome::WHITE,
            chrome::FONTS,
        );
        let point = key.rect.center();
        keyboard.handle(&GestureEvent::Down(point), &font);
        keyboard.handle(&GestureEvent::Tap(point), &font)
    }

    #[test]
    fn it_types_at_the_caret_and_keeps_case() {
        let mut keyboard = Keyboard::new(Name::new(b"OW-7A2F").unwrap());
        for _ in 0..7 {
            press(&mut keyboard, "DEL");
        }
        press(&mut keyboard, "R");
        press(&mut keyboard, "Aa");
        assert_eq!(keyboard.mode, Mode::Lower);
        for label in ["i", "d", "g", "e"] {
            press(&mut keyboard, label);
        }
        assert_eq!(keyboard.draft(), "Ridge");
        press(&mut keyboard, "123");
        press(&mut keyboard, "0");
        assert_eq!(keyboard.draft(), "Ridge0");
        keyboard.cursor = 1;
        press(&mut keyboard, "DELETE");
        assert_eq!((keyboard.draft(), keyboard.cursor), ("idge0", 0));
    }

    #[test]
    fn a_seventeenth_character_is_refused_with_a_note() {
        let mut keyboard = Keyboard::new(Name::new(b"ABCDEFGHIJKLMNOP").unwrap());
        press(&mut keyboard, "Q");
        assert_eq!(keyboard.draft(), "ABCDEFGHIJKLMNOP");
        assert_eq!(keyboard.note, Some(Note::Full));
    }

    #[test]
    fn saving_trims_trailing_spaces_and_refuses_a_blank_name() {
        let mut keyboard = Keyboard::new(Name::new(b"Ana").unwrap());
        press(&mut keyboard, "SPACE");
        assert_eq!(press(&mut keyboard, "SAVE"), Outcome::Unchanged);
        press(&mut keyboard, "B");
        assert_eq!(
            press(&mut keyboard, "SAVE"),
            Outcome::Save(Name::new(b"Ana B").unwrap())
        );
        let mut keyboard = Keyboard::new(Name::new(b"A").unwrap());
        press(&mut keyboard, "DEL");
        press(&mut keyboard, "SPACE");
        assert_eq!(press(&mut keyboard, "SAVE"), Outcome::Stay);
        assert_eq!(keyboard.note, Some(Note::Blank));
    }

    #[test]
    fn a_release_outside_the_key_does_nothing() {
        let font = crate::chrome::FontdueRenderer::new(
            crate::chrome::FontdueRendererCtx::new_rc(),
            20,
            chrome::WHITE,
            chrome::FONTS,
        );
        let mut keyboard = Keyboard::new(Name::new(b"A").unwrap());
        let q = keys(Mode::Upper, keyboard.field)[0].rect.center();
        let w = keys(Mode::Upper, keyboard.field)[1].rect.center();
        keyboard.handle(&GestureEvent::Down(q), &font);
        let drag = crate::ui::gesture::Drag {
            start: q,
            current: w,
            velocity: (0.0, 0.0),
        };
        keyboard.handle(&GestureEvent::DragMove(drag), &font);
        assert_eq!(keyboard.held, Some((0, false)));
        keyboard.handle(&GestureEvent::DragEnd(drag), &font);
        assert_eq!(keyboard.draft(), "A");
    }
}
