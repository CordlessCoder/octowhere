//! The Messages root and the screens it opens, from the 2026-10-04 hand-off: the conversations,
//! one conversation, the choice of whom to write to, and a draft read through before it is sent.
//! A message's body is never cut short: rows grow with it, and the screens scroll.

use embedded_graphics::{prelude::Point, primitives::Rectangle};
use heapless::Vec;

use super::{
    FIRST, VIEWPORT, VIEWPORT_HEIGHT, empty,
    parts::{self, FOOTER, PAIR},
    rows::{Scaled, age},
};
use crate::{
    chrome::{self, Color, FontdueRenderer},
    ui::{
        gesture::Micros,
        group::{
            layout::{Face, Line, List, format, rect},
            view::{Carriage, GroupView, IDS, MessageView, MessagesView, TEXT_MAX, Thread},
        },
        text::{self, style, wrap},
    },
};

/// Rows of the inbox and of the choice of whom to write to.
const INBOX_ROW: i32 = 95;
const PICKER_ROW: i32 = 81;
/// A message's body: its column, how wide its lines run, its size and their pitch.
const BODY_X: i32 = 88;
const BODY_WIDTH: f32 = 250.0;
const BODY_SIZE: u8 = 18;
const BODY_PITCH: i32 = 23;
/// A message's row, with up to two lines of body.
const MESSAGE_ROW: i32 = 111;
/// How wide an inbox preview's ink may run.
const PREVIEW_WIDTH: f32 = 270.0;
/// The review's body: where it shows, scrolled, its first line and the lines' pitch.
pub const REVIEW_VIEWPORT: Rectangle = rect(80, 140, 386, 284);
const REVIEW_TOP: i32 = 149;
const REVIEW_PITCH: i32 = 25;
/// The widest a title's ink may be at its smallest, past which it is cut short.
const TITLE_WIDTH: u32 = 274;
/// The most conversations: the group's and one with each other member.
const CONVERSATIONS: usize = IDS as usize + 1;

/// What the messages screens show: the messages and the group they come from.
#[derive(Clone, Copy)]
pub struct Mail<'a> {
    pub messages: Option<&'a MessagesView>,
    pub group: Option<&'a GroupView>,
    pub font: &'a FontdueRenderer<'static, Color>,
}

impl Mail<'_> {
    fn own(&self) -> u8 {
        self.group.map_or(0, |group| group.own)
    }

    fn all(&self) -> impl DoubleEndedIterator<Item = &MessageView> {
        self.messages.into_iter().flat_map(MessagesView::iter)
    }

    /// The messages of `thread`, newest first.
    fn newest_first(&self, thread: Thread) -> impl Iterator<Item = &MessageView> {
        let own = self.own();
        self.all()
            .rev()
            .filter(move |message| message.thread(own) == thread)
    }

    /// The name a conversation goes by: GROUP, or the member's, in capitals.
    #[must_use]
    pub fn name(&self, thread: Thread) -> Line {
        match thread {
            Thread::Group => format(format_args!("GROUP")),
            Thread::Member(id) => self.member_name(id),
        }
    }

    fn member_name(&self, id: u8) -> Line {
        match self.group.and_then(|group| group.member(id)) {
            Some(member) => upper(member.name.as_str()),
            None => format(format_args!("MEMBER {id:02}")),
        }
    }

    /// Who may be written to in `thread`: the group while there is one, a member while it is
    /// one.
    #[must_use]
    pub fn writable(&self, thread: Thread) -> bool {
        match thread {
            Thread::Group => self.group.is_some(),
            Thread::Member(id) => self
                .group
                .is_some_and(|group| id != group.own && group.member(id).is_some()),
        }
    }
}

fn upper(text: &str) -> Line {
    let mut line = Line::new();
    for c in text.chars() {
        if line.push(c.to_ascii_uppercase()).is_err() {
            break;
        }
    }
    line
}

/// What a conversation is labelled with above its name.
fn label(thread: Thread) -> Line {
    match thread {
        Thread::Group => format(format_args!("ALL MEMBERS")),
        Thread::Member(id) => format(format_args!("PRIVATE / [{id:02}]")),
    }
}

/// A conversation in the inbox: its newest message, when that was sent, and how many unread.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Conversation {
    pub thread: Thread,
    pub newest: u32,
    pub at: i64,
    pub unread: usize,
}

