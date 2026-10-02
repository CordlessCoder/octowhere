//! The group screens driven through the stage with a simulated mesh: what they ask of the mesh,
//! when, and that redrawing only their damage leaves what a full redraw would.

use std::{cell::RefCell, rc::Rc};

use embedded_graphics::prelude::Point;
use octowhere_ui::{
    chrome::{Clip, Dirty, FB},
    ui::{
        group::{
            sim::{self, PeerUser, Sim},
            view::{Done, End, MeshView, Phase, Reason, Role},
        },
        rest::Rest,
        screens::{PeripheralState, Screen},
        script::Driver,
        stage::{Key, Stage},
    },
};

fn start(group: Option<u8>) -> Driver<'static> {
    let mut driver = Driver::on(Screen::Clock);
    driver.stage = Stage::new(PeripheralState::default());
    driver.stage.show(Screen::Clock);
    driver.wait(600_000);
    let now = driver.now();
    driver.mesh = Some(Sim::new(group.map(|count| sim::group(count, now))));
    driver.wait(100_000);
    driver
}

fn tap(driver: &mut Driver, x: i32, y: i32) {
    driver.tap(Point::new(x, y));
    driver.wait(300_000);
}

/// The group screen, opened from the panel's second page.
fn hub(group: Option<u8>) -> Driver<'static> {
    let mut driver = start(group);
    driver.swipe(Point::new(233, 80), Point::new(233, 420), 250_000);
    driver.settle();
    driver.wait(600_000);
    driver.swipe(Point::new(380, 250), Point::new(80, 250), 300_000);
    driver.settle();
    driver.wait(400_000);
    tap(&mut driver, 233, 190);
    driver
}

fn view<'a>(driver: &'a Driver) -> &'a MeshView {
    driver.mesh.as_ref().expect("a simulated mesh").view()
}

fn phase(driver: &Driver) -> Option<Phase> {
    view(driver).pairing.as_ref().map(|pairing| pairing.phase)
}

fn until(driver: &mut Driver, test: impl Fn(&Driver) -> bool) {
    for _ in 0..3_000 {
        if test(driver) {
            return;
        }
        driver.wait(16_667);
    }
    panic!("never got there; the mesh is at {:?}", phase(driver));
}

fn accept(driver: &mut Driver) {
    driver.swipe(Point::new(149, 353), Point::new(330, 353), 300_000);
    driver.wait(50_000);
}

/// Adding to an eight-member group, as far as the code.
fn adding_at_the_code() -> Driver<'static> {
    let mut driver = hub(Some(8));
    tap(&mut driver, 307, 353);
    tap(&mut driver, 156, 353);
    tap(&mut driver, 233, 353);
    until(&mut driver, |driver| phase(driver) == Some(Phase::Found));
    tap(&mut driver, 233, 247);
    until(&mut driver, |driver| {
        matches!(phase(driver), Some(Phase::Compare { .. }))
    });
    driver
}

#[test]
fn nothing_asks_the_mesh_to_pair_before_start() {
    let mut driver = hub(None);
    tap(&mut driver, 156, 353);
    driver.wait(5_000_000);
    assert_eq!(view(&driver).sessions, 0);
    tap(&mut driver, 233, 353);
    assert_eq!(view(&driver).sessions, 1);
    assert_eq!(
        view(&driver).pairing.as_ref().map(|pairing| pairing.role),
        Some(Role::Add)
    );
}

#[test]
fn adding_runs_through_the_code_to_a_stored_member() {
    let mut driver = adding_at_the_code();
    // A tap or a short drag confirms nothing.
    tap(&mut driver, 149, 353);
    driver.swipe(Point::new(149, 353), Point::new(260, 353), 200_000);
    driver.wait(300_000);
    assert!(matches!(phase(&driver), Some(Phase::Compare { .. })));
    accept(&mut driver);
    assert!(matches!(phase(&driver), Some(Phase::Waiting { .. })));
    until(&mut driver, |driver| {
        matches!(phase(driver), Some(Phase::Done(Done::Added { .. })))
    });
    tap(&mut driver, 233, 353);
    assert_eq!(
        view(&driver).group.as_ref().map(|group| group.count()),
        Some(9)
    );
    // VIEW GROUP shows the group, its count now nine.
    assert!(driver.stage.page().is_some());
}

#[test]
fn a_vertical_drag_on_the_handle_confirms_nothing() {
    let mut driver = adding_at_the_code();
    driver.swipe(Point::new(149, 353), Point::new(170, 200), 200_000);
    driver.wait(300_000);
    assert!(matches!(phase(&driver), Some(Phase::Compare { .. })));
}

