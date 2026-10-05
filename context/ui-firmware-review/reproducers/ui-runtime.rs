use std::{cell::RefCell, rc::Rc};

use embedded_graphics::prelude::Point;
use octowhere_ui::ui::{
    group::sim::{self, Sim},
    rest::{Rest, Timeout},
    screens::{PeripheralState, Screen},
    script::Driver,
    stage::{Input, Key, Stage},
};

fn start_with(group: Option<u8>, timeout: Timeout) -> Driver<'static> {
    let mut driver = Driver::on(Screen::Clock);
    driver.stage = Stage::new(PeripheralState {
        timeout,
        ..PeripheralState::default()
    });
    driver.stage.show(Screen::Clock);
    driver.wait(600_000);
    let now = driver.now();
    driver.mesh = Some(Sim::new(group.map(|count| sim::group(count, now))));
    driver.wait(100_000);
    driver
}

#[test]
fn a_short_press_on_a_dimmed_screen() {
    for darkening in [false, true] {
        let mut driver = start_with(None, Timeout::Seconds15);
        let mut guard = 0;
        loop {
            let rest = driver.stage.rest();
            let reached = if darkening {
                matches!(rest, Rest::Darkening { .. })
            } else {
                matches!(rest, Rest::Dimmed { .. })
            };
            if reached {
                break;
            }
            driver.step(Input::default());
            guard += 1;
            assert!(guard < 3_000);
        }
        if !darkening {
            driver.wait(200_000);
        }
        let before = (driver.stage.rest(), driver.stage.shown_level());
        let update = driver.key(Key::Short);
        let after_key = (driver.stage.rest(), driver.stage.shown_level(), update.brightness);
        let mut levels = Vec::new();
        for _ in 0..90 {
            let update = driver.step(Input::default());
            if let Some(level) = update.brightness {
                levels.push(level);
            }
        }
        println!(
            "darkening {darkening}: before {before:?}; after the key {after_key:?}; 1.5 s on {:?} level {}; levels {levels:?}",
            driver.stage.rest(),
            driver.stage.shown_level()
        );
    }
}

#[test]
fn the_refresh_screen_before_the_mesh_takes_it_up() {
    let mut driver = start_with(Some(8), Timeout::Never);
    driver.swipe(Point::new(233, 80), Point::new(233, 420), 250_000);
    driver.settle();
    driver.wait(600_000);
    driver.swipe(Point::new(380, 250), Point::new(80, 250), 300_000);
    driver.settle();
    driver.wait(400_000);
    driver.tap(Point::new(233, 190));
    driver.wait(300_000);
    driver.tap(Point::new(159, 353));
    driver.wait(300_000);
    driver.tap(Point::new(233, 178));
    driver.wait(300_000);
    let seen: Rc<RefCell<Vec<Vec<String>>>> = Rc::default();
    let record = Rc::clone(&seen);
    driver.observe(move |stage, _| {
        record
            .borrow_mut()
            .push(stage.group_text().map(String::from).collect());
    });
    driver.tap(Point::new(233, 353));
    driver.wait(100_000);
    for (step, lines) in seen.borrow().iter().enumerate() {
        if lines.iter().any(|line| line == "LISTENING") {
            let left: Vec<_> = lines.iter().filter(|line| line.contains(':')).collect();
            println!("step {step}: {left:?}");
        }
    }
}

fn resting_driver() -> Driver<'static> {
    use octowhere_ui::ui::rest::AlwaysOn;
    let mut driver = Driver::on(Screen::Clock);
    driver.stage = Stage::new(PeripheralState {
        timeout: Timeout::Seconds15,
        always_on: AlwaysOn::Dim,
        ..PeripheralState::default()
    });
    driver.stage.show(Screen::Clock);
    driver.wait(600_000);
    driver
}

fn render(stage: &Stage) -> Box<octowhere_ui::chrome::FB> {
    let mut fb = octowhere_ui::chrome::FB::boxed();
    stage.draw(&mut *fb);
    fb
}

