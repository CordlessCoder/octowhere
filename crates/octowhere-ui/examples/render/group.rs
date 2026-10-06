//! The group screens in each of their states, reached from the settings panel's second page
//! with a simulated mesh. Most names match the hand-off's renders, to compare against them.

use embedded_graphics::prelude::Point;
use octowhere_ui::ui::{
    group::{
        sim::{self, Sim},
        view::{Done, End, MeshView, Phase, Reason},
    },
    rest::Timeout,
    screens::{PeripheralState, Screen},
    script::Driver,
    stage::Stage,
};

fn start(group: Option<u8>) -> Driver<'static> {
    start_with(group, Timeout::default())
}

fn start_with(group: Option<u8>, timeout: Timeout) -> Driver<'static> {
    let mut driver = Driver::on(Screen::Clock);
    driver.stage = Stage::new(PeripheralState {
        firmware: "0.1.0",
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

fn tap(driver: &mut Driver, x: i32, y: i32) {
    driver.tap(Point::new(x, y));
    driver.wait(300_000);
}

/// The panel open on its second page.
fn panel(group: Option<u8>) -> Driver<'static> {
    let mut driver = start(group);
    driver.swipe(Point::new(233, 80), Point::new(233, 420), 250_000);
    driver.settle();
    driver.wait(600_000);
    driver.swipe(Point::new(380, 250), Point::new(80, 250), 300_000);
    driver.settle();
    driver.wait(400_000);
    driver
}

fn hub(group: Option<u8>) -> Driver<'static> {
    let mut driver = panel(group);
    tap(&mut driver, 233, 190);
    driver
}

/// The group screen on a screen that never times out, for states minutes away.
fn awake_hub(group: Option<u8>) -> Driver<'static> {
    let mut driver = start_with(group, Timeout::Never);
    driver.swipe(Point::new(233, 80), Point::new(233, 420), 250_000);
    driver.settle();
    driver.wait(600_000);
    driver.swipe(Point::new(380, 250), Point::new(80, 250), 300_000);
    driver.settle();
    driver.wait(400_000);
    tap(&mut driver, 233, 190);
    driver
}

const SECOND: u64 = 1_000_000;

/// A founding whose last acknowledgement never came, at its CHECK MEMBER, with the joining
/// device heard `heard_after` into the wait.
fn founding(heard_after: Option<u64>, store_fails: bool) -> Driver<'static> {
    let mut driver = awake_hub(None);
    mesh(&mut driver).final_reply_lost = true;
    mesh(&mut driver).joiner_heard_after = heard_after.map(|seconds| seconds * SECOND);
    tap(&mut driver, 156, 353);
    tap(&mut driver, 233, 353);
    until(&mut driver, |view| phase(view) == Some(Phase::Found));
    tap(&mut driver, 233, 247);
    until(&mut driver, |view| {
        matches!(phase(view), Some(Phase::Compare { .. }))
    });
    accept(&mut driver);
    until(&mut driver, |view| {
        phase(view) == Some(Phase::Ended(End::Unconfirmed))
    });
    mesh(&mut driver).store_fails = store_fails;
    driver
}

fn mesh<'a>(driver: &'a mut Driver<'_>) -> &'a mut Sim {
    driver.mesh.as_mut().expect("a simulated mesh")
}

/// Steps until the mesh's view passes `test`.
fn until(driver: &mut Driver, test: impl Fn(&MeshView) -> bool) {
    for _ in 0..2_000 {
        if test(mesh(driver).view()) {
            driver.wait(50_000);
            return;
        }
        driver.wait(16_667);
    }
    panic!("the simulated mesh never got there");
}

fn phase(view: &MeshView) -> Option<Phase> {
    view.pairing.as_ref().map(|pairing| pairing.phase)
}

/// Puts the pairing in `phase`, as the mesh would report it.
fn force(driver: &mut Driver, phase: Phase) {
    mesh(driver).hold();
    if let Some(pairing) = mesh(driver).view_mut().pairing.as_mut() {
        pairing.phase = phase;
        pairing.deadline = None;
    }
    driver.wait(100_000);
}

/// A pairing in `role`, started from the group screen, before the other device has answered.
fn pairing(add: bool, group: Option<u8>) -> Driver<'static> {
    let mut driver = hub(group);
    match (add, group.is_some()) {
        (true, true) => {
            tap(&mut driver, 307, 353);
            tap(&mut driver, 156, 353);
        }
        (true, false) => tap(&mut driver, 156, 353),
        (false, _) => tap(&mut driver, 310, 353),
    }
    tap(&mut driver, 233, 353);
    driver
}

