//! The messages screens, driven through the stage by the gestures and mesh views that reach them
//! on the device, and drawn as the frame loop draws them.

mod common;

use common::{Buffers, differing};
use embedded_graphics::prelude::Point;
use octowhere_ui::{
    chrome::FB,
    ui::{
        drawer::{Child, Root},
        events::Kind,
        group::{
            keyboard::{Mode, taps},
            sim::{self, Arrival, Sim},
            view::{Carriage, MessagesView, Name, Thread},
        },
        rest::Timeout,
        screens::{PeripheralState, Screen},
        script::Driver,
        stage::Stage,
    },
};

const SECOND: u64 = 1_000_000;

/// The clock face, in a group of four, with the design's conversations if `conversations`.
fn start(conversations: bool) -> Driver<'static> {
    let mut driver = Driver::on(Screen::Clock);
    driver.stage = Stage::new(PeripheralState {
        timeout: Timeout::Never,
        ..PeripheralState::default()
    });
    driver.stage.use_messages(Box::leak(MessagesView::boxed()));
    driver.stage.show(Screen::Clock);
    driver.wait(600_000);
    let now = driver.now();
    let mut mesh = Sim::new(Some(sim::group(4, now)));
    if conversations {
        mesh.conversations(now);
    }
    driver.mesh = Some(mesh);
    driver.wait(100_000);
    driver
}

fn open_messages(driver: &mut Driver) {
    driver.swipe(Point::new(233, 430), Point::new(233, 120), 250_000);
    driver.settle();
    driver.wait(300_000);
    driver.swipe(Point::new(380, 260), Point::new(80, 260), 300_000);
    driver.wait(400_000);
}

fn tap(driver: &mut Driver, x: i32, y: i32) {
    driver.tap(Point::new(x, y));
    driver.wait(300_000);
}

fn child(driver: &Driver) -> Option<Child> {
    driver.stage.drawer().and_then(|drawer| drawer.child())
}

fn type_text(driver: &mut Driver, text: &str) {
    for point in taps(text, Mode::Lower) {
        driver.tap(point);
        driver.wait(20_000);
    }
}

fn mesh<'d>(driver: &'d Driver) -> &'d Sim {
    driver.mesh.as_ref().expect("a scripted mesh")
}

/// The conversation with the member at `id`.
fn member(driver: &Driver, id: u8) -> Thread {
    let device = mesh(driver)
        .view()
        .group
        .as_ref()
        .and_then(|group| group.member(id))
        .map_or([0; 8], |member| member.device);
    Thread::Member(id, device)
}

fn own(driver: &Driver) -> u8 {
    mesh(driver)
        .view()
        .group
        .as_ref()
        .map_or(0, |group| group.own)
}

#[test]
fn the_inbox_lists_conversations_and_a_tap_opens_one() {
    let mut driver = start(true);
    open_messages(&mut driver);
    assert_eq!(
        driver.stage.drawer().map(|drawer| drawer.root()),
        Some(Root::Messages)
    );
    // The group's conversation is the newest.
    tap(&mut driver, 233, 170);
    assert_eq!(child(&driver), Some(Child::Thread(Thread::Group)));
    tap(&mut driver, 233, 26);
    assert_eq!(child(&driver), None);
    tap(&mut driver, 233, 265);
    assert_eq!(child(&driver), Some(Child::Thread(member(&driver, 1))));
}

#[test]
fn a_message_counts_as_read_once_it_has_shown_whole_for_a_second() {
    let mut driver = start(true);
    let unread = |driver: &Driver| mesh(driver).messages().unread();
    assert_eq!(unread(&driver), 2);
    open_messages(&mut driver);
    assert_eq!(unread(&driver), 2, "the inbox alone reads nothing");
    tap(&mut driver, 233, 170);
    driver.wait(500_000);
    assert_eq!(unread(&driver), 2, "not yet a second");
    driver.wait(SECOND);
    assert_eq!(unread(&driver), 1, "the group's unread message is read");
    let events = driver.stage.events();
    let group_event = events
        .ordered()
        .find(|event| {
            matches!(
                event.kind,
                Kind::Messages {
                    thread: Thread::Group,
                    ..
                }
            )
        })
        .expect("the group's messages have an event");
    assert!(!group_event.unread, "its event follows");
}

