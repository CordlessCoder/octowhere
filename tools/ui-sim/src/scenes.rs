//! Scenes for `--play` and `--record`: the screens driven on a fixed timeline with synthetic
//! readings, so a recording comes out the same every run.

use embedded_graphics::prelude::Point;

use crate::{
    buttons::{self, Button},
    caption::say,
};
use octowhere_ui::ui::{
    clock::{ClockState, DateTime, ZoneMode, ZoneState},
    compass::CompassView,
    gesture::Micros,
    group::{
        keyboard::{Mode, taps},
        sim::{self, Arrival, Sim},
        view::{Carriage, GroupView, IDS, MemberView, MessagesView, Name, Position},
    },
    panel::Cell,
    rest::{AlwaysOn, Timeout},
    screens::PeripheralState,
    screens::{Battery, Gnss, Screen},
    script::{self, Driver},
    second::Store,
    stage::Stage,
    stage::{Key, Motion, Sensors},
    startup::{Outcome, Part, Report},
};

pub struct Scene {
    pub name: &'static str,
    pub about: &'static str,
    pub run: fn(&mut Driver),
    /// Whether a recording adds a column for what the scene says with [`say`].
    pub captioned: bool,
}

pub const SCENES: &[Scene] = &[
    Scene {
        name: "startup",
        about: "the self-test as each part answers, the identity, the logo card and the clock, charging",
        run: startup,
        captioned: false,
    },
    Scene {
        name: "startup-unplugged",
        about: "the start-up into the clock with the battery not charging",
        run: startup_unplugged,
        captioned: false,
    },
    Scene {
        name: "startup-failed",
        about: "the self-test with the magnetometer failing, the fault screen and the clock",
        run: startup_failed,
        captioned: false,
    },
    Scene {
        name: "startup-radio-failed",
        about: "the self-test scrolling to a radio that does not answer, and its fault screen",
        run: startup_radio_failed,
        captioned: false,
    },
    Scene {
        name: "startup-radio-slow",
        about: "the self-test scrolling to a radio that takes its whole deadline to answer",
        run: startup_radio_slow,
        captioned: false,
    },
    Scene {
        name: "startup-power-failed",
        about: "the self-test with POWER failing, scrolled out of view by the radio and back",
        run: startup_power_failed,
        captioned: false,
    },
    Scene {
        name: "startup-power-radio-failed",
        about: "the self-test with POWER and the radio failing, and their fault screen",
        run: startup_power_radio_failed,
        captioned: false,
    },
    Scene {
        name: "clock-entry",
        about: "the clock face building in, then ticking",
        run: clock_entry,
        captioned: false,
    },
    Scene {
        name: "clock-charging",
        about: "the clock's battery: plugged in, the slices' loop, unplugged, and a plug-in cut short",
        run: clock_charging,
        captioned: false,
    },
    Scene {
        name: "pixel-shift",
        about: "the picture moving a step against burn-in as each page and the panel settle",
        run: pixel_shift,
        captioned: true,
    },
    Scene {
        name: "swipe-to-compass",
        about: "from the clock to the compass, a turn, and back",
        run: swipe_to_compass,
        captioned: false,
    },
    Scene {
        name: "compass-calibration",
        about: "the compass calibrating, finding its heading, turning, and meeting interference",
        run: compass_calibration,
        captioned: false,
    },
    Scene {
        name: "compass-states",
        about: "each change between the compass's states",
        run: compass_states,
        captioned: false,
    },
    Scene {
        name: "settings",
        about: "the settings panel: opening, scrolling, brightness, timeout, always on, the zone picker, and closing",
        run: settings,
        captioned: false,
    },
    Scene {
        name: "zone-scroll",
        about: "the zone picker on a name too long for its slab, which scrolls to its end and back",
        run: zone_scroll,
        captioned: false,
    },
    Scene {
        name: "rest-always-on",
        about: "a 15 s timeout: the dim, the always-on face, and a double tap back to the clock",
        run: rest_always_on,
        captioned: false,
    },
    Scene {
        name: "rest-off",
        about: "a 15 s timeout on the compass: the dim, the panel off, and a double tap back",
        run: rest_off,
        captioned: false,
    },
    Scene {
        name: "power-off",
        about: "the power key's confirmation: a slide let go short, a cancel, a wake from dark onto it, and a slide that powers off",
        run: power_off,
        captioned: false,
    },
    Scene {
        name: "power-off-dim-cancel",
        about: "a 15 s timeout dims the clock, the power key opens the confirmation over the dim, and a cancel lets it dim and go dark",
        run: power_off_dim_cancel,
        captioned: false,
    },
    Scene {
        name: "drawer",
        about: "the Events drawer opened over the clock, a whole 10 s breathing cycle at rest, its list scrolled, Messages beside it, and closed",
        run: drawer,
        captioned: true,
    },
    Scene {
        name: "member-face",
        about: "the member face reached from the clock, turning with the heading, and a member selected",
        run: member_face,
        captioned: true,
    },
    Scene {
        name: "crowded-sectors",
        about: "a crowded member face: runs of members as sectors, turning with the heading, and the selection stepping through them at each member's own bearing",
        run: crowded_sectors,
        captioned: true,
    },
    Scene {
        name: "long-message",
        about: "a message too tall for its conversation, unread after a quick look at its end, and read once every line has shown for a second",
        run: long_message,
        captioned: true,
    },
    Scene {
        name: "messages",
        about: "a private message arriving over the clock, read in its conversation, and a reply written, reviewed, sent and delivered",
        run: messages,
        captioned: true,
    },
    Scene {
        name: "removal",
        about: "removing a member with the slide and its countdown, then another member's request declined with the slide",
        run: removal,
        captioned: true,
    },
    Scene {
        name: "tour",
        about: "every screen and state, slowly: the start-up, the clock, the battery, the compass, settings, the always-on face and a demonstrated failure",
        run: tour,
        captioned: true,
    },
];

pub fn find(name: &str) -> &'static Scene {
    SCENES
        .iter()
        .find(|scene| scene.name == name)
        .unwrap_or_else(|| {
            let names: Vec<_> = SCENES.iter().map(|scene| scene.name).collect();
            panic!("no scene {name}; there are {}", names.join(", "))
        })
}

const fn ms(milliseconds: u64) -> Micros {
    milliseconds * 1_000
}