/// The conversations with messages, newest first.
#[must_use]
pub fn conversations(mail: &Mail) -> Vec<Conversation, CONVERSATIONS> {
    let own = mail.own();
    let mut list: Vec<Conversation, CONVERSATIONS> = Vec::new();
    for message in mail.all() {
        let thread = message.thread(own);
        match list.iter_mut().find(|held| held.thread == thread) {
            Some(held) => {
                held.newest = message.id;
                held.at = message.at;
                held.unread += usize::from(message.unread);
            }
            None => {
                _ = list.push(Conversation {
                    thread,
                    newest: message.id,
                    at: message.at,
                    unread: usize::from(message.unread),
                });
            }
        }
    }
    list.sort_unstable_by_key(|held| core::cmp::Reverse((held.at, held.newest)));
    list
}

/// `content` in `style`, cut short with an ellipsis where its ink runs past `width`.
fn fitted(style: &FontdueRenderer<'static, Color>, content: &str, width: f32) -> Line {
    let mut line = format(format_args!("{content}"));
    if line.len() == content.len() && style.advance(&line) <= width {
        return line;
    }
    while !line.is_empty() {
        line.pop();
        let trimmed = line.trim_end();
        if trimmed.len() + 3 <= line.capacity()
            && style.advance(trimmed) + style.advance("...") <= width
        {
            let mut cut = format(format_args!("{trimmed}"));
            _ = cut.push_str("...");
            return cut;
        }
    }
    line
}

/// A screen's title, cut short where even its smallest size runs too wide; the caption under
/// it names the conversation in full by its id.
fn title(list: &mut List, content: &str, font: &FontdueRenderer<'static, Color>) {
    let smallest = style(font, chrome::WHITE, 26, Face::Title.index());
    parts::title(list, &fitted(&smallest, content, TITLE_WIDTH as f32), font);
}

/// The Messages root: every conversation with messages, newest first, and NEW MESSAGE.
pub fn inbox(list: &mut List, scroll: i32, mail: &Mail, now: Micros) {
    parts::root_top(list, 1);
    parts::title(list, "MESSAGES", mail.font);
    let unread = mail.messages.map_or(0, MessagesView::unread);
    parts::meta(
        list,
        &format(format_args!("{unread:02} UNREAD / GROUP + PRIVATE")),
    );
    let conversations = conversations(mail);
    if conversations.is_empty() {
        empty(list, "NO MESSAGES", "Messages will appear here.");
    } else {
        list.clip(Some(VIEWPORT));
        for (i, conversation) in conversations.iter().enumerate() {
            let top = VIEWPORT.top_left.y + FIRST + i as i32 * INBOX_ROW - scroll;
            if visible(top, INBOX_ROW) {
                inbox_row(list, conversation, top, mail, now);
            }
        }
        list.clip(None);
        parts::scroll_arc(list, inbox_height(&conversations), VIEWPORT_HEIGHT, scroll);
    }
    parts::action(list, FOOTER, "NEW MESSAGE", mail.group.is_some());
}

/// Whether a row at `top`, `height` high, shows in the viewport.
fn visible(top: i32, height: i32) -> bool {
    top < VIEWPORT.top_left.y + VIEWPORT_HEIGHT && top + height > VIEWPORT.top_left.y
}

#[must_use]
pub fn inbox_height(conversations: &[Conversation]) -> i32 {
    FIRST + conversations.len() as i32 * INBOX_ROW
}

fn inbox_row(list: &mut List, conversation: &Conversation, top: i32, mail: &Mail, now: Micros) {
    let scaled = Scaled::new(top, INBOX_ROW);
    let unread = conversation.unread > 0;
    if unread {
        list.fill(scaled.rect(rect(73, top + 25, 79, top + 31)), chrome::WHITE);
    }
    let thread = conversation.thread;
    scaled.text(
        list,
        &label(thread),
        (94, top + 3),
        Face::Mono,
        11,
        chrome::VIOLET,
    );
    let elapsed = (now as i64 - conversation.at).max(0) as Micros;
    list.changes_at(now + super::age_due(elapsed));
    scaled.text(
        list,
        &age(elapsed),
        (340, top + 3),
        Face::Mono,
        11,
        chrome::GRAY,
    );
    scaled.text(
        list,
        &mail.name(thread),
        (94, top + 23),
        Face::Kh,
        23,
        chrome::WHITE,
    );
    if unread {
        scaled.text(
            list,
            &format(format_args!("{:02}", conversation.unread.min(99))),
            (350, top + 28),
            Face::Mono,
            12,
            chrome::WHITE,
        );
    }
    if let Some(newest) = mail
        .messages
        .and_then(|messages| messages.get(conversation.newest))
    {
        let own = mail.own();
        let preview = if newest.from == own {
            format(format_args!("You: {}", newest.text()))
        } else if thread == Thread::Group {
            match mail.group.and_then(|group| group.member(newest.from)) {
                Some(member) => format(format_args!("{}: {}", member.name.as_str(), newest.text())),
                None => format(format_args!("{}", newest.text())),
            }
        } else {
            format(format_args!("{}", newest.text()))
        };
        let preview_style = style(mail.font, chrome::WHITE, 15, Face::Sans.index());
        let ink = if unread { chrome::WHITE } else { chrome::GRAY };
        scaled.text(
            list,
            &fitted(&preview_style, &preview, PREVIEW_WIDTH),
            (94, top + 55),
            Face::Sans,
            15,
            ink,
        );
    }
    list.fill(
        scaled.rect(rect(82, top + 85, 384, top + 86)),
        chrome::TRACK,
    );
}