#[test]
fn an_arrival_tells_of_its_conversation_and_its_toast_opens_it() {
    let mut driver = start(false);
    let now = driver.now();
    let own = own(&driver);
    if let Some(mesh) = &mut driver.mesh {
        mesh.arrive(
            Arrival {
                from: 1,
                to: Some(own),
                text: "Where are you?",
                ago: 0,
                carriage: Carriage::Received,
                unread: true,
            },
            now,
        );
    }
    driver.wait(100_000);
    assert!(driver.stage.toast().is_some(), "a toast tells of it");
    tap(&mut driver, 233, 360);
    driver.wait(300_000);
    assert_eq!(child(&driver), Some(Child::Thread(member(&driver, 1))));
}

#[test]
fn a_recovered_message_is_neither_told_of_nor_unread() {
    let mut driver = start(false);
    let now = driver.now();
    if let Some(mesh) = &mut driver.mesh {
        mesh.arrive(
            Arrival {
                from: 2,
                to: None,
                text: "From before the restart.",
                ago: 30 * SECOND,
                carriage: Carriage::Received,
                unread: false,
            },
            now,
        );
    }
    driver.wait(100_000);
    assert_eq!(driver.stage.toast(), None);
    assert_eq!(driver.stage.events().unread(), 0);
}

#[test]
fn a_draft_is_reviewed_then_sent_once() {
    let mut driver = start(true);
    open_messages(&mut driver);
    tap(&mut driver, 233, 426);
    assert_eq!(child(&driver), Some(Child::Recipients));
    // RIDGE, the second row.
    tap(&mut driver, 233, 245);
    assert_eq!(child(&driver), Some(Child::Draft));
    // REVIEW does nothing with an empty draft.
    tap(&mut driver, 333, 131);
    assert_eq!(child(&driver), Some(Child::Draft));
    type_text(&mut driver, "on my way");
    tap(&mut driver, 333, 131);
    assert_eq!(child(&driver), Some(Child::Review));
    // EDIT returns to the same draft.
    tap(&mut driver, 160, 391);
    assert_eq!(child(&driver), Some(Child::Draft));
    type_text(&mut driver, "!");
    tap(&mut driver, 333, 131);
    let before = mesh(&driver).messages().len();
    tap(&mut driver, 306, 391);
    tap(&mut driver, 306, 391);
    assert_eq!(child(&driver), Some(Child::Thread(member(&driver, 1))));
    let messages = mesh(&driver).messages();
    assert_eq!(messages.len(), before + 1, "sent once");
    let sent = *messages.iter().last().unwrap();
    assert_eq!((sent.text(), sent.to), ("on my way!", Some(1)));
    driver.wait(12 * SECOND);
    assert_eq!(
        mesh(&driver)
            .messages()
            .iter()
            .find(|message| message.id == sent.id)
            .map(|message| message.carriage),
        Some(Carriage::Delivered)
    );
}

#[test]
fn a_cancelled_draft_comes_back_to_the_same_conversation() {
    let mut driver = start(true);
    open_messages(&mut driver);
    tap(&mut driver, 233, 265);
    tap(&mut driver, 233, 426);
    type_text(&mut driver, "half a thought");
    tap(&mut driver, 133, 131);
    assert_eq!(child(&driver), Some(Child::Thread(member(&driver, 1))));
    // Closed and opened again, the draft is still there.
    driver.cover();
    driver.wait(SECOND);
    open_messages(&mut driver);
    tap(&mut driver, 233, 265);
    tap(&mut driver, 233, 426);
    tap(&mut driver, 333, 131);
    assert_eq!(child(&driver), Some(Child::Review));
}

