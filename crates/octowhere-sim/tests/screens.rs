//! Devices whose screens drive their nodes, as the simulators show several side by side: each
//! a stage on `octowhere-ui-script`'s driver, stepped on its node's clock, taking the views the
//! node publishes and passing on the requests its screens make.

use std::{cell::RefCell, rc::Rc};

use embedded_graphics::prelude::Point;
use octowhere_sim::{Config, Link, Sim, alone};
use octowhere_ui::ui::{
    gesture::Micros,
    group::view::{Done, MeshView, Phase, Request},
    rest::Timeout,
    screens::{PeripheralState, Screen},
    stage::Stage,
};
use octowhere_ui_script::{Driver, MeshLink};

/// A device's node on the shared air.
struct Node {
    sim: Rc<RefCell<Sim>>,
    node: usize,
    seen: u32,
}

impl MeshLink for Node {
    fn view(&mut self, now: Micros) -> Option<MeshView> {
        let mut sim = self.sim.borrow_mut();
        // Every node started at time zero without drift, so its clock is the air's.
        sim.run_to(now);
        let (view, seen) = sim.view_since(self.node, self.seen)?;
        self.seen = seen;
        Some(view)
    }

    fn request(&mut self, request: Request, _: Micros) {
        self.sim.borrow().command(self.node, request.into());
    }
}

/// Several devices, each a driver on its own node.
struct Devices {
    sim: Rc<RefCell<Sim>>,
    drivers: Vec<Driver<'static>>,
}

impl Devices {
    /// `count` devices alone, in reach of each other, each awake on its clock face.
    fn alone(count: u8, seed: u64) -> Self {
        let sim = Rc::new(RefCell::new(Sim::new(seed)));
        let drivers = (0..count)
            .map(|n| {
                let node = sim.borrow_mut().add(alone(n), Config::default());
                let mut driver = Driver::new();
                driver.stage = Stage::new(PeripheralState {
                    timeout: Timeout::Never,
                    ..PeripheralState::default()
                });
                driver.stage.show(Screen::Clock);
                driver.link = Some(Box::new(Node {
                    sim: sim.clone(),
                    node,
                    seen: 0,
                }));
                driver
            })
            .collect();
        sim.borrow().link_all(Link::default());
        let mut devices = Self { sim, drivers };
        devices.wait(600_000);
        devices
    }

    /// Steps every device on until each has run `duration` past the furthest, the one behind
    /// first, so that none runs far ahead of the air.
    fn wait(&mut self, duration: Micros) {
        let end = self.now() + duration;
        while let Some(behind) = self
            .drivers
            .iter_mut()
            .filter(|driver| driver.now() < end)
            .min_by_key(|driver| driver.now())
        {
            behind.wait(1);
        }
    }

    fn now(&self) -> Micros {
        self.drivers.iter().map(Driver::now).max().unwrap_or(0)
    }

    /// Lets device `n` act, then brings the others up to its time.
    fn on(&mut self, n: usize, act: impl FnOnce(&mut Driver)) {
        act(&mut self.drivers[n]);
        self.wait(0);
    }

    fn tap(&mut self, n: usize, x: i32, y: i32) {
        self.on(n, |driver| {
            driver.tap(Point::new(x, y));
        });
        self.wait(300_000);
    }

    /// The slide that confirms a code, on device `n`.
    fn accept(&mut self, n: usize) {
        self.on(n, |driver| {
            driver.swipe(Point::new(149, 353), Point::new(330, 353), 300_000);
        });
        self.wait(50_000);
    }

    /// Opens the group screen from the panel's second page, on device `n`.
    fn open_group(&mut self, n: usize) {
        self.on(n, |driver| {
            driver.swipe(Point::new(233, 80), Point::new(233, 420), 250_000);
            driver.settle();
            driver.wait(600_000);
            driver.swipe(Point::new(380, 250), Point::new(80, 250), 300_000);
            driver.settle();
            driver.wait(400_000);
        });
        self.tap(n, 233, 190);
    }

    fn phase(&self, n: usize) -> Option<Phase> {
        self.drivers[n]
            .stage
            .mesh()
            .pairing
            .as_ref()
            .map(|pairing| pairing.phase)
    }

    /// Waits until `test` holds, for at most `seconds` of the air's time.
    fn until(&mut self, seconds: u64, test: impl Fn(&Self) -> bool) -> bool {
        let end = self.now() + seconds * 1_000_000;
        while self.now() < end {
            if test(self) {
                return true;
            }
            self.wait(100_000);
        }
        test(self)
    }
}

#[test]
fn two_devices_pair_through_their_screens() {
    let mut devices = Devices::alone(2, 31);
    devices.open_group(0);
    devices.open_group(1);
    // ADD on the first, JOIN on the second, then START on each.
    devices.tap(0, 156, 353);
    devices.tap(0, 233, 353);
    devices.tap(1, 310, 353);
    devices.tap(1, 233, 353);
    let found = devices.until(5 * 60, |devices| devices.phase(0) == Some(Phase::Found));
    assert!(
        found,
        "the adding device found the joining one: {:?}",
        devices.phase(0)
    );
    devices.tap(0, 233, 247);
    let compared = devices.until(5 * 60, |devices| {
        (0..2).all(|n| matches!(devices.phase(n), Some(Phase::Compare { .. })))
    });
    assert!(
        compared,
        "both show the code: {:?} {:?}",
        devices.phase(0),
        devices.phase(1)
    );
    devices.accept(0);
    devices.accept(1);
    let done = devices.until(5 * 60, |devices| {
        matches!(devices.phase(0), Some(Phase::Done(Done::Added { .. })))
            && matches!(devices.phase(1), Some(Phase::Done(Done::Joined { .. })))
    });
    assert!(
        done,
        "the pairing finished: {:?} {:?}",
        devices.phase(0),
        devices.phase(1)
    );
    let sim = devices.sim.borrow();
    assert!(
        sim.key(0).is_some() && sim.key(0) == sim.key(1),
        "both stored one key"
    );
    assert_eq!(sim.members(0), 0b11);
    assert_eq!(sim.members(1), 0b11);
}