/// The conversation whose inbox row `point` falls on.
#[must_use]
pub fn inbox_row_at(point: Point, scroll: i32, mail: &Mail) -> Option<Thread> {
    if !VIEWPORT.contains(point) {
        return None;
    }
    let y = point.y - VIEWPORT.top_left.y - FIRST + scroll;
    (y >= 0)
        .then(|| {
            conversations(mail)
                .get((y / INBOX_ROW) as usize)
                .map(|held| held.thread)
        })
        .flatten()
}

/// A message's body, broken into lines as its row shows it.
fn lines(mail: &Mail, message: &MessageView) -> Vec<core::ops::Range<usize>, { text::LINES }> {
    let body = style(
        mail.font,
        chrome::WHITE,
        u32::from(BODY_SIZE),
        Face::Sans.index(),
    );
    wrap(&body, message.text(), BODY_WIDTH)
}

/// How far below a message's first line its state sits, by how many lines its body takes.
fn below(lines: usize) -> i32 {
    BODY_PITCH * (lines as i32 - 2).max(0)
}

fn message_height(lines: usize) -> i32 {
    MESSAGE_ROW + below(lines)
}

/// Each message of `thread`, newest first, with its top in the list and its height.
fn thread_rows<'m>(
    thread: Thread,
    mail: &'m Mail,
) -> impl Iterator<Item = (&'m MessageView, i32, i32)> {
    let mut top = FIRST;
    mail.newest_first(thread).map(move |message| {
        let height = message_height(lines(mail, message).len());
        let row = (message, top, height);
        top += height;
        row
    })
}

/// How tall `thread`'s list is.
#[must_use]
pub fn thread_height(thread: Thread, mail: &Mail) -> i32 {
    thread_rows(thread, mail)
        .last()
        .map_or(0, |(_, top, height)| top + height)
}

/// The unread messages of `thread` whose rows show whole at `scroll`.
pub fn shown_unread(thread: Thread, scroll: i32, mail: &Mail) -> Vec<u32, 8> {
    thread_rows(thread, mail)
        .filter(|(message, top, height)| {
            let top = VIEWPORT.top_left.y + top - scroll;
            message.unread
                && top >= VIEWPORT.top_left.y
                && top + height <= VIEWPORT.top_left.y + VIEWPORT_HEIGHT
        })
        .map(|(message, ..)| message.id)
        .take(8)
        .collect()
}

/// The place in `thread`'s list of the message `id`, its top, if it is there.
#[must_use]
pub fn message_top(thread: Thread, id: u32, mail: &Mail) -> Option<i32> {
    thread_rows(thread, mail)
        .find(|(message, ..)| message.id == id)
        .map(|(_, top, _)| top)
}

/// The first message of `thread` that shows at `scroll`, and its top below the viewport's.
#[must_use]
pub fn thread_anchor(thread: Thread, scroll: i32, mail: &Mail) -> Option<(u32, i32)> {
    thread_rows(thread, mail)
        .find(|&(_, top, height)| top + height > scroll)
        .map(|(message, top, _)| (message.id, top - scroll))
}