#[test]
fn stop_reports_a_mismatch_or_declines() {
    for (x, end) in [(156, End::Mismatch), (310, End::Declined)] {
        let mut driver = adding_at_the_code();
        tap(&mut driver, 132, 115);
        // BACK from the sheet returns to the code with nothing sent.
        tap(&mut driver, 132, 115);
        assert!(matches!(phase(&driver), Some(Phase::Compare { .. })));
        tap(&mut driver, 132, 115);
        tap(&mut driver, x, 353);
        assert_eq!(phase(&driver), Some(Phase::Ended(end)));
    }
}

#[test]
fn the_other_users_rejection_ends_it_here() {
    let mut driver = hub(Some(8));
    driver.mesh.as_mut().unwrap().peer = PeerUser::ReportsMismatch;
    tap(&mut driver, 307, 353);
    tap(&mut driver, 156, 353);
    tap(&mut driver, 233, 353);
    until(&mut driver, |driver| phase(driver) == Some(Phase::Found));
    tap(&mut driver, 233, 247);
    until(&mut driver, |driver| {
        phase(driver).is_some_and(Phase::is_final)
    });
    assert_eq!(
        phase(&driver),
        Some(Phase::Ended(End::Peer(Reason::Mismatch)))
    );
}

#[test]
fn a_cover_cancels_a_pairing_and_returns_to_the_clock() {
    let mut driver = adding_at_the_code();
    driver.cover();
    driver.settle();
    assert_eq!(phase(&driver), Some(Phase::Ended(End::Cancelled)));
    assert!(driver.stage.page().is_none());
    assert_eq!(driver.stage.screen(), Screen::Clock);
}

#[test]
fn the_power_key_cancels_a_pairing_before_the_screen_rests() {
    for key in [Key::Short, Key::Long] {
        let mut driver = adding_at_the_code();
        driver.key(key);
        driver.wait(100_000);
        assert_eq!(
            phase(&driver),
            Some(Phase::Ended(End::Cancelled)),
            "{key:?}"
        );
    }
}

#[test]
fn a_pairing_holds_the_screen_awake_and_the_group_screen_does_not() {
    let mut idle = hub(Some(8));
    idle.wait(70_000_000);
    assert_ne!(idle.stage.rest(), Rest::Awake);

    let mut pairing = hub(Some(8));
    tap(&mut pairing, 307, 353);
    tap(&mut pairing, 156, 353);
    tap(&mut pairing, 233, 353);
    pairing.wait(70_000_000);
    assert_eq!(phase(&pairing), Some(Phase::Found));
    assert_eq!(pairing.stage.rest(), Rest::Awake);
}

#[test]
fn a_full_group_blocks_adding_before_discovery() {
    let mut driver = hub(Some(32));
    tap(&mut driver, 307, 353);
    tap(&mut driver, 156, 353);
    driver.wait(1_000_000);
    assert_eq!(view(&driver).sessions, 0);
}

#[test]
fn joining_from_a_group_leaves_it_first() {
    let mut driver = hub(Some(8));
    tap(&mut driver, 307, 353);
    tap(&mut driver, 310, 353);
    tap(&mut driver, 233, 353);
    // BACK before the drag erases nothing.
    tap(&mut driver, 132, 115);
    assert!(view(&driver).group.is_some());
    tap(&mut driver, 233, 353);
    accept(&mut driver);
    driver.wait(300_000);
    assert!(view(&driver).group.is_none());
    assert_eq!(
        view(&driver).pairing.as_ref().map(|pairing| pairing.role),
        Some(Role::Join)
    );
}

#[test]
fn a_failed_leave_keeps_the_group() {
    let mut driver = hub(Some(8));
    driver.mesh.as_mut().unwrap().store_fails = true;
    tap(&mut driver, 233, 247);
    tap(&mut driver, 310, 353);
    tap(&mut driver, 233, 353);
    accept(&mut driver);
    driver.wait(300_000);
    assert!(view(&driver).group.is_some());
    assert_eq!(view(&driver).sessions, 0);
}

/// The name keyboard opened from the panel's NAME, with the saved name deleted.
fn naming() -> Driver<'static> {
    let mut driver = start(None);
    driver.swipe(Point::new(233, 80), Point::new(233, 420), 250_000);
    driver.settle();
    driver.wait(600_000);
    driver.swipe(Point::new(380, 250), Point::new(80, 250), 300_000);
    driver.settle();
    driver.wait(400_000);
    tap(&mut driver, 233, 260);
    for _ in 0..7 {
        tap(&mut driver, 389, 296);
    }
    // "Ab"
    tap(&mut driver, 77, 241);
    tap(&mut driver, 57, 296);
    tap(&mut driver, 272, 296);
    driver
}