/// 13:07:42 on Thu 24 Sep 2026 in Europe/Dublin, found automatically, with GNSS having set the
/// clock.
fn dublin() -> Sensors {
    Sensors {
        clock: ClockState {
            utc: Some(
                DateTime {
                    year: 2026,
                    month: 9,
                    day: 24,
                    hour: 12,
                    minute: 7,
                    second: 42,
                }
                .to_unix(),
            ),
            set_from_gnss: true,
            stopped: false,
        },
        zone: ZoneState {
            mode: ZoneMode::Automatic,
            zone: octowhere_ui::tz::DATABASE
                .find("Europe/Dublin")
                .map(|zone| zone.id),
        },
        battery: Some(Battery {
            present: true,
            percent: 87,
            millivolts: 4020,
            charging: true,
            usb: true,
        }),
        gnss: Gnss {
            fix: true,
            in_use: 9,
            in_view: 14,
            position: Some((533_498_000, -62_603_000)),
            ..Gnss::default()
        },
    }
}

/// Calibrated and level, facing `degrees`.
fn facing(degrees: f32) -> Motion {
    Motion {
        compass: CompassView {
            live: true,
            calibration_percent: 100,
            heading_decidegrees: Some((degrees.rem_euclid(360.0) * 10.0).round() as u16 % 3600),
            ..CompassView::default()
        },
    }
}