/// One conversation: its messages, newest first, each with its author, age, whole body and how
/// far it has gone, and WRITE.
pub fn conversation(list: &mut List, thread: Thread, scroll: i32, mail: &Mail, now: Micros) {
    parts::back(list);
    title(list, &mail.name(thread), mail.font);
    let unread = mail
        .newest_first(thread)
        .filter(|message| message.unread)
        .count();
    let meta = match (thread, unread) {
        (Thread::Group, unread) => format(format_args!("ALL MEMBERS / {unread:02} UNREAD")),
        (Thread::Member(id), 0) => format(format_args!("PRIVATE / [{id:02}]")),
        (Thread::Member(id), unread) => {
            format(format_args!("PRIVATE / [{id:02}] / {unread:02} UNREAD"))
        }
    };
    parts::meta(list, &meta);
    if mail.newest_first(thread).next().is_none() {
        empty(list, "NO MESSAGES", "Write the first one.");
    } else {
        list.clip(Some(VIEWPORT));
        for (message, top, height) in thread_rows(thread, mail) {
            let top = VIEWPORT.top_left.y + top - scroll;
            if visible(top, height) {
                message_row(list, message, top, height, mail, now);
            }
        }
        list.clip(None);
        parts::scroll_arc(list, thread_height(thread, mail), VIEWPORT_HEIGHT, scroll);
    }
    parts::action(list, FOOTER, "WRITE", mail.writable(thread));
}

/// What a message's row says of how far it has gone.
fn carriage(message: &MessageView) -> &'static str {
    match message.carriage {
        Carriage::Queued => "QUEUED",
        Carriage::Sent => "SENT",
        Carriage::Relayed => "HEARD RELAYED",
        Carriage::Delivered => "DELIVERED",
        Carriage::Received if message.recovered => "RECOVERED",
        Carriage::Received => "RECEIVED",
    }
}

fn message_row(
    list: &mut List,
    message: &MessageView,
    top: i32,
    height: i32,
    mail: &Mail,
    now: Micros,
) {
    let scaled = Scaled::new(top, height);
    if message.unread {
        list.fill(scaled.rect(rect(73, top + 6, 79, top + 12)), chrome::WHITE);
    }
    if message.from == mail.own() {
        scaled.text(list, "YOU", (88, top + 3), Face::Mono, 12, chrome::GRAY);
    } else {
        scaled.text(
            list,
            &mail.member_name(message.from),
            (88, top + 3),
            Face::Mono,
            12,
            chrome::VIOLET,
        );
    }
    let elapsed = (now as i64 - message.at).max(0) as Micros;
    list.changes_at(now + super::age_due(elapsed));
    scaled.text(
        list,
        &format(format_args!("{} AGO", age(elapsed))),
        (338, top + 3),
        Face::Mono,
        11,
        chrome::GRAY,
    );
    let lines = lines(mail, message);
    for (i, line) in lines.iter().enumerate() {
        scaled.text(
            list,
            &message.text()[line.clone()],
            (BODY_X, top + 27 + i as i32 * BODY_PITCH),
            Face::Sans,
            BODY_SIZE,
            chrome::WHITE,
        );
    }
    let extra = below(lines.len());
    scaled.text(
        list,
        carriage(message),
        (88, top + 80 + extra),
        Face::Mono,
        10,
        chrome::GRAY,
    );
    list.fill(
        scaled.rect(rect(82, top + 103 + extra, 384, top + 104 + extra)),
        chrome::TRACK,
    );
}

/// Whom a new message may go to: the group, then each other member by id.
#[must_use]
pub fn destinations(mail: &Mail) -> Vec<Thread, CONVERSATIONS> {
    let mut list = Vec::new();
    if let Some(group) = mail.group {
        _ = list.push(Thread::Group);
        for (id, _) in group.members().filter(|&(id, _)| id != group.own) {
            _ = list.push(Thread::Member(id));
        }
    }
    list
}

#[must_use]
pub fn picker_height(mail: &Mail) -> i32 {
    FIRST + destinations(mail).len() as i32 * PICKER_ROW
}

/// SEND TO: the group, or one member, each by name and id, and BACK.
pub fn picker(list: &mut List, scroll: i32, mail: &Mail) {
    parts::back(list);
    parts::title(list, "SEND TO", mail.font);
    parts::meta(list, "GROUP OR ONE MEMBER");
    list.clip(Some(VIEWPORT));
    for (i, &thread) in destinations(mail).iter().enumerate() {
        let top = VIEWPORT.top_left.y + FIRST + i as i32 * PICKER_ROW - scroll;
        if visible(top, PICKER_ROW) {
            let scaled = Scaled::new(top, PICKER_ROW);
            scaled.text(
                list,
                &label(thread),
                (94, top + 3),
                Face::Mono,
                11,
                chrome::VIOLET,
            );
            scaled.text(
                list,
                &mail.name(thread),
                (94, top + 23),
                Face::Kh,
                23,
                chrome::WHITE,
            );
            list.fill(
                scaled.rect(rect(82, top + 71, 384, top + 72)),
                chrome::TRACK,
            );
        }
    }
    list.clip(None);
    parts::scroll_arc(list, picker_height(mail), VIEWPORT_HEIGHT, scroll);
    parts::button(list, FOOTER, "BACK", true);
}