fn differing(a: &octowhere_ui::chrome::FB, b: &octowhere_ui::chrome::FB) -> usize {
    (0..466 * 466)
        .map(|index| Point::new(index % 466, index / 466))
        .filter(|&point| a.pixel(point) != b.pixel(point))
        .count()
}

#[test]
fn the_drawer_left_open_through_the_timeout() {
    let mut with = resting_driver();
    with.swipe(Point::new(233, 430), Point::new(233, 120), 250_000);
    with.settle();
    with.wait(300_000);
    assert!(with.stage.drawer().is_some());
    let mut guard = 0;
    let mut last_dimmed = None;
    while with.stage.rest() != Rest::AlwaysOn {
        last_dimmed = Some(render(&with.stage));
        with.step(Input::default());
        guard += 1;
        assert!(guard < 3_000);
    }
    let resting_with = render(&with.stage);
    let mut without = resting_driver();
    while without.stage.rest() != Rest::AlwaysOn {
        without.step(Input::default());
    }
    let resting_without = render(&without.stage);
    println!(
        "drawer still held: {}; drawer text lines: {}; pixels differing from the last dimmed frame: {}; from the always-on face drawn without the drawer: {}",
        with.stage.drawer().is_some(),
        with.stage.drawer_text().count(),
        differing(&resting_with, last_dimmed.as_ref().unwrap()),
        differing(&resting_with, &resting_without),
    );
}

#[test]
fn list_size() {
    println!(
        "List {} bytes, Item {} bytes on this host",
        core::mem::size_of::<octowhere_ui::ui::group::layout::List>(),
        core::mem::size_of::<octowhere_ui::ui::group::layout::Item>()
    );
}

#[test]
fn a_refresh_numbered_again_from_one() {
    use octowhere_ui::ui::group::view::{RefreshPhase, RefreshView};
    let mut driver = start_with(None, Timeout::Never);
    let set = |driver: &mut Driver, refresh: Option<RefreshView>| {
        driver.stage.update_mesh(|mesh| mesh.refresh = refresh);
        driver.wait(50_000);
    };
    let view = |session, phase| RefreshView { session, phase, heard: 0b10, learned: 0 };
    let until = (driver.now() + 135_000_000) as i64;
    set(&mut driver, Some(view(1, RefreshPhase::Listening { until })));
    println!("first refresh: ongoing events {}", driver.stage.events().ongoing());
    driver.wait(135_000_000);
    let at = driver.now() as i64;
    set(&mut driver, Some(view(1, RefreshPhase::Ended { at })));
    let id = driver.stage.toast().expect("its end is told");
    driver.wait(6_000_000);
    // DISMISS, as its detail offers once it has ended.
    let mut events = driver.stage.events().clone();
    println!("dismissed: {:?}", events.dismiss(id));
    // The stage's own Events cannot be reached mutably from here; go through the drawer instead.
    driver.swipe(Point::new(233, 430), Point::new(233, 120), 250_000);
    driver.settle();
    driver.wait(300_000);
    driver.tap(Point::new(233, 170));
    driver.wait(300_000);
    driver.tap(Point::new(306, 391));
    driver.wait(300_000);
    driver.tap(Point::new(233, 26));
    driver.wait(1_000_000);
    println!("after DISMISS: {} events", driver.stage.events().len());
    // The device leaves its group and joins another: the node forgets its refresh, and numbers
    // the next from 1 again.
    set(&mut driver, None);
    let until = (driver.now() + 135_000_000) as i64;
    set(&mut driver, Some(view(1, RefreshPhase::Listening { until })));
    println!("second refresh: ongoing events {}, events {}", driver.stage.events().ongoing(), driver.stage.events().len());
    driver.wait(135_000_000);
    let at = driver.now() as i64;
    set(&mut driver, Some(view(1, RefreshPhase::Ended { at })));
    println!("second refresh ended: toast {:?}, events {}", driver.stage.toast(), driver.stage.events().len());
}