fn ease(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// A turn by hand: quick at first, a little past the mark, then settling back onto it.
fn swing(t: f32) -> f32 {
    const DAMPING: f32 = 6.0;
    const FREQUENCY: f32 = 7.0;
    let decay = (-DAMPING * t).exp();
    1.0 - decay * ((FREQUENCY * t).cos() + DAMPING / FREQUENCY * (FREQUENCY * t).sin())
}

/// A device held in the hand: a slow drift of pitch and roll about the reading, and of the
/// heading by a degree or so, at unrelated periods so it never visibly repeats.
fn hand(mut motion: Motion, now: Micros) -> Motion {
    let wave = |period: f32, phase: f32| {
        (std::f32::consts::TAU * now as f32 / (period * 1e6) + phase).sin()
    };
    let compass = &mut motion.compass;
    if compass.live {
        compass.pitch_deg = compass
            .pitch_deg
            .saturating_add((3.0 + 2.5 * wave(3.7, 0.0)).round() as i8);
        compass.roll_deg = compass
            .roll_deg
            .saturating_add((-2.0 + 3.5 * wave(5.3, 1.0)).round() as i8);
        compass.heading_decidegrees = compass.heading_decidegrees.map(|heading| {
            let drift = (12.0 * wave(4.3, 2.0) + 5.0 * wave(1.9, 0.5)).round() as i32;
            (i32::from(heading) + drift).rem_euclid(3600) as u16
        });
    }
    motion
}

/// Starts on `screen` with the clock running in Dublin, facing 37°.
fn start(driver: &mut Driver, screen: Screen) {
    driver.stage.show(screen);
    driver.run_clock();
    driver.hold(hand);
    driver.sensors(dublin());
    driver.motion(facing(37.0));
}

fn page_left(driver: &mut Driver, duration: Micros) {
    driver.swipe(Point::new(420, 233), Point::new(60, 233), duration);
    driver.settle();
}

fn page_right(driver: &mut Driver, duration: Micros) {
    driver.swipe(Point::new(60, 233), Point::new(420, 233), duration);
    driver.settle();
}

fn clock_entry(driver: &mut Driver) {
    start(driver, Screen::Clock);
    driver.wait(ms(3_000));
}

/// The battery at `percent`, on USB and charging or not.
fn on_battery(driver: &mut Driver, percent: u8, charging: bool) {
    let mut sensors = dublin_now(driver);
    sensors.battery = Some(Battery {
        present: true,
        percent,
        millivolts: 3_900,
        charging,
        usb: charging,
    });
    driver.sensors(sensors);
}

fn clock_charging(driver: &mut Driver) {
    start(driver, Screen::Clock);
    on_battery(driver, 87, false);
    driver.wait(ms(1_500));
    // The build, then a whole loop: both beats dock and split.
    on_battery(driver, 87, true);
    driver.wait(ms(6_500));
    // The build runs back, and the solid starts out from the middle.
    on_battery(driver, 87, false);
    driver.wait(ms(1_050));
    // Plugged in again halfway through the wipe: the solid turns back and the slices build in.
    on_battery(driver, 87, true);
    driver.wait(ms(2_000));
    on_battery(driver, 87, false);
    driver.wait(ms(2_000));
}

fn pixel_shift(driver: &mut Driver) {
    start(driver, Screen::Clock);
    driver.wait(ms(1_500));
    say("A NEW PAGE SETTLING MOVES THE PICTURE A STEP. THE BAND STILL MEETS THE GLASS.");
    page_left(driver, ms(250));
    driver.wait(ms(1_500));
    page_right(driver, ms(250));
    driver.wait(ms(1_500));
    say("SO DOES THE PANEL SETTLING OPEN OR SHUT.");
    open_settings(driver);
    close_settings(driver);
}

fn swipe_to_compass(driver: &mut Driver) {
    start(driver, Screen::Clock);
    driver.wait(ms(1_200));
    page_left(driver, ms(250));
    driver.wait(ms(800));
    driver.motion_over(ms(700), |t| facing(37.0 + 50.0 * swing(t)));
    driver.wait(ms(800));
    page_right(driver, ms(250));
    driver.wait(ms(1_000));
}

/// Live, calibrating, with no heading yet.
fn calibrating(percent: u8) -> Motion {
    Motion {
        compass: CompassView {
            live: true,
            calibration_percent: percent,
            heading_decidegrees: None,
            ..CompassView::default()
        },
    }
}

/// From calibration starting, through finding a heading and turning, to interference and back.
fn compass_walk(driver: &mut Driver) {
    driver.wait(ms(600));
    driver.motion_over(ms(1_500), |t| calibrating((t * 99.0) as u8));
    driver.wait(ms(300));
    driver.motion(facing(212.0));
    driver.wait(ms(1_000));
    driver.motion_over(ms(900), |t| facing(212.0 + 70.0 * swing(t)));
    driver.wait(ms(700));
    let mut disturbed = facing(282.0);
    disturbed.compass.disturbed = true;
    driver.motion(disturbed);
    driver.wait(ms(1_200));
    driver.motion(facing(282.0));
    driver.wait(ms(1_000));
}

fn compass_calibration(driver: &mut Driver) {
    driver.stage.show(Screen::Compass);
    driver.hold(hand);
    driver.motion(calibrating(0));
    compass_walk(driver);
}

/// Calibrated, with the top edge raised too near vertical for a heading.
fn hold_level() -> Motion {
    Motion {
        compass: CompassView {
            live: true,
            calibration_percent: 100,
            heading_decidegrees: None,
            pitch_deg: 84,
            ..CompassView::default()
        },
    }
}

/// Facing `degrees` with the top edge raised `pitch` degrees above level. The heading goes
/// above 78°, where the compass loses it 11.5° from vertical.
fn raised(degrees: f32, pitch: f32) -> Motion {
    let mut motion = if pitch > 78.0 {
        hold_level()
    } else {
        facing(degrees)
    };
    motion.compass.pitch_deg = pitch.round() as i8;
    motion
}

fn disturbed(degrees: f32) -> Motion {
    let mut motion = facing(degrees);
    motion.compass.disturbed = true;
    motion
}

/// A row of each kind in `SCREEN-DESIGN-BRIEF.md`'s table of the compass's changes of state.
fn compass_states(driver: &mut Driver) {
    driver.stage.show(Screen::Compass);
    driver.hold(hand);
    driver.motion(calibrating(80));
    driver.wait(ms(600));
    driver.motion_over(ms(600), |t| calibrating(80 + (t * 19.0) as u8));
    // Calibration completes with the top edge raised, then the device is laid level.
    driver.motion(hold_level());
    driver.wait(ms(1_000));
    driver.motion(facing(120.0));
    driver.wait(ms(1_000));
    // Raised briefly, inside the grace, and then for longer than it.
    driver.motion(hold_level());
    driver.wait(ms(400));
    driver.motion(facing(120.0));
    driver.wait(ms(800));
    driver.motion(hold_level());
    driver.wait(ms(1_200));
    driver.motion(facing(120.0));
    driver.wait(ms(1_000));
    // Interference as the motion task's holds let it through: shown at least a second.
    driver.motion(disturbed(120.0));
    driver.wait(ms(1_200));
    driver.motion(facing(120.0));
    driver.wait(ms(800));
    driver.motion(Motion {
        compass: CompassView::default(),
    });
    driver.wait(ms(1_000));
    // Calibration never falls on the device, but going through NO DATA shows both of its rows.
    driver.motion(calibrating(96));
    driver.wait(ms(1_000));
    driver.motion(facing(120.0));
    driver.wait(ms(1_200));
    driver.motion(Motion {
        compass: CompassView::default(),
    });
    driver.wait(ms(1_000));
    driver.motion(facing(120.0));
    driver.wait(ms(1_200));
}

/// How long the tour holds a state for a viewer to read it.
const HOLD: Micros = ms(3_500);
/// How long the tour's drags take.
const SLOW: Micros = ms(700);
/// How long the tour's page and panel swipes take.
const SWIPE: Micros = ms(280);

/// Dublin's readings at the driver's time, as the clock started by `boot` has run to.
fn dublin_now(driver: &Driver) -> Sensors {
    let mut sensors = dublin();
    sensors.clock.utc = sensors
        .clock
        .utc
        .map(|utc| utc + (driver.now() / 1_000_000) as i64);
    sensors
}

/// Says `caption`, steps in Dublin's readings at the driver's time changed by `change`, and
/// holds them.
fn show(driver: &mut Driver, caption: &'static str, change: impl FnOnce(&mut Sensors)) {
    say(caption);
    let mut sensors = dublin_now(driver);
    change(&mut sensors);
    driver.sensors(sensors);
    driver.wait(HOLD);
}

/// Steps the battery from `from` to `to` percent a percent at a time. The tour moves every
/// continuous reading this way rather than jumping it.
fn drain(driver: &mut Driver, from: u8, to: u8, charging: bool) {
    let step = if to < from { -1 } else { 1 };
    let mut percent = i16::from(from);
    while percent != i16::from(to) {
        percent += step;
        let mut sensors = dublin_now(driver);
        sensors.battery = Some(Battery {
            present: true,
            percent: percent as u8,
            millivolts: 3_900,
            charging,
            usb: charging,
        });
        driver.sensors(sensors);
        driver.wait(ms(40));
    }
}

/// A tap, and a pause to see what it did.
/// Presses `button` as a finger does, so a recording shows it held: a short press reaches the
/// stage as the key comes back up, a long one once it has been held for the second the power
/// controller is set to, and the finger lets go a little after.
fn press(driver: &mut Driver, button: Button, key: Key) {
    buttons::hold(button, true);
    driver.wait(match key {
        Key::Short => ms(150),
        Key::Long => ms(1_000),
    });
    if let Key::Short = key {
        buttons::hold(button, false);
    }
    match button {
        Button::Power => driver.key(key),
        Button::Boot => driver.boot_key(key),
    };
    if let Key::Long = key {
        driver.wait(ms(300));
        buttons::hold(button, false);
    }
}

/// Drags a finger from `from` through each of `waypoints`, reaching each over its time and
/// staying put on one that repeats the point before, then lifts it.
fn drag(driver: &mut Driver, from: Point, waypoints: &[(Point, Micros)]) {
    let mut path = vec![from];
    let mut at = from;
    for &(to, duration) in waypoints {
        let steps = (duration / script::FRAME).max(1) as i32;
        path.extend((1..=steps).map(|step| at + (to - at) * step / steps));
        at = to;
    }
    driver.stroke(&path);
}

fn slow_tap(driver: &mut Driver, x: i32, y: i32) {
    driver.tap(Point::new(x, y));
    driver.wait(ms(1_500));
}

/// Taps a row of the settings panel, turning to its page first.
fn tap_row(driver: &mut Driver, cell: Cell) {
    if !octowhere_ui::ui::panel::in_view(cell.page(), driver.stage.panel_scroll()) {
        let (from, to) = if cell.page() == 1 {
            (380, 80)
        } else {
            (80, 380)
        };
        driver.swipe(Point::new(from, 250), Point::new(to, 250), SWIPE);
        driver.settle();
        driver.wait(ms(1_200));
    }
    let middle = cell.bounds(driver.stage.panel_scroll()).center();
    slow_tap(driver, middle.x, middle.y);
}

fn open_settings(driver: &mut Driver) {
    driver.swipe(Point::new(233, 70), Point::new(233, 420), SWIPE);
    driver.settle();
    driver.wait(ms(2_000));
}

fn close_settings(driver: &mut Driver) {
    driver.swipe(Point::new(233, 420), Point::new(233, 80), SWIPE);
    driver.settle();
    driver.wait(ms(2_000));
}

/// Every screen and state, paced for a viewer who has not seen the device.
fn tour(driver: &mut Driver) {
    say("SELF-TEST. EACH PART OF THE BOARD IS TICKED OFF AS IT ANSWERS.");
    boot(driver, &ANSWERING);
    driver.motion(facing(37.0));
    driver.wait(ms(1_700).saturating_sub(driver.now()));
    say("EVERY PART ANSWERED, SO THE IDENTITY AND THE LOGO CARD PLAY.");
    while driver.stage.starting_up() {
        driver.wait(ms(100));
    }
    say(
        "THE CLOCK FACE. THE BLUE ICON MEANS GNSS SET THE TIME, AND THE ZONE CAME FROM THE POSITION.",
    );
    driver.wait(ms(5_000));

    show(
        driver,
        "RTC. NO FIX SINCE START-UP, SO THE TIME IS THE BOARD'S OWN CLOCK.",
        |s| {
            s.clock.set_from_gnss = false;
        },
    );
    show(
        driver,
        "MANUAL. A ZONE CHOSEN BY HAND IN SETTINGS, HERE NEW YORK.",
        |s| {
            s.zone = ZoneState {
                mode: ZoneMode::Manual,
                zone: octowhere_ui::tz::DATABASE
                    .find("America/New_York")
                    .map(|zone| zone.id),
            };
        },
    );
    show(
        driver,
        "STOPPED. THE CLOCK STOPPED SINCE IT WAS LAST SET, SO ITS TIME IS UNRELIABLE.",
        |s| {
            s.clock.stopped = true;
        },
    );
    show(
        driver,
        "NO ZONE. NO FIX HAS PLACED THE DEVICE YET, SO THE FACE SHOWS UTC.",
        |s| {
            s.zone = ZoneState::default();
        },
    );
    show(driver, "NO DATA. THE CLOCK COULD NOT BE READ.", |s| {
        s.clock.utc = None
    });
    show(driver, "BACK TO A GNSS FIX.", |_| {});
    // Keeps the minute's timeout from dimming the screen before the swipe to the compass.
    say("A TAP ON THE CLOCK FACE DOES NOTHING BUT KEEP THE SCREEN AWAKE.");
    slow_tap(driver, 233, 400);

    let battery = |percent, charging| {
        Some(Battery {
            present: true,
            percent,
            millivolts: 3_900,
            charging,
            usb: charging,
        })
    };
    show(
        driver,
        "THE BATTERY, UNDER THE BAND'S LINES. OFF USB, THE FILL GOES SOLID.",
        |s| {
            s.battery = battery(87, false);
        },
    );
    say("ON BATTERY ALONE, THE FILL FALLS WITH THE CHARGE, HERE TO 64%.");
    drain(driver, 87, 64, false);
    driver.wait(HOLD);
    show(
        driver,
        "PLUGGED IN. THE SOLID DRAINS DOWN TO SLICES LIKE THE IDENTITY'S BARCODE, WHICH GATHER AND REGROUP, THEN REST FOR FOUR SECONDS.",
        |s| {
            s.battery = battery(64, true);
        },
    );
    driver.wait(ms(2_500));
    show(
        driver,
        "UNPLUGGED. THE SOLID RISES BACK OVER THE SLICES.",
        |s| s.battery = battery(64, false),
    );
    say("LOW BATTERY, AT 15% OR LESS.");
    drain(driver, 64, 12, false);
    driver.wait(HOLD);
    show(driver, "LOW AND CHARGING: FEWER SLICES.", |s| {
        s.battery = battery(12, true)
    });
    show(driver, "NO BATTERY READING. NO FILL, A GRAY HATCH.", |s| {
        s.battery = None
    });
    show(driver, "BACK ON USB, CHARGING.", |_| {});

    say("SWIPE LEFT FOR THE COMPASS.");
    page_left(driver, SWIPE);
    say("THE HEADING. THE DIAL TURNS WITH THE DEVICE OVER A STILL BLUE FIELD.");
    driver.wait(ms(2_000));
    driver.motion_over(ms(2_000), |t| facing(37.0 + 90.0 * swing(t)));
    driver.wait(HOLD);
    say("CALIBRATING. TURN THE DEVICE EVERY WAY UNTIL THE COUNT FILLS.");
    driver.motion(calibrating(0));
    driver.wait(ms(1_500));
    driver.motion_over(ms(3_000), |t| calibrating((t * 99.0) as u8));
    driver.wait(ms(1_500));
    say("CALIBRATED. THE HEADING RETURNS.");
    driver.motion(facing(127.0));
    driver.wait(HOLD);
    say("INTERFERENCE. A MAGNETIC DISTURBANCE MAKES THE HEADING UNRELIABLE.");
    driver.motion(disturbed(127.0));
    driver.wait(HOLD);
    driver.motion(facing(127.0));
    driver.wait(ms(2_000));
    say("HOLD LEVEL. STOOD ON AN EDGE, THE DIAL NO LONGER LIES FLAT, SO THE HEADING GOES.");
    driver.motion_over(ms(1_500), |t| raised(127.0, 84.0 * ease(t)));
    driver.wait(HOLD);
    driver.motion_over(ms(1_500), |t| raised(127.0, 84.0 * (1.0 - ease(t))));
    driver.wait(ms(2_000));
    say("NO DATA. THE MOTION SENSORS STOPPED ANSWERING.");
    driver.motion(Motion {
        compass: CompassView::default(),
    });
    driver.wait(HOLD);
    say("THEY ANSWER AGAIN.");
    driver.motion(facing(127.0));
    driver.wait(HOLD);
    say("SWIPE RIGHT FOR THE CLOCK.");
    page_right(driver, SWIPE);
    driver.wait(ms(2_000));

    say("DRAG DOWN FROM EITHER FACE FOR SETTINGS.");
    open_settings(driver);
    say("BRIGHTNESS. THE PANEL FOLLOWS THE DRAG, AND A TAP KEEPS THE LEVEL.");
    tap_row(driver, Cell::Brightness);
    driver.swipe(Point::new(330, 280), Point::new(150, 280), ms(1_500));
    driver.wait(ms(1_500));
    slow_tap(driver, 233, 378);
    say("ZONE. DRAG TO AN OFFSET, TAP FOR ITS ZONES, AND TAP ONE TO SET IT BY HAND.");
    tap_row(driver, Cell::Zone);
    driver.swipe(Point::new(233, 300), Point::new(233, 255), SLOW);
    driver.wait(ms(1_500));
    slow_tap(driver, 233, 250);
    // The sensor task carries the stored zone from then on, as the firmware's does.
    let stored = driver
        .tap(Point::new(233, 250))
        .iter()
        .find_map(|update| update.store);
    let Some(Store::ManualZone(zone)) = stored else {
        panic!("no zone was stored: {stored:?}")
    };
    let zone = ZoneState {
        mode: ZoneMode::Manual,
        zone: Some(zone),
    };
    let mut sensors = dublin_now(driver);
    sensors.zone = zone;
    driver.sensors(sensors);
    driver.wait(ms(1_500));
    say("THE CLOCK NOW SHOWS AMSTERDAM'S TIME, MARKED MANUAL.");
    close_settings(driver);
    driver.wait(HOLD);
    say("TIMEOUT, SET TO 15 SECONDS, AND ALWAYS ON, AT THE DIM LEVEL.");
    open_settings(driver);
    tap_row(driver, Cell::Timeout);
    driver.swipe(Point::new(233, 250), Point::new(233, 360), SLOW);
    driver.wait(ms(1_500));
    slow_tap(driver, 233, 258);
    tap_row(driver, Cell::AlwaysOn);
    driver.stroke(&[
        Point::new(233, 300),
        Point::new(233, 280),
        Point::new(233, 255),
    ]);
    driver.wait(ms(1_000));
    slow_tap(driver, 233, 258);
    close_settings(driver);

    // The idle seconds and the dim's hold play at three times speed; the fade to the always-on
    // face does not.
    say("15 SECONDS UNTOUCHED, AT THREE TIMES SPEED.");
    crate::caption::fast_forward(3);
    // Closing settings already waited two of them.
    driver.wait(ms(12_500));
    say("THE SCREEN DIMS FOR 5 SECONDS, AT THREE TIMES SPEED.");
    driver.wait(ms(4_700));
    crate::caption::fast_forward(1);
    say("THEN IT RESTS ON THE ALWAYS-ON FACE.");
    driver.wait(ms(3_800));
    say("THE ALWAYS-ON FACE: THE TIME AND THE BATTERY, REDRAWN ONCE A MINUTE.");
    driver.wait(HOLD);
    show(driver, "ALWAYS ON, STOPPED.", |s| {
        s.zone = zone;
        s.clock.stopped = true;
    });
    show(driver, "ALWAYS ON, NO ZONE, IN UTC.", |s| {
        s.zone = ZoneState::default()
    });
    show(driver, "ALWAYS ON, NO DATA.", |s| s.clock.utc = None);
    show(driver, "ALWAYS ON, LOCAL TIME AGAIN.", |s| s.zone = zone);
    say("A DOUBLE TAP WAKES THE SCREEN. A SINGLE TAP OR A SWIPE DOES NOT.");
    driver.double_tap();
    driver.wait(ms(2_000));

    say("THE ZONE BACK TO AUTOMATIC, FROM THE GNSS POSITION.");
    open_settings(driver);
    tap_row(driver, Cell::Zone);
    driver.tap(Point::new(233, 342));
    let sensors = dublin_now(driver);
    driver.sensors(sensors);
    driver.wait(ms(1_500));
    close_settings(driver);
    driver.wait(HOLD);
    say("DEVICE. AT THE END OF ITS PAGE, REPLAY START-UP.");
    open_settings(driver);
    tap_row(driver, Cell::Device);
    driver.swipe(Point::new(233, 440), Point::new(233, 80), ms(1_200));
    driver.wait(ms(2_000));
    slow_tap(driver, 233, 298);
    say("A REPLAY CAN DEMONSTRATE A PART FAILING. HERE, THE MAGNETOMETER.");
    driver.swipe(
        Point::new(233, 330),
        Point::new(233, 330 - 40 * 5 - 10),
        ms(1_500),
    );
    driver.wait(ms(2_000));
    driver.tap(Point::new(233, 258));
    say(
        "THE SELF-TEST MARKS THE PART FAILED, AND THE FAULT SCREEN NAMES IT. BOTH SAY IT IS A DEMO.",
    );
    // The last report at 1.3 s, its glyph and the 300 ms hold, then the fault screen's 4 s.
    driver.wait(ms(3_000));
    say("THE TICKER TURNS BETWEEN THE FAILED PART IN BLUE AND FAULT IN YELLOW.");
    driver.wait(ms(2_700));
    say("AFTER FOUR SECONDS THE FAULT SCREEN BREAKS UP AND LIFTS AWAY.");
    while driver.stage.starting_up() {
        driver.wait(ms(100));
    }
    say("THEN THE CLOCK, AS AFTER ANY START-UP.");
    driver.wait(ms(4_000));
    say("A PRESS OF PWR RESTS THE SCREEN AT ONCE, HERE ON THE ALWAYS-ON FACE.");
    press(driver, Button::Power, Key::Short);
    driver.wait(ms(3_500));
    say("ANOTHER PRESS WAKES IT.");
    press(driver, Button::Power, Key::Short);
    driver.wait(ms(3_000));
    say("HELD FOR A SECOND, PWR ASKS WHETHER TO POWER OFF.");
    press(driver, Button::Power, Key::Long);
    driver.wait(ms(2_000));
    say("SLIDING THE HANDLE ACROSS POWERS THE DEVICE OFF.");
    driver.swipe(Point::new(113, 258), Point::new(400, 258), ms(800));
    driver.wait(ms(2_500));
}

fn settings(driver: &mut Driver) {
    driver.stage = Stage::new(PeripheralState {
        firmware: "0.1.0",
        ..PeripheralState::default()
    });
    start(driver, Screen::Clock);
    driver.motion(calibrating(54));
    driver.wait(ms(1_200));
    settings_walk(driver);
}

/// The picker opened on Port-au-Prince, whose name and identifier overhang the slab.
fn zone_scroll(driver: &mut Driver) {
    driver.stage = Stage::new(PeripheralState {
        firmware: "0.1.0",
        ..PeripheralState::default()
    });
    start(driver, Screen::Clock);
    let mut sensors = dublin();
    sensors.zone = ZoneState {
        mode: ZoneMode::Manual,
        zone: octowhere_ui::tz::DATABASE
            .find("America/Port-au-Prince")
            .map(|zone| zone.id),
    };
    driver.sensors(sensors);
    driver.wait(ms(1_200));
    open_settings(driver);
    tap_row(driver, Cell::Zone);
    slow_tap(driver, 233, 250);
    driver.wait(ms(8_000));
}

/// From the clock face into the settings panel, through brightness and the zone picker, and
/// back to the face.
fn settings_walk(driver: &mut Driver) {
    // Down from the face, across the two settings pages and back.
    driver.swipe(Point::new(233, 70), Point::new(233, 420), ms(300));
    driver.settle();
    driver.wait(ms(900));
    driver.swipe(Point::new(380, 250), Point::new(80, 250), ms(300));
    driver.settle();
    driver.wait(ms(800));
    driver.swipe(Point::new(80, 250), Point::new(380, 250), ms(300));
    driver.settle();
    driver.wait(ms(600));
    // Brightness to 80 %, kept.
    driver.tap(Point::new(150, 190));
    driver.wait(ms(700));
    driver.swipe(Point::new(220, 285), Point::new(333, 285), ms(500));
    driver.wait(ms(600));
    driver.tap(Point::new(233, 250));
    driver.wait(ms(700));
    // The timeout to 5 MIN, kept, then ALWAYS ON on.
    driver.tap(Point::new(300, 260));
    driver.wait(ms(700));
    driver.swipe(Point::new(233, 300), Point::new(233, 250), ms(300));
    driver.wait(ms(600));
    driver.tap(Point::new(233, 250));
    driver.wait(ms(700));
    driver.tap(Point::new(300, 330));
    driver.wait(ms(700));
    // The zone picker: one offset later, its zones, and back out.
    driver.tap(Point::new(150, 115));
    driver.wait(ms(800));
    driver.swipe(Point::new(233, 300), Point::new(233, 255), ms(300));
    driver.wait(ms(600));
    driver.tap(Point::new(233, 250));
    driver.wait(ms(900));
    driver.tap(Point::new(120, 120));
    driver.wait(ms(500));
    driver.tap(Point::new(120, 120));
    driver.wait(ms(600));
    // Up to close, back to the clock.
    driver.swipe(Point::new(233, 420), Point::new(233, 80), ms(300));
    driver.settle();
    driver.wait(ms(1_000));
}

/// Boots with the clock running in Dublin, each report arriving at `at` ms from power-on.
fn boot(driver: &mut Driver, reports: &[(Report, u64)]) {
    driver.stage = Stage::starting(PeripheralState {
        firmware: "0.1.0",
        ..PeripheralState::default()
    });
    driver.run_clock();
    driver.hold(hand);
    driver.sensors(dublin());
    for &(report, at) in reports {
        driver.wait(ms(at).saturating_sub(driver.now()));
        driver.report(report);
    }
}

fn startup(driver: &mut Driver) {
    boot(driver, &ANSWERING);
    driver.wait(ms(5_900));
}

fn startup_unplugged(driver: &mut Driver) {
    boot(driver, &ANSWERING);
    on_battery(driver, 87, false);
    driver.wait(ms(5_900));
}

/// Every part answering, as a start-up usually goes. The radio's check starts as GNSS's ends
/// and takes about 2 ms, as on the board.
pub const ANSWERING: [(Report, u64); 8] = [
    (Report::Decided(Part::Power, Outcome::Answered), 150),
    (Report::Decided(Part::Clock, Outcome::Answered), 250),
    (Report::Decided(Part::Touch, Outcome::Answered), 500),
    (Report::Decided(Part::Motion, Outcome::Answered), 600),
    (Report::Decided(Part::Magnet, Outcome::Answered), 750),
    (Report::Decided(Part::Gnss, Outcome::Answered), 1_300),
    (Report::Started(Part::Radio), 1_300),
    (Report::Decided(Part::Radio, Outcome::Answered), 1_302),
];

/// [`ANSWERING`] with `changes` in place of the reports for their parts.
fn answering_but(changes: &[(Report, u64)]) -> Vec<(Report, u64)> {
    let part = |report: &Report| match *report {
        Report::Started(part) | Report::Decided(part, _) => part,
    };
    let decides = |report: &Report| matches!(report, Report::Decided(..));
    let mut reports: Vec<_> = ANSWERING
        .iter()
        .filter(|(report, _)| {
            !changes.iter().any(|(change, _)| {
                part(change) == part(report) && decides(change) == decides(report)
            })
        })
        .chain(changes)
        .copied()
        .collect();
    reports.sort_by_key(|&(_, at)| at);
    reports
}

fn startup_failed(driver: &mut Driver) {
    boot(
        driver,
        &answering_but(&[(Report::Decided(Part::Magnet, Outcome::NoReply), 1_000)]),
    );
    driver.wait(ms(5_200));
}

fn startup_radio_failed(driver: &mut Driver) {
    boot(
        driver,
        &answering_but(&[(Report::Decided(Part::Radio, Outcome::NoReply), 1_302)]),
    );
    driver.wait(ms(5_200));
}

/// The radio answering at the end of its 200 ms deadline.
fn startup_radio_slow(driver: &mut Driver) {
    boot(
        driver,
        &answering_but(&[(Report::Decided(Part::Radio, Outcome::Answered), 1_500)]),
    );
    driver.wait(ms(5_900));
}

/// POWER failing at its 200 ms deadline.
fn startup_power_failed(driver: &mut Driver) {
    boot(
        driver,
        &answering_but(&[(Report::Decided(Part::Power, Outcome::NoReply), 200)]),
    );
    driver.wait(ms(5_200));
}

fn startup_power_radio_failed(driver: &mut Driver) {
    boot(
        driver,
        &answering_but(&[
            (Report::Decided(Part::Power, Outcome::NoReply), 200),
            (Report::Decided(Part::Radio, Outcome::NoReply), 1_302),
        ]),
    );
    driver.wait(ms(5_200));
}

fn power_off(driver: &mut Driver) {
    start(driver, Screen::Clock);
    driver.wait(ms(1_000));
    press(driver, Button::Power, Key::Long);
    driver.wait(ms(700));
    // Drag the handle partway, think better of it and bring it back before letting go.
    let (start, partway) = (Point::new(113, 258), Point::new(290, 258));
    drag(
        driver,
        start,
        &[(partway, ms(700)), (partway, ms(300)), (start, ms(600))],
    );
    driver.wait(ms(1_000));
    driver.tap(Point::new(133, 118));
    driver.wait(ms(1_000));
    press(driver, Button::Power, Key::Short);
    driver.wait(ms(850));
    press(driver, Button::Power, Key::Long);
    driver.wait(ms(900));
    driver.swipe(Point::new(113, 258), Point::new(400, 258), ms(800));
    driver.wait(ms(1_000));
}

fn power_off_dim_cancel(driver: &mut Driver) {
    driver.stage = Stage::new(PeripheralState {
        firmware: "0.1.0",
        timeout: Timeout::Seconds15,
        ..PeripheralState::default()
    });
    start(driver, Screen::Clock);
    driver.wait(ms(15_000));
    press(driver, Button::Power, Key::Long);
    driver.wait(ms(1_200));
    driver.tap(Point::new(133, 118));
    driver.wait(ms(6_500));
}

/// Settles on `screen` with a 15 s timeout, and waits out the timeout and the dim.
fn rest(driver: &mut Driver, screen: Screen, always_on: bool) {
    driver.stage = Stage::new(PeripheralState {
        firmware: "0.1.0",
        timeout: Timeout::Seconds15,
        always_on: if always_on {
            AlwaysOn::Dim
        } else {
            AlwaysOn::Off
        },
        ..PeripheralState::default()
    });
    start(driver, screen);
    driver.wait(ms(22_000));
    driver.double_tap();
    driver.wait(ms(2_000));
}

fn rest_always_on(driver: &mut Driver) {
    rest(driver, Screen::Clock, true);
}

fn rest_off(driver: &mut Driver) {
    rest(driver, Screen::Compass, false);
}

/// The clock in Dublin, awake, in a group with Ridge, Cove and Moss, and with their
/// conversations if `conversations`.
fn in_group(driver: &mut Driver, conversations: bool) {
    driver.stage = Stage::new(PeripheralState {
        firmware: "0.1.0",
        timeout: Timeout::Never,
        ..PeripheralState::default()
    });
    driver.stage.use_messages(Box::leak(MessagesView::boxed()));
    start(driver, Screen::Clock);
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
    driver.wait(ms(6_000));
}

fn mesh<'a>(driver: &'a mut Driver<'_>) -> &'a mut Sim {
    driver.mesh.as_mut().expect("a scripted mesh")
}

fn open_drawer(driver: &mut Driver) {
    driver.swipe(Point::new(233, 430), Point::new(233, 120), ms(300));
    driver.settle();
}

fn drawer(driver: &mut Driver) {
    in_group(driver, true);
    let now = driver.now();
    mesh(driver).request_removal(2, 1, 402_000_000, now);
    driver.wait(ms(6_000));
    say("AN UPWARD DRAG OPENS EVENTS OVER THE FACE.");
    open_drawer(driver);
    say("AT REST FOR A WHOLE 10 S BREATHING CYCLE: ONLY THE HALFTONE CHANGES.");
    driver.wait(ms(10_000));
    say("THE LIST SCROLLS, AND ROWS SHRINK A LITTLE NEAR ITS EDGES.");
    driver.swipe(Point::new(233, 380), Point::new(233, 200), ms(700));
    driver.wait(ms(1_200));
    driver.swipe(Point::new(233, 200), Point::new(233, 380), ms(700));
    driver.wait(ms(1_200));
    say("A SIDEWAYS DRAG MOVES TO MESSAGES, AND BACK.");
    driver.swipe(Point::new(380, 260), Point::new(80, 260), ms(400));
    driver.wait(ms(2_500));
    driver.swipe(Point::new(80, 260), Point::new(380, 260), ms(400));
    driver.wait(ms(2_000));
    say("A PULL DOWN FROM THE TOP OF THE LIST CLOSES IT.");
    driver.swipe(Point::new(233, 160), Point::new(233, 440), ms(400));
    driver.settle();
    driver.wait(ms(1_500));
}

fn member_face(driver: &mut Driver) {
    in_group(driver, false);
    say("THE MEMBER FACE IS THE THIRD, AFTER THE COMPASS.");
    page_left(driver, ms(400));
    driver.wait(ms(1_000));
    page_left(driver, ms(400));
    driver.wait(ms(2_000));
    say("THE GRID AND THE BEARINGS TURN WITH THE TRUE HEADING.");
    driver.motion_over(ms(4_000), |t| facing(37.0 + 60.0 * swing(t)));
    driver.wait(ms(1_000));
    say("A TAP IN THE MIDDLE SELECTS THE NEXT MEMBER.");
    slow_tap(driver, 233, 233);
    driver.wait(ms(1_500));
    slow_tap(driver, 233, 233);
    driver.wait(ms(2_000));
}

fn messages(driver: &mut Driver) {
    in_group(driver, true);
    let now = driver.now();
    let own = mesh(driver)
        .view()
        .group
        .as_ref()
        .map_or(0, |group| group.own);
    say("A PRIVATE MESSAGE ARRIVES. THE TOAST NAMES ITS SENDER, NOT ITS WORDS.");
    mesh(driver).arrive(
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
    driver.wait(ms(1_500));
    say("A TAP ON THE TOAST OPENS THE CONVERSATION. A SECOND IN VIEW READS IT.");
    slow_tap(driver, 233, 360);
    driver.wait(ms(2_500));
    say("WRITE OPENS THE KEYBOARD.");
    slow_tap(driver, 233, 426);
    driver.wait(ms(800));
    for point in taps("at the bridge", Mode::Lower) {
        driver.tap(point);
        driver.wait(ms(120));
    }
    driver.wait(ms(800));
    say("REVIEW READS IT THROUGH BEFORE SEND.");
    slow_tap(driver, 333, 131);
    driver.wait(ms(1_500));
    say("SENT, IT GOES FROM QUEUED TO DELIVERED ONCE RIDGE ACKNOWLEDGES IT.");
    slow_tap(driver, 306, 391);
    driver.wait(ms(12_000));
}

fn removal(driver: &mut Driver) {
    in_group(driver, false);
    say("REMOVE ON A MEMBER OPENS A SLIDE.");
    open_settings(driver);
    page_left(driver, ms(400));
    slow_tap(driver, 233, 190);
    slow_tap(driver, 159, 353);
    slow_tap(driver, 233, 380);
    driver.wait(ms(800));
    slow_tap(driver, 233, 353);
    driver.wait(ms(1_200));
    say("A SLIDE SHORT OF THE END EASES BACK AND ASKS NOTHING.");
    driver.swipe(Point::new(110, 372), Point::new(230, 372), ms(500));
    driver.wait(ms(800));
    say("TO THE END, IT STARTS THE GROUP CHANGE AND COUNTS DOWN TO THE SWITCH.");
    driver.swipe(Point::new(110, 372), Point::new(350, 372), ms(700));
    driver.wait(ms(5_000));
    say("ANOTHER MEMBER'S REQUEST, ON ANOTHER DEVICE: ITS TOAST OPENS IT.");
    in_group(driver, false);
    let now = driver.now();
    mesh(driver).request_removal(2, 1, 402_000_000, now);
    driver.wait(ms(1_000));
    slow_tap(driver, 233, 360);
    driver.wait(ms(2_500));
    say("DECLINE ASKS FOR A SLIDE OF ITS OWN.");
    slow_tap(driver, 306, 391);
    driver.wait(ms(2_000));
    driver.swipe(Point::new(110, 372), Point::new(350, 372), ms(700));
    driver.wait(ms(3_000));
}

/// The point `metres` from `from` at `bearing` degrees true, on a sphere.
fn at(from: (i32, i32), bearing: f64, metres: f64) -> (i32, i32) {
    let (phi1, lambda1) = (
        (f64::from(from.0) * 1e-7).to_radians(),
        (f64::from(from.1) * 1e-7).to_radians(),
    );
    let (d, theta) = (metres / 6_371_008.8, bearing.to_radians());
    let phi2 = (phi1.sin() * d.cos() + phi1.cos() * d.sin() * theta.cos()).asin();
    let lambda2 =
        lambda1 + (theta.sin() * d.sin() * phi1.cos()).atan2(d.cos() - phi1.sin() * phi2.sin());
    (
        (phi2.to_degrees() * 1e7).round() as i32,
        (lambda2.to_degrees() * 1e7).round() as i32,
    )
}

/// The 2026-10-05 hand-off's crowded group: 31 members placed round this device, in four runs
/// of bearings and one apart, their positions seconds to minutes old.
fn crowded_group(now: Micros) -> GroupView {
    const RUNS: [(f64, f64, usize, u64, u64); 4] = [
        (35.0, 61.0, 8, 11, 1_080),
        (118.0, 154.0, 8, 23, 420),
        (218.0, 246.0, 8, 120, 1_080),
        (305.0, 335.0, 6, 11, 180),
    ];
    let names = [
        "Kestrel", "Cove", "Moss", "Lough", "Basecamp", "Ridge", "Harbour", "Fell",
    ];
    let ago = |seconds: u64| now as i64 - (seconds * 1_000_000) as i64;
    let member = |id: usize, bearing: f64, age: u64| MemberView {
        name: Name::new(names[id % names.len()].as_bytes()).expect("a fixture name"),
        mac: [0x48, 0xa1, 0xb2, 0xc3, 0x8c, id as u8],
        device: [0x9c, 0x2a, 0x7f, 0x10, 0, 0, 0, id as u8],
        joined: Some(ago(86_400)),
        heard: Some(ago(7 + id as u64)),
        position: Position::At(ago(age)),
        coordinates: Some(at(
            (533_498_000, -62_603_000),
            bearing,
            120.0 + 40.0 * id as f64,
        )),
    };
    let mut members = [None; IDS as usize];
    members[0] = Some(MemberView {
        coordinates: None,
        position: Position::Never,
        heard: None,
        ..member(0, 0.0, 0)
    });
    let mut id = 1;
    for (first, last, count, youngest, oldest) in RUNS {
        for i in 0..count {
            let share = i as f64 / (count - 1) as f64;
            let age = youngest + ((oldest - youngest) as f64 * share) as u64;
            members[id] = Some(member(id, (first + (last - first) * share) % 360.0, age));
            id += 1;
        }
    }
    members[id] = Some(member(id, 190.0, 300));
    GroupView { own: 0, members }
}

fn crowded_sectors(driver: &mut Driver) {
    driver.stage = Stage::new(PeripheralState {
        firmware: "0.1.0",
        timeout: Timeout::Never,
        ..PeripheralState::default()
    });
    start(driver, Screen::Members);
    let now = driver.now();
    driver.mesh = Some(Sim::new(Some(crowded_group(now))));
    driver.wait(ms(1_000));
    say("31 POSITIONS. A RUN OF MEMBERS TOO CLOSE TO LABEL IS A SECTOR: ITS SPAN, COUNT AND AGES.");
    driver.wait(ms(4_000));
    say("THE FACE TURNS WITH THE HEADING. ONLY THE MEMBERS' BEARINGS APART DECIDE THE SECTORS.");
    driver.motion_over(ms(8_000), |t| facing(37.0 + 330.0 * ease(t)));
    driver.wait(ms(1_000));
    say("A TAP IN THE MIDDLE SELECTS THE NEXT MEMBER, AT ITS OWN BEARING. ITS SECTOR TURNS LIME.");
    for _ in 0..10 {
        slow_tap(driver, 233, 233);
        driver.wait(ms(1_600));
    }
    say("SELECTED, A MEMBER STAYS AT ITS BEARING AS THE FACE TURNS.");
    driver.motion_over(ms(6_000), |t| facing(7.0 - 90.0 * swing(t)));
    driver.wait(ms(2_000));
}

fn long_message(driver: &mut Driver) {
    in_group(driver, false);
    let now = driver.now();
    let own = mesh(driver)
        .view()
        .group
        .as_ref()
        .map_or(0, |group| group.own);
    say("A MESSAGE ARRIVES TOO TALL FOR ITS CONVERSATION TO SHOW WHOLE.");
    mesh(driver).arrive(
        Arrival {
            from: 1,
            to: Some(own),
            text: "WWWWWWWWWWWW MMMMMMMMMMMM WWWWWWWWWWWW MMMMMMMMMMMM WWWWWWWWWWWW MMMMMMMMMMMM \
                   WWWWWWWWWWWW MMMMMMMMMMMM WWWWWWWWWWWW MMMMMMMMMMMM WWWWWWWWWWWW MMMMMMMMMMMM",
            ago: 0,
            carriage: Carriage::Received,
            unread: true,
        },
        now,
    );
    driver.wait(ms(1_500));
    say("ITS CONVERSATION OPENS AT THE TOP. THE LINES IN VIEW COUNT; THOSE BELOW HAVE NOT SHOWN.");
    slow_tap(driver, 233, 360);
    driver.wait(ms(3_000));
    say("A QUICK LOOK AT THE END IS NOT READING IT: IT STAYS UNREAD.");
    driver.swipe(Point::new(233, 380), Point::new(233, 140), ms(200));
    driver.wait(ms(400));
    driver.swipe(Point::new(233, 140), Point::new(233, 380), ms(200));
    driver.wait(ms(2_500));
    say("READ DOWN TO THE END, EVERY LINE HAS SHOWN FOR A SECOND: NOW IT IS READ.");
    driver.swipe(Point::new(233, 380), Point::new(233, 140), ms(1_500));
    driver.wait(ms(3_000));
}