#[test]
fn a_saved_name_is_stored_with_its_case_and_returns_to_the_panel() {
    let mut driver = naming();
    tap(&mut driver, 333, 131);
    assert_eq!(view(&driver).name.as_str(), "Ab");
    assert!(driver.stage.page().is_none());
    assert_eq!(driver.stage.peripherals().name.as_str(), "Ab");
}

#[test]
fn a_name_that_fails_to_store_keeps_the_old_one_and_the_draft() {
    let mut driver = naming();
    driver.mesh.as_mut().unwrap().store_fails = true;
    tap(&mut driver, 333, 131);
    assert_eq!(
        view(&driver).name,
        sim::group(1, 0).members[0].unwrap().name
    );
    // RETURN TO EDIT, then SAVE again once the store works.
    tap(&mut driver, 233, 353);
    driver.mesh.as_mut().unwrap().store_fails = false;
    tap(&mut driver, 333, 131);
    assert_eq!(view(&driver).name.as_str(), "Ab");
}

#[test]
fn cancel_keeps_the_saved_name() {
    let mut driver = naming();
    tap(&mut driver, 133, 131);
    assert_eq!(view(&driver).answered, 0);
    assert!(driver.stage.page().is_none());
}

/// Two framebuffers drawn in turn, each repainting its own and the last step's damage, as the
/// frame loop does; counts the steps whose drawing differs from a full redraw.
struct Buffers {
    fbs: [Box<FB>; 2],
    next: usize,
    previous: Dirty,
    drawn: [bool; 2],
    wrong: Vec<(usize, usize)>,
    steps: usize,
}

impl Buffers {
    fn draw(&mut self, stage: &Stage) {
        let changed = stage.changed();
        let mut repaint = self.previous.clone();
        repaint.extend(changed);
        self.previous = changed.clone();
        let index = self.next;
        self.next ^= 1;
        let fb = &mut *self.fbs[index];
        if repaint.is_full() || !self.drawn[index] {
            self.drawn[index] = true;
            stage.draw(fb);
        } else if !repaint.is_empty() {
            stage.draw(&mut Clip::new(fb, &repaint));
        }
        let mut whole = FB::boxed();
        stage.draw(&mut *whole);
        let differing = (0..466 * 466)
            .map(|index| Point::new(index % 466, index / 466))
            .filter(|&point| {
                let (x, y) = (point.x as f32 + 0.5 - 233.0, point.y as f32 + 0.5 - 233.0);
                x * x + y * y <= 236.0 * 236.0
            })
            .filter(|&point| fb.pixel(point) != whole.pixel(point))
            .count();
        if differing > 0 {
            self.wrong.push((self.steps, differing));
        }
        self.steps += 1;
    }
}

#[test]
fn group_screens_redraw_only_what_changed() {
    let mut driver = hub(Some(8));
    let buffers = Rc::new(RefCell::new(Buffers {
        fbs: [FB::boxed(), FB::boxed()],
        next: 0,
        previous: Dirty::new_full(),
        drawn: [false; 2],
        wrong: Vec::new(),
        steps: 0,
    }));
    let watching = Rc::clone(&buffers);
    driver.observe(move |stage, _| watching.borrow_mut().draw(stage));
    // The member list, scrolled and settled, and a detail.
    tap(&mut driver, 159, 353);
    driver.swipe(Point::new(233, 380), Point::new(233, 220), 300_000);
    driver.wait(400_000);
    tap(&mut driver, 233, 353);
    tap(&mut driver, 132, 115);
    tap(&mut driver, 132, 115);
    // A pairing through its countdown, the code and its drag, to the end.
    tap(&mut driver, 307, 353);
    tap(&mut driver, 156, 353);
    tap(&mut driver, 233, 353);
    driver.wait(3_000_000);
    tap(&mut driver, 233, 247);
    driver.wait(1_000_000);
    driver.swipe(Point::new(149, 353), Point::new(220, 353), 200_000);
    driver.wait(300_000);
    accept(&mut driver);
    driver.wait(5_000_000);
    let buffers = buffers.borrow();
    assert!(buffers.steps > 500);
    assert!(
        buffers.wrong.is_empty(),
        "steps and pixels wrong: {:?}",
        buffers.wrong
    );
}
