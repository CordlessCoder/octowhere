//! Scenarios on several simulated nodes.

use octowhere_mesh::members::Name;
use octowhere_node::Command;
use octowhere_sim::{Config, Link, Sim, UTC0_S, grouped};

/// Two nodes of one group, both with RTC time, in reach of each other.
fn pair(seed: u64) -> Sim {
    let mut sim = Sim::new(seed);
    for start in grouped(2, UTC0_S as u32 - 3_600) {
        sim.add(start, Config::default());
    }
    sim.link_all(Link::default());
    sim
}

#[test]
fn two_nodes_hear_each_other() {
    let mut sim = pair(1);
    let heard = sim.run_while_not(30 * 60, |sim| {
        sim.count(0, "heard id=1") >= 3 && sim.count(1, "heard id=0") >= 3
    });
    assert!(heard, "after {} s", sim.now_s());
}

#[test]
fn a_rename_reaches_the_other_node() {
    let mut sim = pair(2);
    sim.run_while_not(30 * 60, |sim| sim.count(0, "heard id=1") >= 1);
    sim.command(1, Command::Rename(Name::new(b"Roger Saved").unwrap()));
    let learned = sim.run_while_not(15 * 60, |sim| {
        sim.count(0, "member 1 is Roger Saved now") == 1
    });
    assert!(learned, "after {} s", sim.now_s());
}

#[test]
fn nodes_out_of_reach_hear_nothing() {
    let mut sim = Sim::new(3);
    for start in grouped(2, UTC0_S as u32 - 3_600) {
        sim.add(start, Config::default());
    }
    sim.run_for(30 * 60);
    assert_eq!(sim.count(0, "heard id="), 0);
    assert!(sim.count(0, "sent round=") > 0);
}

#[test]
fn a_run_repeats_from_its_seed() {
    let run = |seed| {
        let mut sim = pair(seed);
        sim.run_for(20 * 60);
        sim.lines(0)
            .into_iter()
            .chain(sim.lines(1))
            .map(|line| (line.at, line.text))
            .collect::<Vec<_>>()
    };
    assert_eq!(run(4), run(4));
}

/// A restart before a change was stored leaves two nodes' member digests apart, so each asks
/// the other for every record. Members enrolled meanwhile still reach the other node. (The
/// boards once starved one this way, records going lowest id first; this run's timing does not
/// reproduce that, which `compose`'s own test does.)
#[test]
fn members_enrolled_while_records_are_asked_for_reach_the_other_node() {
    let mut sim = pair(5);
    sim.run_while_not(30 * 60, |sim| sim.count(0, "heard id=1") >= 1);
    sim.command(1, Command::Rename(Name::new(b"Roger Saved").unwrap()));
    sim.run_while_not(15 * 60, |sim| {
        sim.count(0, "member 1 is Roger Saved now") == 1
    });
    sim.restart(0);
    let asked = sim.count(0, "asked=0xffffffff");
    let asking = sim.run_while_not(30 * 60, |sim| sim.count(0, "asked=0xffffffff") > asked);
    assert!(asking, "the restart left the digests apart");
    for _ in 0..3 {
        sim.command(0, Command::Phantom);
    }
    let learned = sim.run_while_not(30 * 60, |sim| sim.count(1, "is Phantom now") == 3);
    assert!(
        learned,
        "{} of 3 after {} s",
        sim.count(1, "is Phantom now"),
        sim.now_s()
    );
}

/// A member out of reach through two removals is caught up a generation at a time once it is
/// back, as two boards were in `docs/logs/lora/catch-up-2026-10-03/`.
#[test]
fn a_member_that_missed_two_switches_is_caught_up() {
    let mut sim = pair(6);
    sim.run_while_not(30 * 60, |sim| {
        sim.count(0, "heard id=1") >= 1 && sim.count(1, "heard id=0") >= 1
    });
    sim.command(0, Command::Phantom);
    sim.command(0, Command::Phantom);
    let learned = sim.run_while_not(30 * 60, |sim| sim.count(1, "is Phantom now") == 2);
    assert!(learned, "the phantoms reached node 1");
    sim.link(0, 1, None);
    sim.link(1, 0, None);
    for (phantom, switches) in [(2, 1), (3, 2)] {
        sim.command(0, Command::Remove(phantom));
        let switched = sim.run_while_not(60 * 60, |sim| {
            sim.count(0, "switched to generation") == switches
        });
        assert!(switched, "node 0 removed {phantom}");
    }
    let start = sim.now_s();
    sim.link_all(Link::default());
    let caught = sim.run_while_not(60 * 60, |sim| sim.count(1, "switched to generation") == 2);
    assert!(caught, "node 1 caught up after {} s", sim.now_s() - start);
    println!("caught up in {:.0} s", sim.now_s() - start);
}

/// A member that declines a removal after its switch goes back to the old key, stays there
/// however often the remover's key comes, and keeps that across a restart. It forgets the
/// records that changed since, the removed member's gone record among them, until a device
/// still on the old key sends them again: here there is none.
#[test]
fn a_removal_declined_after_its_switch_takes_the_decliner_back() {
    let mut sim = pair(7);
    sim.run_while_not(30 * 60, |sim| {
        sim.count(0, "heard id=1") >= 1 && sim.count(1, "heard id=0") >= 1
    });
    sim.command(0, Command::Phantom);
    sim.run_while_not(30 * 60, |sim| sim.count(1, "is Phantom now") == 1);
    sim.command(0, Command::Remove(2));
    let switched = sim.run_while_not(60 * 60, |sim| sim.count(1, "switched to generation 1") == 1);
    assert!(switched, "node 1 switched with node 0");
    sim.command(1, Command::Keep(2));
    let declined = sim.run_while_not(60, |sim| {
        sim.count(1, "declined after the switch: back on generation 0") == 1
    });
    assert!(declined);
    sim.run_for(30 * 60);
    assert_eq!(
        sim.count(1, "switched to generation"),
        1,
        "node 1 stayed back"
    );
    sim.restart(1);
    sim.run_for(1);
    assert_eq!(
        sim.count(1, "generation=0 removal pending=false"),
        2,
        "the decline was stored"
    );
}