#[test]
fn mark_all_read_leaves_messages_unread_and_the_arc_on() {
    let mut driver = start(true);
    driver.wait(6 * SECOND);
    driver.swipe(Point::new(233, 430), Point::new(233, 120), 250_000);
    driver.settle();
    driver.wait(300_000);
    tap(&mut driver, 233, 426);
    tap(&mut driver, 233, 180);
    assert_eq!(driver.stage.events().unread(), 0);
    assert_eq!(mesh(&driver).messages().unread(), 2);
}

/// Drawing only each step's damage, in two buffers as the frame loop does, must leave the panel
/// as drawing whole would, through the inbox, a conversation, a draft, its review and sending.
#[test]
fn the_messages_damage_redraws_what_changed() {
    let mut driver = start(true);
    let mut buffers = Buffers::new();
    let mut steps = 0;
    driver.observe(move |stage, _| {
        let mut whole = FB::boxed();
        stage.draw(&mut *whole);
        let partial = buffers.draw(stage, stage.changed());
        let wrong = differing(partial, &whole);
        assert_eq!(wrong, 0, "step {steps}: {wrong} pixels differ");
        steps += 1;
    });
    open_messages(&mut driver);
    tap(&mut driver, 233, 265);
    driver.swipe(Point::new(233, 380), Point::new(233, 200), 400_000);
    driver.wait(2 * SECOND);
    tap(&mut driver, 233, 426);
    type_text(&mut driver, "On it.");
    tap(&mut driver, 333, 131);
    tap(&mut driver, 306, 391);
    driver.wait(12 * SECOND);
    tap(&mut driver, 233, 26);
    driver.wait(SECOND);
}

/// Member 1 wrote to this device and to the group, was removed, and a new device paired in at
/// the id it freed, as the lowest free id goes to the next device paired. The messages stay in
/// the store for a day. They once named their sender by the member now at its id, showing the
/// newcomer as their writer and sending a reply in that private conversation to it; now they
/// keep the removed device and its name, marked removed, and its conversation takes no reply.
#[test]
fn a_removed_members_messages_are_not_shown_as_a_newcomers() {
    let mut driver = start(false);
    let now = driver.now();
    let own = own(&driver);
    if let Some(mesh) = &mut driver.mesh {
        for to in [Some(own), None] {
            mesh.arrive(
                Arrival {
                    from: 1,
                    to,
                    text: "Meet at the gate.",
                    ago: 20 * 60 * SECOND,
                    carriage: Carriage::Received,
                    unread: false,
                },
                now,
            );
        }
        if let Some(group) = &mut mesh.view_mut().group
            && let Some(member) = &mut group.members[1]
        {
            member.name = Name::new(b"Newcomer").unwrap();
            member.device = [0xAB; 8];
            member.joined = Some(now as i64);
        }
    }
    driver.wait(100_000);
    open_messages(&mut driver);
    let mut shown = Vec::new();
    for row in [170, 265] {
        tap(&mut driver, 233, row);
        let texts: Vec<String> = driver.stage.drawer_text().map(str::to_owned).collect();
        assert!(
            texts.iter().any(|text| text.contains("gate")),
            "{:?} shows the message: {texts:?}",
            child(&driver)
        );
        let thread = child(&driver);
        if let Some(Child::Thread(Thread::Member(..))) = thread {
            tap(&mut driver, 233, 426);
            assert_eq!(
                child(&driver),
                thread,
                "WRITE takes no reply to a removed member"
            );
        }
        shown.push((thread, texts));
        tap(&mut driver, 233, 26);
    }
    assert!(
        shown
            .iter()
            .all(|(_, texts)| !texts.iter().any(|text| text.contains("NEWCOMER"))),
        "{shown:#?}"
    );
    let texts = |thread: fn(&Child) -> bool| {
        shown
            .iter()
            .find(|(child, _)| child.as_ref().is_some_and(thread))
            .map(|(_, texts)| texts.clone())
            .unwrap_or_default()
    };
    let group = texts(|child| matches!(child, Child::Thread(Thread::Group)));
    assert!(
        group.iter().any(|text| text == "NORTH-1 / REMOVED"),
        "{group:?}"
    );
    let private = texts(|child| matches!(child, Child::Thread(Thread::Member(..))));
    for line in ["PRIVATE", "NORTH-1", "Recipient is no longer a member."] {
        assert!(
            private.iter().any(|text| text == line),
            "{line}: {private:?}"
        );
    }
    assert!(
        private.iter().any(|text| text.starts_with("REMOVED / ")),
        "{private:?}"
    );
}

