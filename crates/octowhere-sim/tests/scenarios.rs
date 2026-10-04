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
