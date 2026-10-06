//! A removal's screens in each of their states, with the design's fixture: this device in a
//! group with Ridge, Cove and Moss, Ridge the member removed, an eight-minute switch with 06:42
//! left. Names match the hand-off's targets where one exists.

use embedded_graphics::prelude::Point;
use octowhere_ui::{
    chrome::FB,
    ui::{
        group::view::{At, Decline, Name, Position, RemovalStage},
        rest::Timeout,
        screens::{PeripheralState, Screen},
        stage::Stage,
    },
};
use octowhere_ui_script::{
    Driver,
    sim::{self, SWITCH_AFTER, Sim},
};

const SECOND: u64 = 1_000_000;
const MINUTE: u64 = 60 * SECOND;
const TOAST: Point = Point::new(233, 360);
const RIGHT: Point = Point::new(306, 391);
const LEFT: Point = Point::new(160, 391);
const HANDLE: Point = Point::new(110, 372);
/// What the design's countdown shows: 06:42 of eight minutes left.
const LEFT_OF_EIGHT: u64 = 402 * SECOND;

/// The clock face, in the fixture's group.
fn start() -> Driver<'static> {
    start_named(["Ridge", "Cove", "Moss"])
}

/// The clock face, in a group whose other members have `names`.
fn start_named(names: [&str; 3]) -> Driver<'static> {
    let mut driver = Driver::on(Screen::Clock);
    driver.stage = Stage::new(PeripheralState {
        firmware: "0.1.0",
        timeout: Timeout::Never,
        ..PeripheralState::default()
    });
    driver.stage.show(Screen::Clock);
    driver.sensors(super::sensors());
    driver.wait(600_000);
    let now = driver.now();
    let mut group = sim::group(4, now);
    for (id, name) in [1, 2, 3].into_iter().zip(names) {
        if let Some(member) = &mut group.members[id] {
            member.name = Name::new(name.as_bytes()).expect("a fixture name");
        }
    }
    if let Some(ridge) = &mut group.members[1] {
        ridge.device = [0x9c, 0x2a, 0x7f, 0x10, 0x5e, 0x31, 0xa8, 0x04];
        ridge.heard = Some(now as At - 42 * SECOND as At);
        ridge.position = Position::At(now as At - 18 * SECOND as At);
    }
    driver.mesh = Some(Sim::new(Some(group)));
    driver.wait(100_000);
    driver
}

fn tap(driver: &mut Driver, point: Point) {
    driver.tap(point);
    driver.wait(300_000);
}

fn sim<'a>(driver: &'a mut Driver<'_>) -> &'a mut Sim {
    driver.mesh.as_mut().expect("a scripted mesh")
}

/// Puts the removal under way's switch [`LEFT_OF_EIGHT`] away, as the design's countdown shows
/// it, eight minutes after it began, and Ridge heard and placed as the design has it.
fn as_designed(driver: &mut Driver) {
    let now = driver.now() as At;
    let view = sim(driver).view_mut();
    if let Some(current) = &mut view.removals.current {
        current.stage = RemovalStage::Pending {
            since: now + LEFT_OF_EIGHT as At - SWITCH_AFTER as At,
            switch: Some(now + LEFT_OF_EIGHT as At),
        };
    }
    if let Some(ridge) = view
        .group
        .as_mut()
        .and_then(|group| group.members[1].as_mut())
    {
        ridge.heard = Some(now - 42 * SECOND as At);
        ridge.position = Position::At(now - 18 * SECOND as At);
    }
    driver.wait(1);
}

fn snap(frames: &mut Vec<(String, Box<FB>)>, name: &str, driver: &Driver) {
    let mut fb = FB::boxed();
    driver.stage.draw(&mut *fb);
    frames.push((name.into(), fb));
}

