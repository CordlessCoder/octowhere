//! The Events drawer, its toasts and the unread arc, driven through the stage by the readings
//! and gestures that reach them on the device, and drawn as the frame loop draws them.

mod common;

use common::{Buffers, differing};
use embedded_graphics::prelude::Point;
use octowhere_ui::{
    chrome::FB,
    ui::{
        drawer::{Child, Root},
        events::{Gnss, Kind},
        group::view::{RefreshPhase, RefreshView},
        rest::{self, AlwaysOn, Rest, Timeout},
        screens::{Gnss as Reading, GnssHealth, PeripheralState, Screen},
        script::Driver,
        second::Page,
        stage::{Input, Sensors, Stage, Update},
    },
};

const SECOND: u64 = 1_000_000;

fn start_with(timeout: Timeout, always_on: AlwaysOn) -> Driver<'static> {
    let mut driver = Driver::on(Screen::Clock);
    driver.stage = Stage::new(PeripheralState {
        timeout,
        always_on,
        ..PeripheralState::default()
    });
    driver.stage.show(Screen::Clock);
    driver.wait(600_000);
    driver
}

fn start() -> Driver<'static> {
    start_with(Timeout::Never, AlwaysOn::Off)
}

/// The receiver's health, as the GNSS task reports it.
fn health(driver: &mut Driver, recovering: bool, failed_resets: u8) -> Update {
    driver.sensors(Sensors {
        gnss: Reading {
            health: GnssHealth {
                recovering,
                failed_resets,
                ..GnssHealth::default()
            },
            ..Reading::default()
        },
        ..Sensors::default()
    })
}

fn refresh(driver: &mut Driver, phase: RefreshPhase) {
    driver.stage.update_mesh(|mesh| {
        mesh.refresh = Some(RefreshView {
            session: 1,
            phase,
            heard: 0b110,
            learned: 0,
        });
    });
    driver.wait(50_000);
}

fn open_drawer(driver: &mut Driver) {
    driver.swipe(Point::new(233, 430), Point::new(233, 120), 250_000);
    driver.settle();
    driver.wait(300_000);
}

fn tap(driver: &mut Driver, x: i32, y: i32) {
    driver.tap(Point::new(x, y));
    driver.wait(300_000);
}

fn child(driver: &Driver) -> Option<Child> {
    driver.stage.drawer().and_then(|drawer| drawer.child())
}

#[test]
fn an_incident_shows_a_toast_whose_tap_opens_its_detail() {
    let mut driver = start();
    health(&mut driver, true, 0);
    let id = driver.stage.toast().expect("the incident is told");
    assert_eq!(driver.stage.events().unread(), 1);
    tap(&mut driver, 233, 360);
    assert_eq!(child(&driver), Some(Child::Event(id)));
    assert_eq!(driver.stage.events().unread(), 0, "opening it read it");
    // Active, it cannot be dismissed.
    tap(&mut driver, 233, 391);
    assert!(driver.stage.events().get(id).is_some());
}

#[test]
fn routine_resets_update_the_incident_without_another_toast() {
    let mut driver = start();
    health(&mut driver, true, 0);
    driver.wait(6 * SECOND);
    assert_eq!(driver.stage.toast(), None, "the toast timed out");
    health(&mut driver, true, 1);
    assert_eq!(driver.stage.toast(), None);
    health(&mut driver, true, 3);
    let id = driver.stage.toast().expect("the fault is told");
    assert_eq!(
        driver.stage.events().get(id).map(|event| event.kind),
        Some(Kind::Gnss(Gnss::Fault { failed: 3 }))
    );
    assert_eq!(driver.stage.events().len(), 1);
}

#[test]
fn an_untouched_toast_rests_the_screen_again() {
    let mut driver = start_with(Timeout::Seconds15, AlwaysOn::Dim);
    driver.wait(25 * SECOND);
    assert_eq!(driver.stage.rest(), Rest::AlwaysOn);
    health(&mut driver, true, 0);
    assert_eq!(driver.stage.rest(), Rest::Awake);
    assert!(driver.stage.toast().is_some());
    driver.wait(5 * SECOND + 100_000);
    assert_eq!(driver.stage.toast(), None);
    assert_eq!(driver.stage.rest(), Rest::AlwaysOn);
    assert_eq!(driver.stage.events().unread(), 1, "a timeout reads nothing");
}

#[test]
fn a_toast_from_off_climbs_once_the_panel_is_on() {
    let mut driver = start_with(Timeout::Seconds15, AlwaysOn::Off);
    driver.wait(25 * SECOND);
    assert_eq!(driver.stage.rest(), Rest::Off);
    let woken = driver.now();
    let update = health(&mut driver, true, 0);
    assert_eq!(update.display_on, Some(true));
    let mut levels: Vec<u8> = update.brightness.into_iter().collect();
    while driver.now() < woken + rest::PANEL_WAKE {
        levels.extend(driver.step(Input::default()).brightness);
    }
    assert!(
        levels.iter().all(|&level| level == 0),
        "the level climbed on a panel still asleep: {levels:?}"
    );
    while driver.stage.is_changing() {
        levels.extend(driver.step(Input::default()).brightness);
    }
    assert_eq!(levels.last(), Some(&120));
}