/// A pairing at its code, with nothing confirmed.
fn code(add: bool) -> Driver<'static> {
    let mut driver = pairing(add, if add { Some(8) } else { None });
    if add {
        until(&mut driver, |view| phase(view) == Some(Phase::Found));
        tap(&mut driver, 233, 247);
    }
    until(&mut driver, |view| {
        matches!(phase(view), Some(Phase::Compare { .. }))
    });
    driver
}

fn accept(driver: &mut Driver) {
    driver.swipe(Point::new(149, 353), Point::new(330, 353), 300_000);
    driver.wait(50_000);
}

/// The key labelled `c` on the upper-case letters' rows.
fn letter(c: char) -> Point {
    for (keys, left, y) in [
        ("QWERTYUIOP", 38, 186),
        ("ASDFGHJKL", 58, 241),
        ("ZXCVBNM", 77, 296),
    ] {
        if let Some(i) = keys.find(c.to_ascii_uppercase()) {
            return Point::new(left + 39 * i as i32 + 19, y);
        }
    }
    panic!("no key for {c}");
}

/// The key in column `column` of row `row`, on a view whose rows start at the left edge.
fn key(column: i32, row: i32) -> Point {
    Point::new(38 + 39 * column + 19, 186 + 55 * row)
}

const CASE: Point = Point::new(57, 296);
const NUMBERS: Point = Point::new(120, 353);
const SYMBOLS: Point = Point::new(344, 353);

