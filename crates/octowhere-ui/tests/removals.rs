//! A removal's screens, driven through the stage with a simulated mesh: removing a member, a
//! request from another, declining it before and after the switch, two that compete, and this
//! device removed.

mod common;

use common::{Buffers, differing};
use embedded_graphics::prelude::Point;
use octowhere_ui::{
    chrome::FB,
    ui::{
        drawer::Child,
        events::Kind,
        group::view::{Decline, Name, RemovalStage, RemovalView, Request},
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
const TOAST: Point = Point::new(233, 360);
const LEFT: Point = Point::new(160, 391);
const RIGHT: Point = Point::new(306, 391);
const FOOTER: Point = Point::new(233, 426);
const BACK: Point = Point::new(233, 25);
/// On the group screens: MEMBERS, the second member's row, and REMOVE.
const MEMBERS: Point = Point::new(159, 353);
const RIDGE_ROW: Point = Point::new(233, 380);
const REMOVE: Point = Point::new(233, 353);
const HANDLE: Point = Point::new(110, 372);

/// The clock face, in a group of four: this device, Ridge, Cove and Moss.
fn start() -> Driver<'static> {
    let mut driver = Driver::on(Screen::Clock);
    driver.stage = Stage::new(PeripheralState {
        timeout: Timeout::Never,
        ..PeripheralState::default()
    });
    driver.stage.show(Screen::Clock);
    driver.wait(600_000);
    let now = driver.now();
    let mut group = sim::group(4, now);
    for (id, name) in [(1, "Ridge"), (2, "Cove"), (3, "Moss")] {
        if let Some(member) = &mut group.members[id] {
            member.name = Name::new(name.as_bytes()).unwrap();
        }
    }
    driver.mesh = Some(Sim::new(Some(group)));
    driver.wait(100_000);
    driver
}

fn tap(driver: &mut Driver, point: Point) {
    driver.tap(point);
    driver.wait(300_000);
}

fn slide(driver: &mut Driver, by: i32) {
    driver.swipe(HANDLE, HANDLE + Point::new(by, 0), 400_000);
    driver.wait(300_000);
}

fn sim<'a>(driver: &'a mut Driver<'_>) -> &'a mut Sim {
    driver.mesh.as_mut().expect("a simulated mesh")
}

fn current(driver: &Driver) -> Option<RemovalView> {
    driver.mesh.as_ref()?.view().removals.current
}

fn child(driver: &Driver) -> Option<Child> {
    driver.stage.drawer().and_then(|drawer| drawer.child())
}

fn drawer_shows(driver: &Driver, text: &str) -> bool {
    driver.stage.drawer_text().any(|line| line == text)
}

fn drawer_shown(driver: &Driver) -> Vec<String> {
    driver.stage.drawer_text().map(String::from).collect()
}

fn group_shows(driver: &Driver, text: &str) -> bool {
    driver.stage.group_text().any(|line| line == text)
}

fn group_shown(driver: &Driver) -> Vec<String> {
    driver.stage.group_text().map(String::from).collect()
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
    tap(driver, MEMBERS);
    tap(driver, RIDGE_ROW);
}

/// Cove asks to remove Ridge, its switch `switch_in` away, and its toast opens it.
fn incoming(driver: &mut Driver, switch_in: u64) {
    let now = driver.now();
    sim(driver).request_removal(2, 1, switch_in, now);
    driver.wait(200_000);
    assert!(driver.stage.toast().is_some(), "a toast tells of it");
    tap(driver, TOAST);
}

#[test]
fn removing_a_member_takes_a_slide_then_counts_down_to_its_switch() {
    let mut driver = start();
    ridge(&mut driver);
    tap(&mut driver, REMOVE);
    assert!(
        group_shows(&driver, "SLIDE TO REMOVE"),
        "{:?}",
        group_shown(&driver)
    );
    assert!(group_shows(&driver, "RIDGE"), "{:?}", group_shown(&driver));
    // A tap on the track, or a slide that stops short, asks nothing.
    tap(&mut driver, Point::new(300, 372));
    slide(&mut driver, 200);
    assert_eq!(current(&driver), None);
    slide(&mut driver, 236);
    let removal = current(&driver).expect("the removal started");
    assert_eq!((removal.remover, removal.removed), (0, 1));
    for line in [
        "REMOVING",
        "YOUR REQUEST",
        "SWITCH IN",
        "You cannot decline your own request.",
    ] {
        assert!(
            group_shows(&driver, line),
            "{line}: {:?}",
            group_shown(&driver)
        );
    }
    // This device's own request is listed, read, and cannot be dismissed while it runs.
    let event = driver.stage.events().removal(removal.key).copied().unwrap();
    assert!(!event.unread);
    assert!(event.ongoing());
    driver.wait(SWITCH_AFTER);
    assert!(matches!(
        current(&driver).map(|removal| removal.stage),
        Some(RemovalStage::Switched {
            decline: Decline::Own,
            ..
        })
    ));
    assert!(
        group_shows(&driver, "DECLINE UNAVAILABLE"),
        "{:?}",
        group_shown(&driver)
    );
    let mesh = driver.mesh.as_ref().unwrap().view();
    assert!(
        mesh.group.as_ref().unwrap().member(1).is_none(),
        "gone at its switch"
    );
}

#[test]
fn remove_is_unavailable_while_another_removal_is_under_way() {
    let mut driver = start();
    let now = driver.now();
    sim(&mut driver).request_removal(2, 3, SWITCH_AFTER, now);
    driver.wait(100_000);
    ridge(&mut driver);
    tap(&mut driver, REMOVE);
    for line in [
        "REMOVE UNAVAILABLE",
        "Another removal is under way.",
        "VIEW REQUEST",
    ] {
        assert!(
            group_shows(&driver, line),
            "{line}: {:?}",
            group_shown(&driver)
        );
    }
    tap(&mut driver, FOOTER);
    let key = current(&driver).unwrap().key;
    let id = driver.stage.events().removal(key).unwrap().id;
    assert_eq!(child(&driver), Some(Child::Event(id)));
}

#[test]
fn a_request_is_told_and_declined_before_its_switch() {
    let mut driver = start();
    incoming(&mut driver, 402 * SECOND);
    for line in [
        "GROUP CHANGE",
        "REMOVAL REQUEST",
        "RIDGE",
        "[01] / REQUESTED BY COVE [02]",
        "Scheduled switch in 06:42.",
        "Ridge is still a member until then.",
    ] {
        assert!(
            drawer_shows(&driver, line),
            "{line}: {:?}",
            drawer_shown(&driver)
        );
    }
    let key = current(&driver).unwrap().key;
    let id = driver.stage.events().removal(key).unwrap().id;
    assert_eq!(
        driver
            .stage
            .events()
            .get(id)
            .and_then(|event| event.protected()),
        Some(octowhere_ui::ui::events::Protected::Unfinished),
    );
    tap(&mut driver, LEFT);
    assert_eq!(child(&driver), Some(Child::Details(id)));
    tap(&mut driver, BACK);
    tap(&mut driver, RIGHT);
    assert!(
        drawer_shows(&driver, "STAY IN THE OLD GROUP"),
        "{:?}",
        drawer_shown(&driver)
    );
    // CANCEL backs out without asking.
    tap(&mut driver, FOOTER);
    assert_eq!(child(&driver), Some(Child::Event(id)));
    tap(&mut driver, RIGHT);
    slide(&mut driver, 236);
    assert!(matches!(
        current(&driver).map(|removal| removal.stage),
        Some(RemovalStage::Declined { .. })
    ));
    assert_eq!(child(&driver), Some(Child::Event(id)));
    assert!(
        drawer_shows(&driver, "DECLINED HERE"),
        "{:?}",
        drawer_shown(&driver)
    );
}

/// A slide begun before the switch and finished after it declines nothing: the confirmation
/// changes to what declining costs now, and a fresh slide is needed.
#[test]
fn a_switch_under_the_confirmation_needs_a_fresh_slide() {
    let mut driver = start();
    incoming(&mut driver, 3 * SECOND);
    tap(&mut driver, RIGHT);
    assert!(
        drawer_shows(&driver, "STAY IN THE OLD GROUP"),
        "{:?}",
        drawer_shown(&driver)
    );
    let mut path: Vec<Point> = (0..=10).map(|i| HANDLE + Point::new(i * 10, 0)).collect();
    let held = *path.last().unwrap();
    path.extend(std::iter::repeat_n(held, 240));
    path.extend((1..=15).map(|i| held + Point::new(i * 10, 0)));
    driver.stroke(&path);
    driver.wait(300_000);
    assert!(matches!(
        current(&driver).map(|removal| removal.stage),
        Some(RemovalStage::Switched { .. })
    ));
    assert!(
        drawer_shows(&driver, "RETURN TO THE OLD GROUP"),
        "{:?}",
        drawer_shown(&driver)
    );
    slide(&mut driver, 236);
    assert!(matches!(
        current(&driver).map(|removal| removal.stage),
        Some(RemovalStage::Declined { .. })
    ));
    let mesh = driver.mesh.as_ref().unwrap().view();
    assert!(
        mesh.group.as_ref().unwrap().member(1).is_some(),
        "Ridge is back"
    );
}

#[test]
fn after_the_switch_the_decline_shows_its_time_left_until_it_ends() {
    let mut driver = start();
    incoming(&mut driver, SECOND);
    driver.wait(SECOND);
    for line in [
        "DECLINE AVAILABLE",
        "24:00 LEFT",
        "Ridge was removed from this group.",
    ] {
        assert!(
            drawer_shows(&driver, line),
            "{line}: {:?}",
            drawer_shown(&driver)
        );
    }
    let key = current(&driver).unwrap().key;
    let id = driver.stage.events().removal(key).unwrap().id;
    assert!(driver.stage.events().get(id).unwrap().protected().is_some());
    // Its day ends sooner here than a day.
    let until = driver.now() as i64 + 2 * SECOND as i64;
    if let Some(current) = &mut sim(&mut driver).view_mut().removals.current
        && let RemovalStage::Switched { decline, .. } = &mut current.stage
    {
        *decline = Decline::Until(until);
    }
    driver.wait(3 * SECOND);
    for line in ["DECLINE UNAVAILABLE", "The time to decline has ended."] {
        assert!(
            drawer_shows(&driver, line),
            "{line}: {:?}",
            drawer_shown(&driver)
        );
    }
    assert_eq!(driver.stage.events().get(id).unwrap().protected(), None);
}

#[test]
fn two_requests_show_side_by_side_and_each_opens_its_own() {
    let mut driver = start();
    let now = driver.now();
    sim(&mut driver).request_removal(2, 1, SWITCH_AFTER, now);
    driver.wait(6 * SECOND);
    // Moss asks to remove Cove, and its request wins.
    incoming_rival(&mut driver);
    for line in [
        "TWO REQUESTS",
        "CURRENT WINNER",
        "REMOVE COVE [02]",
        "REQUESTED BY MOSS [03]",
        "RIVAL REQUEST",
        "REMOVE RIDGE [01]",
        "REQUESTED BY COVE [02]",
    ] {
        assert!(
            drawer_shows(&driver, line),
            "{line}: {:?}",
            drawer_shown(&driver)
        );
    }
    let Some(Child::Rivals { ids, selected }) = child(&driver) else {
        panic!("{:?}", child(&driver));
    };
    assert_eq!(selected, 0, "the toast was the winner's");
    tap(&mut driver, Point::new(233, 300));
    tap(&mut driver, FOOTER);
    assert_eq!(child(&driver), Some(Child::Event(ids[1])));
    assert!(
        drawer_shows(&driver, "RIVAL REQUEST"),
        "{:?}",
        drawer_shown(&driver)
    );
}

fn incoming_rival(driver: &mut Driver) {
    let now = driver.now();
    sim(driver).request_removal(3, 2, SWITCH_AFTER, now);
    driver.wait(200_000);
    tap(driver, TOAST);
}

#[test]
fn this_device_removed_stays_and_leave_group_asks_first() {
    let mut driver = start();
    let now = driver.now();
    sim(&mut driver).removed_by(2, now);
    driver.wait(200_000);
    tap(&mut driver, TOAST);
    for line in [
        "REMOVED",
        "You were removed by Cove [02].",
        "This device stays in the old group.",
        "Leaving is your choice.",
    ] {
        assert!(
            drawer_shows(&driver, line),
            "{line}: {:?}",
            drawer_shown(&driver)
        );
    }
    tap(&mut driver, LEFT);
    assert_eq!(child(&driver), None, "CLOSE goes back to the list");
    let id = driver
        .stage
        .events()
        .ordered()
        .find(|event| matches!(event.kind, Kind::Removed { .. }))
        .unwrap()
        .id;
    // Its row, at the top of the list.
    tap(&mut driver, Point::new(233, 220));
    assert_eq!(child(&driver), Some(Child::Event(id)));
    tap(&mut driver, RIGHT);
    assert!(driver.stage.drawer().is_none());
    assert!(
        group_shows(&driver, "LEAVE GROUP?"),
        "{:?}",
        group_shown(&driver)
    );
    assert!(
        driver.mesh.as_ref().unwrap().view().group.is_some(),
        "nothing left yet"
    );
}

#[test]
fn a_member_awaiting_its_switch_shows_it_and_opens_its_request() {
    let mut driver = start();
    let now = driver.now();
    sim(&mut driver).request_removal(2, 1, 402 * SECOND, now);
    driver.wait(6 * SECOND);
    ridge(&mut driver);
    for line in [
        "MEMBER",
        "RIDGE",
        "REMOVAL PENDING",
        "Still a member until the switch.",
        "VIEW REQUEST",
    ] {
        assert!(
            group_shows(&driver, line),
            "{line}: {:?}",
            group_shown(&driver)
        );
    }
    assert!(
        driver
            .stage
            .group_text()
            .any(|line| line.starts_with("SWITCH IN 06:")),
        "{:?}",
        group_shown(&driver)
    );
    tap(&mut driver, FOOTER);
    let key = current(&driver).unwrap().key;
    let id = driver.stage.events().removal(key).unwrap().id;
    assert_eq!(child(&driver), Some(Child::Event(id)));
}

#[test]
fn a_decline_goes_to_the_mesh_by_its_key() {
    let mut driver = start();
    incoming(&mut driver, 402 * SECOND);
    let key = current(&driver).unwrap().key;
    tap(&mut driver, RIGHT);
    let mut asked = Vec::new();
    for update in driver.swipe(HANDLE, HANDLE + Point::new(236, 0), 400_000) {
        asked.extend(update.mesh);
    }
    assert_eq!(asked, [Request::Keep { key }]);
}

/// Drawing only each step's damage, in two buffers as the frame loop does, must leave the panel
/// as drawing whole would, through a removal's confirmation, its countdown and its switch, and
/// a request's detail and decline.
#[test]
fn the_removal_screens_redraw_only_what_changed() {
    let mut driver = start();
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
    ridge(&mut driver);
    tap(&mut driver, REMOVE);
    slide(&mut driver, 120);
    slide(&mut driver, 236);
    driver.wait(3 * SECOND);
    let now = driver.now();
    sim(&mut driver).removed_by(2, now);
    driver.wait(2 * SECOND);
}

/// ADD, from the panel's group screen.
fn add(driver: &mut Driver) {
    driver.swipe(Point::new(233, 80), Point::new(233, 420), 250_000);
    driver.settle();
    driver.wait(600_000);
    driver.swipe(Point::new(380, 250), Point::new(80, 250), 300_000);
    driver.settle();
    driver.wait(400_000);
    tap(driver, Point::new(233, 190));
    tap(driver, Point::new(307, 353));
    tap(driver, Point::new(156, 353));
    tap(driver, Point::new(233, 353));
}

/// Refused for a removal under way, ADD offers that removal's request, and opens it whatever
/// is under way by then: a rival that came since does not stand in for it.
#[test]
fn an_add_refused_for_a_removal_opens_that_removal() {
    let mut driver = start();
    let now = driver.now();
    sim(&mut driver).request_removal(2, 3, SWITCH_AFTER, now);
    driver.wait(100_000);
    let blocking = current(&driver).unwrap().key;
    add(&mut driver);
    for line in ["REMOVING", "VIEW REQUEST", "BACK"] {
        assert!(
            group_shows(&driver, line),
            "{line}: {:?}",
            group_shown(&driver)
        );
    }
    let now = driver.now();
    sim(&mut driver).request_removal(3, 2, SWITCH_AFTER, now);
    // Its toast goes first.
    driver.wait(6 * SECOND);
    assert_ne!(current(&driver).unwrap().key, blocking);
    tap(&mut driver, Point::new(233, 353));
    let id = driver.stage.events().removal(blocking).unwrap().id;
    // The two compete, and the drawer shows both, the one asked for selected.
    let (ids, selected) = driver.stage.events().rivals_of(id).unwrap();
    assert_eq!(child(&driver), Some(Child::Rivals { ids, selected }));
    assert_eq!(ids[selected], id);
}

/// A request gone by the time VIEW REQUEST is tapped is said to be, and the refusal stays.
#[test]
fn a_refusals_request_gone_when_tapped_is_shown_unavailable() {
    let mut driver = start();
    let now = driver.now();
    sim(&mut driver).request_removal(2, 3, SWITCH_AFTER, now);
    driver.wait(100_000);
    add(&mut driver);
    // The mesh stops showing it before its switch, as leaving the group would.
    sim(&mut driver).view_mut().removals = Default::default();
    driver.wait(100_000);
    tap(&mut driver, Point::new(233, 353));
    assert!(driver.stage.drawer().is_none());
    assert!(
        group_shows(&driver, "THE REQUEST IS NO LONGER LISTED"),
        "{:?}",
        group_shown(&driver)
    );
    tap(&mut driver, Point::new(132, 115));
    assert!(group_shows(&driver, "GROUP"), "{:?}", group_shown(&driver));
}