/// Ridge's details, from the panel's group screen.
fn ridge(driver: &mut Driver) {
    driver.swipe(Point::new(233, 80), Point::new(233, 420), 250_000);
    driver.settle();
    driver.wait(600_000);
    driver.swipe(Point::new(380, 250), Point::new(80, 250), 300_000);
    driver.settle();
    driver.wait(400_000);
    tap(driver, Point::new(233, 190));
    tap(driver, Point::new(159, 353));
    tap(driver, Point::new(233, 380));
}

/// Cove's request to remove Ridge, its switch `switch_in` away, opened from its toast.
fn incoming(switch_in: u64) -> Driver<'static> {
    let mut driver = start();
    let now = driver.now();
    sim(&mut driver).request_removal(2, 1, switch_in, now);
    driver.wait(200_000);
    tap(&mut driver, TOAST);
    driver
}

/// Cove's request, switched to `ago` before now and declinable until `until` after it, or not.
fn switched(ago: u64, decline: impl Fn(u64) -> Decline) -> Driver<'static> {
    let mut driver = start();
    let now = driver.now();
    sim(&mut driver).request_removal(2, 1, SECOND, now);
    driver.wait(1_500_000);
    let now = driver.now();
    if let Some(current) = &mut sim(&mut driver).view_mut().removals.current {
        current.stage = RemovalStage::Switched {
            at: now as At - ago as At,
            decline: decline(now),
        };
    }
    driver.wait(6 * SECOND);
    // Its event's row, at the top of the list.
    driver.swipe(Point::new(233, 430), Point::new(233, 120), 250_000);
    driver.settle();
    driver.wait(300_000);
    tap(&mut driver, Point::new(233, 220));
    driver
}

