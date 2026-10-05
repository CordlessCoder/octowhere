//! The 2026-10-04 runtime screens in each of their states: the Events drawer, its details and
//! management, the toasts and the unread arc, reached by the readings, mesh views and gestures
//! that reach them on the device. Names match the hand-off's targets where one exists.

use embedded_graphics::prelude::Point;
use octowhere_ui::{
    chrome::FB,
    ui::{
        group::view::{RefreshPhase, RefreshView},
        rest::{AlwaysOn, Timeout},
        screens::{Gnss, GnssHealth, PeripheralState, Screen},
        script::Driver,
        stage::{Sensors, Stage},
    },
};

const SECOND: u64 = 1_000_000;
const REFRESH: u64 = 135 * SECOND;

/// The clock face at `now`, awake, with the readings of the other renders.
fn start(timeout: Timeout, always_on: AlwaysOn) -> Driver<'static> {
    let mut driver = Driver::on(Screen::Clock);
    driver.stage = Stage::new(PeripheralState {
        firmware: "0.1.0",
        timeout,
        always_on,
        ..PeripheralState::default()
    });
    driver.stage.show(Screen::Clock);
    driver.sensors(super::sensors());
    driver.wait(600_000);
    driver
}

fn awake() -> Driver<'static> {
    start(Timeout::Never, AlwaysOn::Off)
}

/// The receiver's health, as the GNSS task reports it, with what it last did `ago` before now.
fn health(driver: &mut Driver, recovering: bool, failed_resets: u8, fix: bool) {
    let now = driver.now();
    let sensors = super::sensors();
    driver.sensors(Sensors {
        gnss: Gnss {
            fix,
            health: GnssHealth {
                recovering,
                failed_resets,
                last_response: Some(now.saturating_sub(180 * SECOND)),
                last_fix: Some(now.saturating_sub(300 * SECOND)),
            },
            ..sensors.gnss
        },
        ..sensors
    });
}

/// A refresh in the mesh's view.
fn refresh(driver: &mut Driver, session: u32, phase: RefreshPhase, heard: u32, learned: u32) {
    driver.stage.update_mesh(|mesh| {
        mesh.refresh = Some(RefreshView {
            session,
            phase,
            heard,
            learned,
        });
    });
    driver.wait(50_000);
}

/// A refresh run from start to end: `heard` and `learned` as id sets.
fn refreshed(driver: &mut Driver, session: u32, heard: u32, learned: u32) {
    let until = (driver.now() + REFRESH) as i64;
    refresh(driver, session, RefreshPhase::Listening { until }, 0, 0);
    driver.wait(REFRESH);
    let at = driver.now() as i64;
    refresh(driver, session, RefreshPhase::Ended { at }, heard, learned);
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

/// A fault and two refreshes, two minutes apart, all unread.
fn history() -> Driver<'static> {
    let mut driver = awake();
    refreshed(&mut driver, 1, 0b110, 0);
    driver.wait(120 * SECOND);
    refreshed(&mut driver, 2, 0b1110, 0b1000);
    driver.wait(300 * SECOND);
    health(&mut driver, true, 3, false);
    driver.wait(11 * SECOND);
    driver
}

/// Draws what `driver` shows now, as frame `name`.
fn snap(frames: &mut Vec<(String, Box<FB>)>, name: &str, driver: &Driver) {
    let mut fb = FB::boxed();
    driver.stage.draw(&mut *fb);
    frames.push((name.into(), fb));
}

pub fn frames() -> Vec<(String, Box<FB>)> {
    let mut frames = Vec::new();

    let mut new = history();
    open_drawer(&mut new);
    snap(&mut frames, "events-drawer-new", &new);
    new.swipe(Point::new(233, 380), Point::new(233, 133), 400_000);
    new.wait(300_000);
    snap(&mut frames, "events-drawer-history", &new);

    let mut fault = history();
    open_drawer(&mut fault);
    tap(&mut fault, 233, 170);
    snap(&mut frames, "events-detail-fault", &fault);
    tap(&mut fault, 233, 20);
    tap(&mut fault, 233, 426);
    snap(&mut frames, "events-drawer-manage", &fault);
    tap(&mut fault, 233, 180);
    snap(&mut frames, "events-drawer-read", &fault);
    tap(&mut fault, 233, 426);
    tap(&mut fault, 233, 300);
    snap(&mut frames, "events-drawer-active-after-clear", &fault);

    let mut responding = history();
    health(&mut responding, false, 0, false);
    responding.wait(11 * SECOND);
    open_drawer(&mut responding);
    snap(&mut frames, "events-drawer-responding", &responding);
    tap(&mut responding, 233, 170);
    snap(&mut frames, "events-detail-responding", &responding);

    let mut recovering = awake();
    health(&mut recovering, true, 0, false);
    recovering.wait(11 * SECOND);
    open_drawer(&mut recovering);
    snap(&mut frames, "events-drawer-recovering", &recovering);

    let mut ongoing = awake();
    health(&mut ongoing, true, 3, false);
    ongoing.wait(SECOND);
    let until = (ongoing.now() + REFRESH) as i64;
    refresh(&mut ongoing, 1, RefreshPhase::Listening { until }, 0, 0);
    ongoing.wait(27 * SECOND);
    refresh(
        &mut ongoing,
        1,
        RefreshPhase::Listening { until },
        0b110,
        0b100,
    );
    open_drawer(&mut ongoing);
    snap(&mut frames, "events-drawer-refresh-ongoing", &ongoing);
    tap(&mut ongoing, 233, 170);
    snap(&mut frames, "events-detail-refresh-ongoing", &ongoing);

    let mut ended = awake();
    refreshed(&mut ended, 1, 0b1110, 0b1000);
    ended.wait(300 * SECOND);
    open_drawer(&mut ended);
    tap(&mut ended, 233, 170);
    snap(&mut frames, "events-detail-refresh", &ended);
    tap(&mut ended, 306, 391);
    snap(&mut frames, "events-drawer-empty", &ended);
    ended.swipe(Point::new(380, 260), Point::new(80, 260), 300_000);
    ended.wait(400_000);
    snap(&mut frames, "messages-empty", &ended);

    let mut none = awake();
    refreshed(&mut none, 1, 0, 0);
    none.wait(SECOND);
    snap(&mut frames, "events-toast-quiet-refresh-clock", &none);
    open_drawer(&mut none);
    snap(&mut frames, "events-drawer-no-devices", &none);

    let mut toast = awake();
    health(&mut toast, true, 3, false);
    toast.wait(SECOND);
    snap(&mut frames, "events-toast-fault-clock", &toast);
    toast.wait(5 * SECOND);
    snap(&mut frames, "events-clock-unread", &toast);

    let mut recovering_toast = awake();
    health(&mut recovering_toast, true, 0, false);
    recovering_toast.wait(SECOND);
    snap(
        &mut frames,
        "events-toast-recovering-clock",
        &recovering_toast,
    );

    let mut refresh_toast = awake();
    refreshed(&mut refresh_toast, 1, 0b1110, 0b1000);
    refresh_toast.wait(SECOND);
    snap(&mut frames, "events-toast-refresh-clock", &refresh_toast);

    // Resting on the always-on face, an event wakes it for its toast, and the face it rests
    // on again keeps the unread arc.
    let mut resting = start(Timeout::Seconds15, AlwaysOn::Dim);
    resting.wait(30 * SECOND);
    health(&mut resting, true, 0, false);
    resting.wait(SECOND);
    snap(&mut frames, "events-toast-woke", &resting);
    resting.wait(6 * SECOND);
    snap(&mut frames, "events-aod-unread", &resting);

    frames
}