#[test]
fn a_toast_over_a_dim_wakes_to_the_level_being_edited() {
    let mut driver = start_with(Timeout::Seconds15, AlwaysOn::Off);
    driver.swipe(Point::new(233, 80), Point::new(233, 420), 250_000);
    driver.settle();
    driver.wait(500_000);
    tap(&mut driver, 150, 190);
    assert!(matches!(driver.stage.page(), Some(Page::Brightness(_))));
    driver.swipe(Point::new(150, 280), Point::new(420, 280), 300_000);
    driver.wait(16 * SECOND);
    assert!(matches!(driver.stage.rest(), Rest::Dimmed { .. }));
    let mut levels: Vec<u8> = health(&mut driver, true, 0)
        .brightness
        .into_iter()
        .collect();
    while driver.stage.is_changing() {
        levels.extend(driver.step(Input::default()).brightness);
    }
    assert_eq!(levels.last(), Some(&255), "{levels:?}");
    assert_eq!(driver.stage.peripherals().brightness, 120);
}

#[test]
fn a_toast_waits_for_a_finger_to_lift() {
    let mut driver = start();
    driver.touch(Some(Point::new(100, 233)));
    health(&mut driver, true, 0);
    assert_eq!(driver.stage.toast(), None);
    driver.lift();
    driver.wait(50_000);
    assert!(driver.stage.toast().is_some());
}

#[test]
fn an_upward_drag_opens_events_and_a_pull_from_the_top_closes_it() {
    let mut driver = start();
    open_drawer(&mut driver);
    let drawer = driver.stage.drawer().expect("the drawer is open");
    assert_eq!(drawer.root(), Root::Events);
    driver.swipe(Point::new(233, 200), Point::new(233, 420), 250_000);
    driver.settle();
    driver.wait(300_000);
    assert!(driver.stage.drawer().is_none());
}

#[test]
fn the_chevron_closes_and_a_sideways_swipe_pages_to_messages() {
    let mut driver = start();
    open_drawer(&mut driver);
    driver.swipe(Point::new(380, 260), Point::new(80, 260), 300_000);
    driver.wait(400_000);
    assert_eq!(
        driver.stage.drawer().map(|drawer| drawer.root()),
        Some(Root::Messages)
    );
    driver.swipe(Point::new(80, 260), Point::new(380, 260), 300_000);
    driver.wait(400_000);
    assert_eq!(
        driver.stage.drawer().map(|drawer| drawer.root()),
        Some(Root::Events)
    );
    tap(&mut driver, 233, 26);
    driver.settle();
    driver.wait(300_000);
    assert!(driver.stage.drawer().is_none());
}

#[test]
fn a_resolved_incident_dismisses_and_the_list_empties() {
    let mut driver = start();
    health(&mut driver, true, 3);
    health(&mut driver, false, 0);
    driver.wait(6 * SECOND);
    open_drawer(&mut driver);
    tap(&mut driver, 233, 170);
    assert!(matches!(child(&driver), Some(Child::Event(_))));
    tap(&mut driver, 233, 426);
    assert_eq!(child(&driver), None, "back on Events");
    assert!(driver.stage.events().is_empty());
}

#[test]
fn a_running_refresh_is_kept_by_clearing_and_dismissing() {
    let mut driver = start();
    let until = (driver.now() + 135 * SECOND) as i64;
    refresh(&mut driver, RefreshPhase::Listening { until });
    assert_eq!(
        driver.stage.toast(),
        None,
        "a refresh started here is not told"
    );
    assert_eq!(driver.stage.events().ongoing(), 1);
    open_drawer(&mut driver);
    tap(&mut driver, 233, 426);
    tap(&mut driver, 233, 300);
    assert_eq!(driver.stage.events().len(), 1);
    assert_eq!(child(&driver), Some(Child::Manage), "nothing to clear");
    tap(&mut driver, 233, 426);
    tap(&mut driver, 233, 170);
    tap(&mut driver, 306, 391);
    assert_eq!(driver.stage.events().len(), 1);
    driver.wait(135 * SECOND);
    let at = driver.now() as i64;
    refresh(&mut driver, RefreshPhase::Ended { at });
    assert_eq!(driver.stage.events().ongoing(), 0);
    tap(&mut driver, 306, 391);
    assert!(driver.stage.events().is_empty());
}

/// Drawing only each step's damage, in two buffers as the frame loop does, must leave the panel
/// as drawing whole would, through toasts, the arc, and the drawer's opening, scrolling, pages
/// and closing.
#[test]
fn the_drawers_damage_redraws_what_changed() {
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
    let until = (driver.now() + 135 * SECOND) as i64;
    refresh(&mut driver, RefreshPhase::Listening { until });
    health(&mut driver, true, 0);
    driver.wait(2 * SECOND);
    health(&mut driver, true, 3);
    driver.wait(6 * SECOND);
    open_drawer(&mut driver);
    driver.swipe(Point::new(233, 380), Point::new(233, 200), 400_000);
    driver.wait(1_200_000);
    tap(&mut driver, 233, 170);
    driver.wait(1_100_000);
    tap(&mut driver, 233, 26);
    driver.swipe(Point::new(380, 260), Point::new(80, 260), 300_000);
    driver.wait(400_000);
    driver.swipe(Point::new(80, 260), Point::new(380, 260), 300_000);
    driver.wait(400_000);
    driver.swipe(Point::new(233, 200), Point::new(233, 420), 250_000);
    driver.settle();
    driver.wait(SECOND);
    health(&mut driver, false, 0);
    driver.wait(6 * SECOND);
}