pub fn frames() -> Vec<(String, Box<FB>)> {
    let mut frames = Vec::new();

    // This device removes Ridge.
    let mut own = start();
    ridge(&mut own);
    snap(&mut frames, "removal-member-detail", &own);
    tap(&mut own, Point::new(233, 353));
    snap(&mut frames, "removal-01-confirm-remove", &own);
    own.swipe(HANDLE, HANDLE + Point::new(120, 0), 300_000);
    snap(&mut frames, "removal-slide-released", &own);
    own.wait(400_000);
    own.swipe(HANDLE, HANDLE + Point::new(236, 0), 400_000);
    own.wait(SECOND);
    as_designed(&mut own);
    snap(&mut frames, "removal-02-removal-ongoing", &own);
    own.wait(LEFT_OF_EIGHT);
    snap(&mut frames, "removal-own-switched", &own);

    // Cove's request, before its switch.
    let mut request = incoming(LEFT_OF_EIGHT);
    as_designed(&mut request);
    snap(&mut frames, "removal-03-incoming-request", &request);
    tap(&mut request, LEFT);
    snap(&mut frames, "removal-details", &request);
    tap(&mut request, Point::new(233, 25));
    tap(&mut request, RIGHT);
    snap(&mut frames, "removal-09-confirm-decline-pending", &request);
    request.swipe(HANDLE, HANDLE + Point::new(236, 0), 400_000);
    request.wait(300_000);
    snap(&mut frames, "removal-declined", &request);

    // Its toast, over the clock face.
    let mut toast = start();
    let now = toast.now();
    sim(&mut toast).request_removal(2, 1, LEFT_OF_EIGHT, now);
    toast.wait(SECOND);
    snap(&mut frames, "removal-toast-request", &toast);

    // After the switch, with its day to decline nearly whole, and past it.
    let mut after = switched(18 * MINUTE, |now| {
        Decline::Until((now + 24 * 60 * MINUTE - 18 * MINUTE) as At)
    });
    snap(&mut frames, "removal-04-after-switch", &after);
    tap(&mut after, RIGHT);
    snap(&mut frames, "removal-05-confirm-return", &after);
    let expired = switched(25 * 60 * MINUTE, |_| Decline::Expired);
    snap(&mut frames, "removal-10-decline-expired", &expired);

    // Two requests: Cove's to remove Ridge won over Moss's to remove Cove.
    let mut rivals = start();
    let now = rivals.now();
    sim(&mut rivals).request_removal(2, 1, LEFT_OF_EIGHT, now);
    rivals.wait(6 * SECOND);
    let now = rivals.now();
    sim(&mut rivals).losing_rival(3, 2, now);
    rivals.wait(200_000);
    tap(&mut rivals, TOAST);
    tap(&mut rivals, Point::new(233, 170));
    snap(&mut frames, "removal-06-rival-requests", &rivals);
    rivals.swipe(Point::new(233, 430), Point::new(233, 120), 250_000);
    rivals.wait(SECOND);
    tap(&mut rivals, Point::new(233, 25));
    rivals.wait(SECOND);
    snap(&mut frames, "removal-drawer-events", &rivals);

    // Cove removed this device.
    let mut removed = start();
    let now = removed.now();
    sim(&mut removed).removed_by(2, now);
    removed.wait(SECOND);
    snap(&mut frames, "removal-toast-removed", &removed);
    tap(&mut removed, TOAST);
    snap(&mut frames, "removal-07-this-device-removed", &removed);

    // Ridge, awaiting the switch of Cove's request.
    let mut pending = start();
    let now = pending.now();
    sim(&mut pending).request_removal(2, 1, LEFT_OF_EIGHT, now);
    pending.wait(6 * SECOND);
    ridge(&mut pending);
    as_designed(&mut pending);
    snap(&mut frames, "removal-08-pending-member", &pending);

    // REMOVE while another removal is under way.
    let mut busy = start();
    let now = busy.now();
    sim(&mut busy).request_removal(2, 3, LEFT_OF_EIGHT, now);
    busy.wait(6 * SECOND);
    ridge(&mut busy);
    tap(&mut busy, Point::new(233, 353));
    snap(&mut frames, "removal-unavailable-underway", &busy);

    // A request the mesh could not store.
    let mut failed = start();
    sim(&mut failed).store_fails = true;
    ridge(&mut failed);
    tap(&mut failed, Point::new(233, 353));
    failed.swipe(HANDLE, HANDLE + Point::new(236, 0), 400_000);
    failed.wait(300_000);
    snap(&mut frames, "removal-unavailable-unsaved", &failed);

    // The longest names a member can have.
    let long = || start_named(["Ridge_Walker07!?", "ABCDEFGHIJKLMNOP", "camp stove mk II"]);
    let mut request = long();
    let now = request.now();
    sim(&mut request).request_removal(2, 1, LEFT_OF_EIGHT, now);
    request.wait(200_000);
    snap(&mut frames, "removal-long-toast", &request);
    tap(&mut request, TOAST);
    snap(&mut frames, "removal-long-request", &request);
    tap(&mut request, RIGHT);
    snap(&mut frames, "removal-long-decline", &request);
    let mut rivals = long();
    let now = rivals.now();
    sim(&mut rivals).request_removal(2, 1, LEFT_OF_EIGHT, now);
    rivals.wait(6 * SECOND);
    let now = rivals.now();
    sim(&mut rivals).losing_rival(3, 2, now);
    rivals.wait(200_000);
    tap(&mut rivals, TOAST);
    snap(&mut frames, "removal-long-rivals", &rivals);
    tap(&mut rivals, Point::new(233, 25));
    rivals.wait(SECOND);
    snap(&mut frames, "removal-long-events", &rivals);
    let mut removed = long();
    let now = removed.now();
    sim(&mut removed).removed_by(2, now);
    removed.wait(200_000);
    tap(&mut removed, TOAST);
    snap(&mut frames, "removal-long-removed", &removed);
    let mut own = long();
    ridge(&mut own);
    tap(&mut own, Point::new(233, 353));
    snap(&mut frames, "removal-long-confirm", &own);
    frames
}