/// The destination whose row `point` falls on.
#[must_use]
pub fn picker_row_at(point: Point, scroll: i32, mail: &Mail) -> Option<Thread> {
    if !VIEWPORT.contains(point) {
        return None;
    }
    let y = point.y - VIEWPORT.top_left.y - FIRST + scroll;
    (y >= 0)
        .then(|| destinations(mail).get((y / PICKER_ROW) as usize).copied())
        .flatten()
}

/// What the draft's title says: whom it goes to.
#[must_use]
pub fn draft_title(thread: Thread, mail: &Mail) -> Line {
    let mut title = format(format_args!("TO {}", mail.name(thread)));
    let widest = style(mail.font, chrome::WHITE, 26, Face::Title.index());
    if widest.advance(&title) > TITLE_WIDTH as f32 {
        title = fitted(&widest, &title, TITLE_WIDTH as f32);
    }
    title
}

fn review_lines(text: &str, mail: &Mail) -> Vec<core::ops::Range<usize>, { text::LINES }> {
    let body = style(
        mail.font,
        chrome::WHITE,
        u32::from(BODY_SIZE),
        Face::Sans.index(),
    );
    wrap(&body, text, BODY_WIDTH)
}

/// How far the review's body scrolls at most.
#[must_use]
pub fn review_scroll(text: &str, mail: &Mail) -> i32 {
    let lines = review_lines(text, mail).len() as i32;
    let content = REVIEW_TOP - REVIEW_VIEWPORT.top_left.y + lines * REVIEW_PITCH;
    (content - REVIEW_VIEWPORT.size.height as i32).max(0)
}

/// The draft to read through before it is sent: whom it goes to, its whole text, scrolled,
/// EDIT and SEND.
pub fn review(list: &mut List, thread: Thread, text: &str, scroll: i32, mail: &Mail) {
    parts::back(list);
    parts::title(list, "REVIEW", mail.font);
    let name = mail.name(thread);
    let (meta, caption) = match thread {
        Thread::Group => (
            format(format_args!("TO GROUP / ALL MEMBERS")),
            format(format_args!("TO ALL MEMBERS")),
        ),
        Thread::Member(_) => (
            format(format_args!("TO {name} / PRIVATE")),
            format(format_args!("PRIVATE TO {name}")),
        ),
    };
    parts::meta(list, &meta);
    list.clip(Some(REVIEW_VIEWPORT));
    for (i, line) in review_lines(text, mail).iter().enumerate() {
        list.text(parts::text(
            &text[line.clone()],
            BODY_X,
            REVIEW_TOP + i as i32 * REVIEW_PITCH - scroll,
            Face::Sans,
            BODY_SIZE,
            chrome::WHITE,
        ));
    }
    list.clip(None);
    list.text(parts::centred(
        &format(format_args!("{:03} / {TEXT_MAX} CHARACTERS", text.len())),
        parts::CENTRE,
        291,
        Face::Mono,
        11,
        chrome::GRAY,
    ));
    list.text(parts::centred(
        "Send waits for your radio slot.",
        parts::CENTRE,
        333,
        Face::Sans,
        15,
        chrome::GRAY,
    ));
    parts::button(list, PAIR[0], "EDIT", true);
    parts::action(list, PAIR[1], "SEND", mail.writable(thread));
    list.text(parts::centred(
        &caption,
        parts::CENTRE,
        430,
        Face::Mono,
        11,
        chrome::GRAY,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome::FontdueRendererCtx;

    #[test]
    fn a_long_preview_is_cut_with_an_ellipsis() {
        let font = FontdueRenderer::new(
            FontdueRendererCtx::new_rc(),
            20,
            chrome::WHITE,
            chrome::FONTS,
        );
        let preview = style(&font, chrome::WHITE, 15, Face::Sans.index());
        let cut = fitted(
            &preview,
            "Take the north path. I will wait at the turn.",
            PREVIEW_WIDTH,
        );
        assert!(cut.ends_with("..."), "{cut}");
        assert!(preview.advance(&cut) <= PREVIEW_WIDTH);
    }
}