pub fn frames() -> Vec<(String, Stage)> {
    let mut frames: Vec<(String, Stage)> = Vec::new();
    let mut push = |name: &str, driver: Driver| frames.push((name.into(), driver.stage));

    push("panel-page2", panel(Some(8)));
    push("group-none", hub(None));
    push("group-home", hub(Some(8)));
    let mut options = hub(Some(8));
    tap(&mut options, 307, 353);
    push("group-pair-options", options);

    let members = || {
        let mut driver = hub(Some(8));
        tap(&mut driver, 159, 353);
        driver
    };
    push("group-members", members());
    let mut scrolled = members();
    scrolled.swipe(Point::new(233, 400), Point::new(233, 82), 400_000);
    scrolled.wait(400_000);
    push("group-never-heard", scrolled);
    let mut large = hub(Some(32));
    tap(&mut large, 159, 353);
    for _ in 0..12 {
        large.swipe(Point::new(233, 400), Point::new(233, 82), 400_000);
        large.wait(400_000);
    }
    push("group-members-large", large);
    let mut no_radio = hub(Some(8));
    mesh(&mut no_radio).view_mut().radio = false;
    no_radio.wait(50_000);
    tap(&mut no_radio, 159, 353);
    push("group-members-no-radio", no_radio);
    let mut detail = members();
    tap(&mut detail, 233, 353);
    push("member-detail", detail);
    let mut long = members();
    long.swipe(Point::new(233, 380), Point::new(233, 274), 300_000);
    long.wait(400_000);
    tap(&mut long, 233, 353);
    push("member-detail-long", long);
    let mut unavailable = members();
    tap(&mut unavailable, 233, 353);
    tap(&mut unavailable, 233, 353);
    push("remove-unavailable", unavailable);

    let own = || {
        let mut driver = hub(Some(8));
        tap(&mut driver, 233, 247);
        driver
    };
    push("member-own", own());
    let leave = |fails: bool| {
        let mut driver = own();
        mesh(&mut driver).store_fails = fails;
        tap(&mut driver, 310, 353);
        driver
    };
    push("leave-entry", leave(false));
    let mut drag = leave(false);
    tap(&mut drag, 233, 353);
    push("leave-drag", drag);
    for (name, fails) in [("leave-done", false), ("leave-save-failed", true)] {
        let mut driver = leave(fails);
        tap(&mut driver, 233, 353);
        accept(&mut driver);
        driver.wait(300_000);
        push(name, driver);
    }

    let keyboard = || {
        let mut driver = panel(Some(8));
        tap(&mut driver, 233, 260);
        driver
    };
    push("keyboard-cancelled", keyboard());
    let ridge = || {
        let mut driver = keyboard();
        for _ in 0..7 {
            tap(&mut driver, 389, 296);
        }
        tap(&mut driver, letter('r').x, letter('r').y);
        tap(&mut driver, 57, 296);
        for c in ['i', 'd', 'g', 'e'] {
            tap(&mut driver, letter(c).x, letter(c).y);
        }
        driver
    };
    let mut empty = keyboard();
    for _ in 0..7 {
        tap(&mut empty, 389, 296);
    }
    tap(&mut empty, 57, 296);
    tap(&mut empty, 57, 296);
    push("keyboard-empty", empty);
    push("keyboard-lowercase", ridge());
    let mut upper = ridge();
    tap(&mut upper, 57, 296);
    push("keyboard-uppercase", upper);
    let mut numbers = ridge();
    tap(&mut numbers, 120, 353);
    push("keyboard-numbers", numbers);
    let mut symbols = ridge();
    tap(&mut symbols, 344, 353);
    push("keyboard-symbols", symbols);
    let mut pressed = ridge();
    pressed.touch(Some(letter('e')));
    pressed.wait(50_000);
    push("keyboard-pressed", pressed);
    // To sixteen characters: Ridge_Walker07!?
    let mut limit = ridge();
    let mut sequence = vec![SYMBOLS, key(6, 0), NUMBERS, letter('w'), CASE];
    sequence.extend("alker".chars().map(letter));
    sequence.extend([NUMBERS, key(0, 0), key(7, 0), key(0, 1), SYMBOLS, key(0, 0)]);
    for point in sequence {
        tap(&mut limit, point.x, point.y);
    }
    push("keyboard-limit", limit);
    let mut failed = ridge();
    mesh(&mut failed).store_fails = true;
    tap(&mut failed, 333, 131);
    push("name-save-failed", failed);

    push("add-create-entry", {
        let mut driver = hub(None);
        tap(&mut driver, 156, 353);
        driver
    });
    push("add-entry", {
        let mut driver = hub(Some(8));
        tap(&mut driver, 307, 353);
        tap(&mut driver, 156, 353);
        driver
    });
    push("join-entry", {
        let mut driver = hub(None);
        tap(&mut driver, 310, 353);
        driver
    });
    push("add-search", pairing(true, Some(8)));
    push("join-announce", pairing(false, None));
    let found = || {
        let mut driver = pairing(true, Some(8));
        until(&mut driver, |view| phase(view) == Some(Phase::Found));
        driver
    };
    push("add-found", found());
    let mut multiple = found();
    if let Some(pairing) = mesh(&mut multiple).view_mut().pairing.as_mut() {
        _ = pairing
            .candidates
            .push([0x48, 0xa1, 0xb2, 0xc3, 0x3f, 0x02]);
    }
    multiple.wait(100_000);
    push("add-multiple", multiple);
    let mut held = found();
    held.touch(Some(Point::new(233, 247)));
    held.wait(50_000);
    push("add-found-pressed", held);
    let mut preparing = found();
    tap(&mut preparing, 233, 247);
    push("add-preparing", preparing);
    let mut joining = pairing(false, None);
    until(&mut joining, |view| phase(view) == Some(Phase::Connecting));
    push("join-preparing", joining);

    for add in [true, false] {
        let role = if add { "add" } else { "join" };
        push(&format!("{role}-code"), code(add));
        let mut zero = code(add);
        force(&mut zero, Phase::Compare { code: 4_731 });
        push(&format!("{role}-code-leading-zero"), zero);
        let mut sliding = code(add);
        sliding.touch(Some(Point::new(149, 353)));
        for x in [170, 200, 241] {
            sliding.touch(Some(Point::new(x, 353)));
        }
        push(&format!("{role}-confirm-drag"), sliding);
        let mut stop = code(add);
        tap(&mut stop, 132, 115);
        push(&format!("{role}-stop-code"), stop);
        let mut waiting = code(add);
        accept(&mut waiting);
        push(&format!("{role}-waiting"), waiting);
        let mut transfer = code(add);
        accept(&mut transfer);
        until(&mut transfer, |view| {
            phase(view) == Some(Phase::Transfer { done: 1, total: 2 })
        });
        push(&format!("{role}-transfer"), transfer);
        let mut saving = code(add);
        accept(&mut saving);
        until(&mut saving, |view| phase(view) == Some(Phase::Storing));
        force(&mut saving, Phase::Storing);
        push(&format!("{role}-saving"), saving);
        let mut done = code(add);
        accept(&mut done);
        until(&mut done, |view| {
            matches!(phase(view), Some(Phase::Done(_)))
        });
        push(&format!("{role}-done"), done);
    }
    let mut finishing = code(false);
    accept(&mut finishing);
    until(&mut finishing, |view| phase(view) == Some(Phase::Finishing));
    force(&mut finishing, Phase::Finishing);
    push("join-finishing", finishing);
    let mut returning = code(true);
    accept(&mut returning);
    until(&mut returning, |view| {
        matches!(phase(view), Some(Phase::Done(_)))
    });
    force(
        &mut returning,
        Phase::Done(Done::Added {
            id: 7,
            returning: true,
        }),
    );
    push("add-returning", returning);
    let mut created = pairing(true, None);
    until(&mut created, |view| phase(view) == Some(Phase::Found));
    tap(&mut created, 233, 247);
    until(&mut created, |view| {
        matches!(phase(view), Some(Phase::Compare { .. }))
    });
    accept(&mut created);
    until(&mut created, |view| {
        matches!(phase(view), Some(Phase::Done(_)))
    });
    push("add-create-done", created);
    let mut first = code(false);
    accept(&mut first);
    until(&mut first, |view| {
        matches!(phase(view), Some(Phase::Done(_)))
    });
    if let Some(pairing) = mesh(&mut first).view_mut().pairing.as_mut() {
        pairing.group = Some((1, 2));
    }
    force(
        &mut first,
        Phase::Done(Done::Joined {
            id: 1,
            confirmed: true,
        }),
    );
    push("join-create-done", first);
    let mut lost = code(false);
    accept(&mut lost);
    until(&mut lost, |view| {
        matches!(phase(view), Some(Phase::Done(_)))
    });
    force(
        &mut lost,
        Phase::Done(Done::Joined {
            id: 2,
            confirmed: false,
        }),
    );
    push("join-confirmation-lost", lost);

    for (name, end, add) in [
        ("add-none", End::NotFound, true),
        ("add-timeout", End::TimedOut, true),
        ("add-rejected", End::Declined, true),
        ("add-peer-rejected", End::Peer(Reason::Declined), true),
        ("add-mismatch", End::Mismatch, true),
        ("add-cancelled", End::Cancelled, true),
        ("add-peer-cancelled", End::Peer(Reason::Cancelled), true),
        ("add-contact-lost", End::Lost, true),
        ("add-store-failed", End::StoreFailed, true),
        ("add-confirmation-lost", End::Unconfirmed, true),
        ("add-inauthentic", End::Inauthentic, true),
        ("join-none", End::NotFound, false),
        ("join-peer-rejected", End::Peer(Reason::Mismatch), false),
        ("join-store-failed", End::StoreFailed, false),
        ("join-malformed", End::Malformed, false),
    ] {
        let mut driver = pairing(add, add.then_some(8));
        force(&mut driver, Phase::Ended(end));
        push(name, driver);
    }
    let mut full = hub(Some(32));
    tap(&mut full, 307, 353);
    tap(&mut full, 156, 353);
    push("add-full", full);
    let mut no_radio = hub(None);
    mesh(&mut no_radio).view_mut().radio = false;
    no_radio.wait(50_000);
    tap(&mut no_radio, 156, 353);
    tap(&mut no_radio, 233, 353);
    push("add-no-radio", no_radio);
    let warning = || {
        let mut driver = hub(Some(8));
        tap(&mut driver, 307, 353);
        tap(&mut driver, 310, 353);
        driver
    };
    push("join-leave-warning", warning());
    let mut first = warning();
    tap(&mut first, 233, 353);
    push("join-leave-first", first);

    // A founder's wait for the joining device.
    let mut check = founding(Some(215), false);
    check.wait(138 * SECOND);
    push("founder-check-member", check);
    let mut pending = founding(Some(215), false);
    tap(&mut pending, 233, 353);
    pending.wait(138 * SECOND);
    push("founder-pending-hub", pending);
    let mut stored = founding(Some(215), false);
    stored.wait(216 * SECOND);
    push("founder-group-stored", stored);
    let mut stored_hub = founding(Some(215), false);
    stored_hub.wait(216 * SECOND);
    tap(&mut stored_hub, 233, 353);
    push("founder-stored-hub", stored_hub);
    let mut expired = founding(None, false);
    tap(&mut expired, 233, 353);
    expired.wait(601 * SECOND);
    push("founder-wait-expired", expired);
    let mut replace = founding(None, false);
    tap(&mut replace, 233, 353);
    tap(&mut replace, 233, 377);
    push("founder-replace-warning", replace);
    let mut failed = founding(Some(215), true);
    failed.wait(216 * SECOND);
    push("founder-save-failed", failed);
    let mut not_stored = founding(Some(215), true);
    not_stored.wait(601 * SECOND);
    push("founder-not-stored", not_stored);
    frames
}
