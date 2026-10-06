//! The messages screens in each of their states, with the design's fixture: this device in a
//! group with Ridge, Cove and Moss, conversations with the group and with Ridge and Cove, some
//! unread. Names match the hand-off's targets where one exists.

use embedded_graphics::prelude::Point;
use octowhere_ui::{
    chrome::FB,
    ui::{
        group::{
            keyboard::{Mode, taps},
            sim::{self, Arrival, Sim},
            view::{Carriage, MessagesView, Name},
        },
        rest::Timeout,
        screens::{PeripheralState, Screen},
        script::Driver,
        stage::Stage,
    },
};

const SECOND: u64 = 1_000_000;
const DRAFT: &str = "I am at the bridge. Please take the north path and wait at the turn. I will meet you there in ten minutes.";

/// The clock face, with the fixture's group and, with `conversations`, its messages.
fn start(conversations: bool) -> Driver<'static> {
    let mut driver = Driver::on(Screen::Clock);
    driver.stage = Stage::new(PeripheralState {
        firmware: "0.1.0",
        timeout: Timeout::Never,
        ..PeripheralState::default()
    });
    driver.stage.use_messages(Box::leak(MessagesView::boxed()));
    driver.stage.show(Screen::Clock);
    driver.sensors(super::sensors());
    driver.wait(600_000);
    let now = driver.now();
    let mut group = sim::group(4, now);
    for (id, name) in [(1, "Ridge"), (2, "Cove"), (3, "Moss")] {
        if let Some(member) = &mut group.members[id] {
            member.name = Name::new(name.as_bytes()).expect("a fixture name");
        }
    }
    let mut mesh = Sim::new(Some(group));
    if conversations {
        mesh.conversations(now);
    }
    driver.mesh = Some(mesh);
    driver.wait(100_000);
    // The fixture's unread messages are told of as they come; let their toast go.
    driver.wait(6 * SECOND);
    driver
}

fn tap(driver: &mut Driver, x: i32, y: i32) {
    driver.tap(Point::new(x, y));
    driver.wait(300_000);
}

/// The Messages root, from the clock face.
fn inbox(conversations: bool) -> Driver<'static> {
    let mut driver = start(conversations);
    driver.swipe(Point::new(233, 430), Point::new(233, 120), 250_000);
    driver.settle();
    driver.wait(300_000);
    driver.swipe(Point::new(380, 260), Point::new(80, 260), 300_000);
    driver.wait(400_000);
    driver
}

fn type_text(driver: &mut Driver, text: &str) {
    for point in taps(text, Mode::Lower) {
        driver.tap(point);
        driver.wait(20_000);
    }
}

fn snap(frames: &mut Vec<(String, Box<FB>)>, name: &str, driver: &Driver) {
    let mut fb = FB::boxed();
    driver.stage.draw(&mut *fb);
    frames.push((name.into(), fb));
}

pub fn frames() -> Vec<(String, Box<FB>)> {
    let mut frames = Vec::new();

    let mut inbox_driver = inbox(true);
    snap(&mut frames, "messages-01-inbox", &inbox_driver);
    tap(&mut inbox_driver, 233, 170);
    snap(&mut frames, "messages-02-group", &inbox_driver);

    let mut private = inbox(true);
    tap(&mut private, 233, 265);
    snap(&mut frames, "messages-03-private", &private);
    tap(&mut private, 233, 426);
    type_text(&mut private, DRAFT);
    snap(&mut frames, "messages-04-write", &private);
    tap(&mut private, 333, 131);
    snap(&mut frames, "messages-05-review", &private);
    tap(&mut private, 306, 391);
    snap(&mut frames, "messages-sent-queued", &private);
    private.wait(12 * SECOND);
    snap(&mut frames, "messages-sent-delivered", &private);

    let mut recipient = inbox(true);
    tap(&mut recipient, 233, 426);
    snap(&mut frames, "messages-07-recipient", &recipient);

    let mut empty = inbox(false);
    snap(&mut frames, "messages-empty", &empty);
    tap(&mut empty, 233, 426);
    tap(&mut empty, 233, 170);
    snap(&mut frames, "messages-write-empty", &empty);

    // A private message arriving over the clock face.
    let mut arrival = start(true);
    let now = arrival.now();
    if let Some(mesh) = &mut arrival.mesh {
        let own = mesh.view().group.as_ref().map_or(0, |group| group.own);
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
    arrival.wait(SECOND);
    snap(&mut frames, "messages-06-arrival", &arrival);

    // A whole message to read through: every character a draft holds.
    let mut long = inbox(true);
    tap(&mut long, 233, 265);
    tap(&mut long, 233, 426);
    type_text(
        &mut long,
        "Meet at the north gate at 14:30; bring water, the spare cells & both radios. If the ridge path is closed, take the river trail (east side) instead!",
    );
    snap(&mut frames, "messages-write-long", &long);
    tap(&mut long, 333, 131);
    snap(&mut frames, "messages-review-long", &long);

    // Ridge's whole name, and Moss taking the same name on another device.
    let identity = || {
        let mut driver = inbox(true);
        for id in [1, 3] {
            rename(&mut driver, id, "Ridge_Walker07!?");
        }
        driver
    };
    let mut ridge = identity();
    tap(&mut ridge, 233, 265);
    tap(&mut ridge, 233, 426);
    type_text(&mut ridge, DRAFT);
    snap(&mut frames, "messages-08-long-recipient-draft", &ridge);
    tap(&mut ridge, 333, 131);
    snap(&mut frames, "messages-09-long-recipient-review", &ridge);
    // Removed while the review shows: SEND waits, and says why.
    remove(&mut ridge, 1);
    snap(&mut frames, "messages-10-recipient-removed-review", &ridge);
    tap(&mut ridge, 233, 26);
    snap(&mut frames, "messages-11-recipient-removed-draft", &ridge);
    let mut other = identity();
    tap(&mut other, 233, 426);
    tap(&mut other, 233, 390);
    type_text(&mut other, DRAFT);
    snap(&mut frames, "messages-12-same-name-other-device", &other);
    let mut group = identity();
    tap(&mut group, 233, 426);
    tap(&mut group, 233, 170);
    type_text(&mut group, DRAFT);
    snap(&mut frames, "messages-13-group-draft", &group);
    let mut removed = identity();
    remove(&mut removed, 1);
    tap(&mut removed, 233, 265);
    snap(
        &mut frames,
        "messages-14-removed-recipient-thread",
        &removed,
    );
    frames
}

fn rename(driver: &mut Driver, id: usize, name: &str) {
    if let Some(member) = driver
        .mesh
        .as_mut()
        .and_then(|mesh| mesh.view_mut().group.as_mut())
        .and_then(|group| group.members[id].as_mut())
    {
        member.name = Name::new(name.as_bytes()).expect("a fixture name");
    }
    driver.wait(100_000);
}

fn remove(driver: &mut Driver, id: usize) {
    if let Some(group) = driver
        .mesh
        .as_mut()
        .and_then(|mesh| mesh.view_mut().group.as_mut())
    {
        group.members[id] = None;
    }
    driver.wait(100_000);
}