fn texts(driver: &Driver) -> Vec<String> {
    driver.stage.drawer_text().map(str::to_owned).collect()
}

fn fingerprint(thread: Thread) -> String {
    let Thread::Member(_, device) = thread else {
        return String::new();
    };
    device.iter().map(|byte| format!("{byte:02X}")).collect()
}

/// Gives the member at `id` a new name, or with `device`, hands its id to another device.
fn change_member(driver: &mut Driver, id: usize, name: &str, device: Option<[u8; 8]>) {
    if let Some(member) = driver
        .mesh
        .as_mut()
        .and_then(|mesh| mesh.view_mut().group.as_mut())
        .and_then(|group| group.members[id].as_mut())
    {
        member.name = Name::new(name.as_bytes()).unwrap();
        if let Some(device) = device {
            member.device = device;
        }
    }
    driver.wait(100_000);
}

/// The draft and its review name the recipient in full, with its id and its device's
/// fingerprint, which tell two members of one name apart. A rename changes the name shown, not
/// where the message goes.
#[test]
fn a_draft_names_its_recipient_by_name_id_and_device() {
    let mut driver = start(true);
    let ridge = member(&driver, 1);
    let identity = format!("PRIVATE [01] / {}", fingerprint(ridge));
    open_messages(&mut driver);
    tap(&mut driver, 233, 265);
    tap(&mut driver, 233, 426);
    type_text(&mut driver, "on my way");
    assert!(texts(&driver).contains(&identity), "{:?}", texts(&driver));
    change_member(&mut driver, 1, "Ridge_Walker07!?", None);
    for line in ["Ridge_Walker07!?", &identity] {
        assert!(
            texts(&driver).iter().any(|text| text == line),
            "{line}: {:?}",
            texts(&driver)
        );
    }
    tap(&mut driver, 333, 131);
    assert_eq!(child(&driver), Some(Child::Review));
    for line in ["Ridge_Walker07!?", &identity, "PRIVATE MESSAGE"] {
        assert!(
            texts(&driver).iter().any(|text| text == line),
            "{line}: {:?}",
            texts(&driver)
        );
    }
    tap(&mut driver, 306, 391);
    let sent = *mesh(&driver).messages().iter().last().unwrap();
    assert_eq!(sent.text(), "on my way");
    assert_eq!(sent.thread(own(&driver)), ridge);
}

/// A recipient removed while its draft shows keeps the draft, and SEND waits, saying why. A
/// device given its id afterwards does not take the draft over.
#[test]
fn a_draft_to_a_removed_member_keeps_its_text_and_cannot_be_sent() {
    let mut driver = start(true);
    let ridge = member(&driver, 1);
    open_messages(&mut driver);
    tap(&mut driver, 233, 265);
    tap(&mut driver, 233, 426);
    type_text(&mut driver, "on my way");
    tap(&mut driver, 333, 131);
    change_member(&mut driver, 1, "Newcomer", Some([0xAB; 8]));
    let removed = format!("REMOVED / {}", fingerprint(ridge));
    for line in [removed.as_str(), "Recipient is no longer a member."] {
        assert!(
            texts(&driver).iter().any(|text| text == line),
            "{line}: {:?}",
            texts(&driver)
        );
    }
    assert!(!texts(&driver).iter().any(|text| text.contains("Newcomer")));
    let before = mesh(&driver).messages().len();
    tap(&mut driver, 306, 391);
    assert_eq!(child(&driver), Some(Child::Review));
    assert_eq!(mesh(&driver).messages().len(), before, "nothing sent");
    tap(&mut driver, 160, 391);
    assert_eq!(child(&driver), Some(Child::Draft));
    assert!(
        texts(&driver).iter().any(|text| text.contains("on my way")),
        "{:?}",
        texts(&driver)
    );
}
