//! The Messages root and the screens it opens, from the 2026-10-04 hand-off: the conversations,
//! one conversation, the choice of whom to write to, and a draft read through before it is sent.
//! A message's body is never cut short: rows grow with it, and the screens scroll.

use embedded_graphics::{prelude::Point, primitives::Rectangle};
use heapless::Vec;

use super::{
    FIRST, VIEWPORT, WIDTH, empty,
    parts::{self, CENTRE, FOOTER, PAIR},
    rows::{Scaled, age},
};
use crate::{
    chrome::{self, Color, FontdueRenderer},
    ui::{
        events::CONVERSATIONS,
        gesture::Micros,
        group::{
            layout::{Face, Line, List, format, rect},
            view::{
                Carriage, GroupView, MemberView, MessageView, MessagesView, Name, TEXT_MAX, Thread,
            },
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
const REVIEW_VIEWPORT: Rectangle = rect(80, 140, 386, 284);
const REVIEW_TOP: i32 = 149;
const REVIEW_PITCH: i32 = 25;
/// The widest a title's ink may be at its smallest, past which it is cut short.
const TITLE_WIDTH: u32 = 274;
/// The recipient's whole name and what tells it apart: over the keyboard, and under a title.
const DRAFT_NAME_TOP: i32 = 24;
const DRAFT_IDENTITY_TOP: i32 = 47;
const NAME_TOP: i32 = 93;
const IDENTITY_TOP: i32 = 117;
/// A conversation whose member has gone lists its messages under the member's name and
/// fingerprint, and above why it cannot be written to.
const ENDED_VIEWPORT: Rectangle = rect(0, 142, WIDTH, 370);
const ENDED_REASON_TOP: i32 = 381;
const GONE: &str = "Recipient is no longer a member.";

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
            Thread::Member(id, device) => match self.device_name(device) {
                Some(name) => upper(name.as_str()),
                None => format(format_args!("MEMBER {id:02}")),
            },
        }
    }

    /// The member with `device`, at whichever id it holds now.
    fn member_with(&self, device: [u8; 8]) -> Option<&MemberView> {
        if device == [0; 8] {
            return None;
        }
        let (_, member) = self
            .group?
            .members()
            .find(|(_, member)| member.device == device)?;
        Some(member)
    }

    /// Whether `thread` is with a member whose id no longer holds its device, which a removal
    /// or leaving the group ends. A device given the id later has a conversation of its own.
    #[must_use]
    pub fn ended(&self, thread: Thread) -> bool {
        matches!(thread, Thread::Member(..)) && !self.writable(thread)
    }

    /// `device`'s name: the member's now, or the name it had when its newest message here was
    /// taken.
    fn device_name(&self, device: [u8; 8]) -> Option<Name> {
        if let Some(member) = self.member_with(device) {
            return Some(member.name);
        }
        if device == [0; 8] {
            return None;
        }
        self.all()
            .rev()
            .filter(|message| message.peer == device)
            .find_map(MessageView::peer_name)
    }

    /// Who sent `message`, in capitals for a row, and the colour to set it in: a member's now,
    /// or a member no more, marked removed.
    #[must_use]
    pub fn sender(&self, message: &MessageView) -> (Line, Color) {
        if let Some(member) = self.member_with(message.peer) {
            return (upper(member.name.as_str()), chrome::VIOLET);
        }
        match message.peer_name() {
            Some(name) => (
                format(format_args!("{} / REMOVED", upper(name.as_str()))),
                chrome::GRAY,
            ),
            None => (
                format(format_args!("MEMBER {:02}", message.from)),
                chrome::GRAY,
            ),
        }
    }

    /// Who may be written to in `thread`: the group while there is one, a member while its
    /// device holds its id.
    #[must_use]
    pub fn writable(&self, thread: Thread) -> bool {
        match thread {
            Thread::Group => self.group.is_some(),
            Thread::Member(id, device) => self.group.is_some_and(|group| {
                id != group.own
                    && group
                        .member(id)
                        .is_some_and(|member| member.device == device)
            }),
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
fn label(thread: Thread, mail: &Mail) -> Line {
    match thread {
        Thread::Group => format(format_args!("ALL MEMBERS")),
        Thread::Member(..) if mail.ended(thread) => format(format_args!("PRIVATE / REMOVED")),
        Thread::Member(id, _) => format(format_args!("PRIVATE / [{id:02}]")),
    }
}

/// Whom `thread` is with in full: the member's name with its case, or GROUP.
fn full_name(thread: Thread, mail: &Mail) -> Line {
    match thread {
        Thread::Group => format(format_args!("GROUP")),
        Thread::Member(id, device) => match mail.device_name(device) {
            Some(name) => format(format_args!("{}", name.as_str())),
            None => format(format_args!("MEMBER {id:02}")),
        },
    }
}

/// What tells `thread`'s recipient apart, and its colour: the member's id and its device's
/// fingerprint, which two members of one name do not share.
fn identity(thread: Thread, mail: &Mail) -> (Line, Color) {
    match thread {
        Thread::Group => (format(format_args!("ALL MEMBERS / GROUP")), chrome::VIOLET),
        Thread::Member(_, device) if mail.ended(thread) => (
            format(format_args!("REMOVED / {}", parts::fingerprint(&device))),
            chrome::GRAY,
        ),
        Thread::Member(id, device) => (
            format(format_args!(
                "PRIVATE [{id:02}] / {}",
                parts::fingerprint(&device)
            )),
            chrome::VIOLET,
        ),
    }
}

/// The recipient under a title: its whole name, and what tells it apart.
fn recipient(list: &mut List, thread: Thread, mail: &Mail) {
    list.text(parts::centred(
        &full_name(thread, mail),
        CENTRE,
        NAME_TOP,
        Face::Mono,
        16,
        chrome::WHITE,
    ));
    let (identity, color) = identity(thread, mail);
    list.text(parts::centred(
        &identity,
        CENTRE,
        IDENTITY_TOP,
        Face::Mono,
        11,
        color,
    ));
}

/// Where `thread`'s messages show.
#[must_use]
pub fn viewport(thread: Thread, mail: &Mail) -> Rectangle {
    if mail.ended(thread) {
        ENDED_VIEWPORT
    } else {
        VIEWPORT
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

/// The conversations with messages, newest first, as many as [`CONVERSATIONS`]: every one with
/// unread messages before any without, so that only one past the bound of those can be left out.
#[must_use]
pub fn conversations(mail: &Mail) -> Vec<Conversation, CONVERSATIONS> {
    let own = mail.own();
    let mut list: Vec<Conversation, CONVERSATIONS> = Vec::new();
    for unread_only in [true, false] {
        for message in mail.all().rev() {
            let thread = message.thread(own);
            if (unread_only && !message.unread) || list.iter().any(|held| held.thread == thread) {
                continue;
            }
            let conversation = Conversation {
                thread,
                newest: message.id,
                at: message.at,
                unread: 0,
            };
            if list.push(conversation).is_err() {
                break;
            }
        }
    }
    for message in mail.all() {
        let thread = message.thread(own);
        if let Some(held) = list.iter_mut().find(|held| held.thread == thread) {
            held.newest = message.id;
            held.at = message.at;
            held.unread += usize::from(message.unread);
        }
    }
    list.sort_unstable_by_key(|held| core::cmp::Reverse((held.at, held.newest)));
    list
}

/// A screen's title, cut short where even its smallest size runs too wide; the caption under
/// it names the conversation in full by its id.
fn title(list: &mut List, content: &str, font: &FontdueRenderer<'static, Color>) {
    let smallest = style(font, chrome::WHITE, 26, Face::Title.index());
    parts::title(
        list,
        &parts::fitted(&smallest, content, TITLE_WIDTH as f32),
        font,
    );
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
            if visible(VIEWPORT, top, INBOX_ROW) {
                inbox_row(list, conversation, top, mail, now);
            }
        }
        list.clip(None);
        parts::scroll_arc(
            list,
            inbox_height(&conversations),
            VIEWPORT.size.height as i32,
            scroll,
        );
    }
    parts::action(list, FOOTER, "NEW MESSAGE", mail.group.is_some());
}

/// Whether a row at `top`, `height` high, shows in `viewport`.
fn visible(viewport: Rectangle, top: i32, height: i32) -> bool {
    top < viewport.top_left.y + viewport.size.height as i32 && top + height > viewport.top_left.y
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
        &label(thread, mail),
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
            match mail.device_name(newest.peer) {
                Some(name) => format(format_args!("{}: {}", name.as_str(), newest.text())),
                None => format(format_args!("{}", newest.text())),
            }
        } else {
            format(format_args!("{}", newest.text()))
        };
        let preview_style = style(mail.font, chrome::WHITE, 15, Face::Sans.index());
        let ink = if unread { chrome::WHITE } else { chrome::GRAY };
        scaled.text(
            list,
            &parts::fitted(&preview_style, &preview, PREVIEW_WIDTH),
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

/// What shows of an unread message: its whole row, or one line of a message too tall for the
/// viewport to show whole, by the bytes of its text the line holds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Showing {
    Row(u32),
    Line { id: u32, bytes: (u8, u8) },
}

/// The most of a conversation's unread rows and lines that show at once.
pub const SHOWING: usize = 24;

/// What of `thread`'s unread messages shows whole at `scroll`: each row that fits the viewport
/// and shows whole, and of each row too tall for it, each line of its body whose ink shows
/// whole where the row's edge shrinking puts it.
pub fn showing(thread: Thread, scroll: i32, mail: &Mail) -> Vec<Showing, SHOWING> {
    let viewport = viewport(thread, mail);
    let (above, below) = (
        viewport.top_left.y,
        viewport.top_left.y + viewport.size.height as i32,
    );
    let mut shown = Vec::new();
    for (message, top, height) in thread_rows(thread, mail).filter(|(message, ..)| message.unread) {
        let top = above + top - scroll;
        if !visible(viewport, top, height) {
            continue;
        }
        if height <= below - above {
            if top >= above && top + height <= below {
                _ = shown.push(Showing::Row(message.id));
            }
            continue;
        }
        let scaled = Scaled::new(top, height);
        for (i, line) in lines(mail, message).iter().enumerate() {
            let ink = top + 27 + i as i32 * BODY_PITCH;
            let (from, to) = (
                scaled.y(ink as f32),
                scaled.y((ink + i32::from(BODY_SIZE)) as f32),
            );
            if from >= above as f32 && to <= below as f32 {
                _ = shown.push(Showing::Line {
                    id: message.id,
                    bytes: (line.start as u8, line.end as u8),
                });
            }
        }
    }
    shown
}

/// The most messages too tall to show whole whose lines are read in part at once. Past it a
/// message's lines are not recorded, so it stays unread rather than count as read on too little.
const COVERAGE: usize = 16;
const WORDS: usize = TEXT_MAX.div_ceil(32);

/// Which bytes of each message too tall to show whole have shown on a line for long enough, by
/// the message, for as long as the stage runs. Kept by bytes rather than lines, a rewrap cannot
/// carry what was read to text that has not shown.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Coverage {
    held: Vec<(u32, [u32; WORDS]), COVERAGE>,
}

impl Coverage {
    fn bits(&self, id: u32) -> Option<&[u32; WORDS]> {
        self.held
            .iter()
            .find(|(held, _)| *held == id)
            .map(|(_, bits)| bits)
    }

    /// Whether every byte from `start` to `end` of message `id` has shown.
    #[must_use]
    pub fn covers(&self, id: u32, (start, end): (u8, u8)) -> bool {
        self.bits(id).is_some_and(|bits| {
            (usize::from(start)..usize::from(end)).all(|i| bits[i / 32] >> (i % 32) & 1 == 1)
        })
    }

    /// Notes the bytes from `start` to `end` of message `id` as shown, or says there is no room
    /// for another message.
    pub fn cover(&mut self, id: u32, (start, end): (u8, u8)) -> bool {
        let at = match self.held.iter().position(|(held, _)| *held == id) {
            Some(at) => at,
            None => {
                if self.held.push((id, [0; WORDS])).is_err() {
                    return false;
                }
                self.held.len() - 1
            }
        };
        let bits = &mut self.held[at].1;
        for i in usize::from(start)..usize::from(end).min(TEXT_MAX) {
            bits[i / 32] |= 1 << (i % 32);
        }
        true
    }

    /// Whether every byte of `text`, message `id`'s, but its spaces has shown.
    #[must_use]
    pub fn whole(&self, id: u32, text: &str) -> bool {
        self.bits(id).is_some_and(|bits| {
            text.bytes()
                .enumerate()
                .all(|(i, byte)| byte == b' ' || bits[i / 32] >> (i % 32) & 1 == 1)
        })
    }

    /// Keeps only the messages `keep` takes: those still held and unread.
    pub fn retain(&mut self, mut keep: impl FnMut(u32) -> bool) {
        self.held.retain(|(id, _)| keep(*id));
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.held.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }
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
/// far it has gone, and WRITE. One whose member has gone names it in full, as it was, and says
/// why WRITE is unavailable.
pub fn conversation(list: &mut List, thread: Thread, scroll: i32, mail: &Mail, now: Micros) {
    parts::back(list);
    let ended = mail.ended(thread);
    if ended {
        parts::title(list, "PRIVATE", mail.font);
        recipient(list, thread, mail);
    } else {
        title(list, &mail.name(thread), mail.font);
        let unread = mail
            .newest_first(thread)
            .filter(|message| message.unread)
            .count();
        let meta = match (thread, unread) {
            (Thread::Group, unread) => format(format_args!("ALL MEMBERS / {unread:02} UNREAD")),
            (Thread::Member(..), 0) => label(thread, mail),
            (Thread::Member(..), unread) => {
                format(format_args!("{} / {unread:02} UNREAD", label(thread, mail)))
            }
        };
        parts::meta(list, &meta);
    }
    let viewport = viewport(thread, mail);
    if mail.newest_first(thread).next().is_none() {
        empty(list, "NO MESSAGES", "Write the first one.");
    } else {
        list.clip(Some(viewport));
        for (message, top, height) in thread_rows(thread, mail) {
            let top = viewport.top_left.y + top - scroll;
            if visible(viewport, top, height) {
                message_row(list, message, top, height, mail, now);
            }
        }
        list.clip(None);
        parts::scroll_arc(
            list,
            thread_height(thread, mail),
            viewport.size.height as i32,
            scroll,
        );
    }
    if ended {
        list.text(parts::centred(
            GONE,
            CENTRE,
            ENDED_REASON_TOP,
            Face::Sans,
            14,
            chrome::GRAY,
        ));
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
        let (sender, color) = mail.sender(message);
        scaled.text(list, &sender, (88, top + 3), Face::Mono, 12, color);
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
fn destinations(mail: &Mail) -> Vec<Thread, CONVERSATIONS> {
    let mut list = Vec::new();
    if let Some(group) = mail.group {
        _ = list.push(Thread::Group);
        for (id, member) in group.members().filter(|&(id, _)| id != group.own) {
            _ = list.push(Thread::Member(id, member.device));
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
        if visible(VIEWPORT, top, PICKER_ROW) {
            let scaled = Scaled::new(top, PICKER_ROW);
            scaled.text(
                list,
                &label(thread, mail),
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
    parts::scroll_arc(
        list,
        picker_height(mail),
        VIEWPORT.size.height as i32,
        scroll,
    );
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

/// Whom the draft goes to, over the keyboard: the member's whole name and what tells it apart,
/// or GROUP.
pub fn draft_header(list: &mut List, thread: Thread, mail: &Mail) {
    list.text(match thread {
        Thread::Group => parts::centred(
            "GROUP",
            CENTRE,
            DRAFT_NAME_TOP,
            Face::Title,
            26,
            chrome::WHITE,
        ),
        Thread::Member(..) => parts::centred(
            &full_name(thread, mail),
            CENTRE,
            DRAFT_NAME_TOP,
            Face::Mono,
            18,
            chrome::WHITE,
        ),
    });
    let (identity, color) = identity(thread, mail);
    list.text(parts::centred(
        &identity,
        CENTRE,
        DRAFT_IDENTITY_TOP,
        Face::Mono,
        11,
        color,
    ));
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
/// EDIT and SEND. SEND waits on the recipient still being a member, and says so.
pub fn review(list: &mut List, thread: Thread, text: &str, scroll: i32, mail: &Mail) {
    parts::back(list);
    parts::title(list, "REVIEW", mail.font);
    recipient(list, thread, mail);
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
    parts::button(list, PAIR[0], "EDIT", true);
    parts::action(list, PAIR[1], "SEND", mail.writable(thread));
    if mail.ended(thread) {
        parts::reason(list, GONE);
    } else {
        let caption = match thread {
            Thread::Group => "TO ALL MEMBERS",
            Thread::Member(..) => "PRIVATE MESSAGE",
        };
        list.text(parts::centred(
            caption,
            CENTRE,
            430,
            Face::Mono,
            11,
            chrome::GRAY,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome::FontdueRendererCtx;

    #[test]
    fn coverage_counts_bytes_so_a_rewrap_keeps_what_was_read() {
        let mut coverage = Coverage::default();
        let text = "Take the north path and wait";
        assert!(!coverage.whole(7, text));
        // Read as two lines.
        assert!(coverage.cover(7, (0, 14)));
        assert!(coverage.covers(7, (0, 14)));
        assert!(!coverage.covers(7, (0, 15)));
        assert!(!coverage.whole(7, text));
        assert!(coverage.cover(7, (15, 28)));
        assert!(
            coverage.whole(7, text),
            "the space at the break is not text"
        );
        // Wrapped otherwise, the same bytes count, and another message's do not.
        assert!(coverage.covers(7, (15, 23)));
        assert!(!coverage.covers(8, (0, 1)));
        coverage.retain(|id| id != 7);
        assert!(coverage.is_empty());
    }

    #[test]
    fn coverage_full_refuses_rather_than_count_a_message_read() {
        let mut coverage = Coverage::default();
        for id in 0..COVERAGE as u32 {
            assert!(coverage.cover(id, (0, 1)));
        }
        assert!(!coverage.cover(99, (0, 4)));
        assert!(!coverage.whole(99, "Hi"));
        assert!(coverage.cover(3, (1, 2)), "one held takes more");
    }

    #[test]
    fn a_long_preview_is_cut_with_an_ellipsis() {
        let font = FontdueRenderer::new(
            FontdueRendererCtx::new_rc(),
            20,
            chrome::WHITE,
            chrome::FONTS,
        );
        let preview = style(&font, chrome::WHITE, 15, Face::Sans.index());
        let cut = parts::fitted(
            &preview,
            "Take the north path. I will wait at the turn.",
            PREVIEW_WIDTH,
        );
        assert!(cut.ends_with("..."), "{cut}");
        assert!(preview.advance(&cut) <= PREVIEW_WIDTH);
    }
}
