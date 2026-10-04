//! Scenarios on several simulated nodes.

use octowhere_mesh::members::Name;
use octowhere_node::Command;
use octowhere_sim::{Config, Link, Sim, UTC0_S, alone, grouped};

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

/// A node restarted between learning of a removal and its switch still switches with the
/// group, from the removal it stored.
#[test]
fn a_restart_during_a_removal_still_switches() {
    let mut sim = pair(8);
    sim.run_while_not(30 * 60, |sim| {
        sim.count(0, "heard id=1") >= 1 && sim.count(1, "heard id=0") >= 1
    });
    sim.command(0, Command::Phantom);
    sim.run_while_not(30 * 60, |sim| sim.count(1, "is Phantom now") == 1);
    sim.command(0, Command::Remove(2));
    let asked = sim.run_while_not(30 * 60, |sim| sim.count(1, "asks to remove 2") == 1);
    assert!(asked);
    sim.restart(1);
    sim.run_for(1);
    assert_eq!(
        sim.count(1, "removal pending=true"),
        1,
        "the removal was stored"
    );
    let switched = sim.run_while_not(60 * 60, |sim| {
        sim.count(0, "switched to generation 1") == 1
            && sim.count(1, "switched to generation 1") == 1
    });
    assert!(switched, "after {} s", sim.now_s());
}

/// A device in no group joins one of two through pairing, and the member that did not pair
/// learns of it over the mesh.
#[test]
fn a_device_joins_through_pairing() {
    let mut sim = pair(9);
    let joiner = sim.add(alone(2), Config::default());
    sim.link_all(Link::default());
    sim.run_while_not(30 * 60, |sim| {
        sim.count(0, "heard id=1") >= 1 && sim.count(1, "heard id=0") >= 1
    });
    sim.command(0, Command::Add);
    sim.command(joiner, Command::Join);
    let found = sim.run_while_not(5 * 60, |sim| sim.count(0, "[PAIR] found 0") >= 1);
    assert!(found, "node 0 found the joining device");
    sim.command(0, Command::Choose(0));
    let compared = sim.run_while_not(5 * 60, |sim| {
        sim.count(0, "[PAIR] code") == 1 && sim.count(joiner, "[PAIR] code") == 1
    });
    assert!(compared, "both show a code");
    sim.command(0, Command::Accept);
    sim.command(joiner, Command::Accept);
    let done = sim.run_while_not(5 * 60, |sim| {
        sim.count(0, "Done(Added") == 1 && sim.count(joiner, "Done(Joined") == 1
    });
    assert!(done, "the pairing finished");
    let learned = sim.run_while_not(60 * 60, |sim| sim.count(1, "member 2 is OW-0002 now") == 1);
    assert!(
        learned,
        "node 1 learned of the new member after {} s",
        sim.now_s()
    );
}

/// The refresh session a node's view shows.
fn refresh_session(sim: &Sim, node: usize) -> Option<u32> {
    sim.view(node)?.refresh.map(|refresh| refresh.session)
}

/// A node that leaves its group and founds another goes on numbering its refreshes, which the
/// screens tell apart by number.
#[test]
fn refreshes_count_on_across_leaving_and_founding() {
    let mut sim = pair(10);
    sim.run_while_not(30 * 60, |sim| sim.count(1, "heard id=0") >= 1);
    sim.command(1, Command::Refresh);
    let ended = sim.run_while_not(10 * 60, |sim| sim.count(1, "refresh ended") == 1);
    assert!(ended, "the first refresh ended");
    assert_eq!(refresh_session(&sim, 1), Some(1));

    sim.command(1, Command::Leave);
    let left = sim.run_while_not(10 * 60, |sim| sim.count(1, "left the group") == 1);
    assert!(left, "node 1 left");
    let joiner = sim.add(alone(2), Config::default());
    sim.link_all(Link::default());
    sim.command(1, Command::Add);
    sim.command(joiner, Command::Join);
    let found = sim.run_while_not(5 * 60, |sim| sim.count(1, "[PAIR] found 0") >= 1);
    assert!(found, "node 1 found the joining device");
    sim.command(1, Command::Choose(0));
    let compared = sim.run_while_not(5 * 60, |sim| {
        sim.count(1, "[PAIR] code") == 1 && sim.count(joiner, "[PAIR] code") == 1
    });
    assert!(compared, "both show a code");
    sim.command(1, Command::Accept);
    sim.command(joiner, Command::Accept);
    let done = sim.run_while_not(5 * 60, |sim| {
        sim.count(1, "Done(Added") == 1 && sim.count(joiner, "Done(Joined") == 1
    });
    assert!(done, "node 1 founded a group");
    sim.run_for(10);
    assert_eq!(refresh_session(&sim, 1), None);

    sim.command(1, Command::Refresh);
    sim.run_for(10);
    assert_eq!(refresh_session(&sim, 1), Some(2));
}

/// `count` nodes of one group, all in reach of each other, once each has heard every other.
fn group(count: u8, seed: u64) -> Sim {
    let mut sim = Sim::new(seed);
    for start in grouped(count, UTC0_S as u32 - 3_600) {
        sim.add(start, Config::default());
    }
    sim.link_all(Link::default());
    let nodes = usize::from(count);
    let met = sim.run_while_not(60 * 60, |sim| {
        (0..nodes).all(|node| {
            (0..nodes)
                .filter(|&other| other != node)
                .all(|other| sim.count(node, &format!("heard id={other} ")) >= 1)
        })
    });
    assert!(met, "the nodes met");
    sim
}

/// Has node 0 enrol `count` phantoms, at the ids after the nodes', and waits until every node
/// holds them.
fn phantoms(sim: &mut Sim, nodes: usize, count: usize) {
    for _ in 0..count {
        sim.command(0, Command::Phantom);
    }
    let learned = sim.run_while_not(60 * 60, |sim| {
        (1..nodes).all(|node| sim.count(node, "is Phantom now") == count)
    });
    assert!(learned, "every node holds the phantoms");
}

/// Cuts the links between nodes 0 and 1 and nodes 2 and 3.
fn split(sim: &Sim) {
    for (a, b) in [(0, 2), (0, 3), (1, 2), (1, 3)] {
        sim.link(a, b, None);
        sim.link(b, a, None);
    }
}

/// Whether every one of `nodes` stored one key of generation `generation`, and members at the
/// ids in the set `members`.
fn settled(sim: &Sim, nodes: usize, generation: u16, members: u32) -> bool {
    let first = sim.key(0);
    first.as_ref().is_some_and(|(_, held)| *held == generation)
        && (0..nodes).all(|node| sim.key(node) == first && sim.members(node) == members)
}

/// What each node stored, for a failed assertion.
fn stored(sim: &Sim, nodes: usize) -> String {
    (0..nodes)
        .map(|node| {
            let (key, generation) = sim.key(node).unwrap();
            format!(
                "\n  node {node}: generation {generation} key {:02x?} members {:#010x}",
                &key.bytes()[..4],
                sim.members(node)
            )
        })
        .collect()
}

/// Two members remove a member each at the same time, in reach of each other. Every node takes
/// the lower remover's key before either switches, and the other remover removes its member
/// again under it.
#[test]
fn rival_removals_in_reach_settle_before_the_switch() {
    let mut sim = group(3, 10);
    phantoms(&mut sim, 3, 2);
    sim.command(1, Command::Remove(4));
    sim.command(0, Command::Remove(3));
    let settled = sim.run_while_not(60 * 60, |sim| settled(sim, 3, 2, 0b111));
    assert!(settled, "after {} s:{}", sim.now_s(), stored(&sim, 3));
    for node in 0..3 {
        assert_eq!(
            sim.count(node, "switched to generation 1: 0 removed Some(3)"),
            1
        );
        assert_eq!(
            sim.count(node, "switched to generation 2: 1 removed Some(4)"),
            1
        );
    }
}

/// As above, with the removers out of reach of each other but for a third node between them.
#[test]
fn rival_removals_through_a_relay_settle_before_the_switch() {
    let mut sim = group(3, 12);
    phantoms(&mut sim, 3, 2);
    sim.link(0, 2, None);
    sim.link(2, 0, None);
    sim.command(2, Command::Remove(4));
    sim.command(0, Command::Remove(3));
    let settled = sim.run_while_not(60 * 60, |sim| settled(sim, 3, 2, 0b111));
    assert!(settled, "after {} s:{}", sim.now_s(), stored(&sim, 3));
    assert!((0..3).all(|node| sim.count(node, "a rival key won") == 0));
}

/// Two parts of a group apart remove a member each, and each switches to its own key. Once they
/// meet, the part on the higher remover's key takes the lower's, with the member its own key
/// removed back, and its remover removes that member again.
#[test]
fn rival_removals_apart_settle_once_the_parts_meet() {
    for seed in 20..28 {
        let mut sim = group(4, seed);
        phantoms(&mut sim, 4, 2);
        split(&sim);
        sim.command(0, Command::Remove(4));
        sim.command(2, Command::Remove(5));
        let switched = sim.run_while_not(60 * 60, |sim| {
            (0..4).all(|node| sim.count(node, "switched to generation 1") == 1)
        });
        assert!(switched, "seed {seed}: both parts switched");
        sim.link_all(Link::default());
        let start = sim.now_s();
        let settled = sim.run_while_not(3 * 60 * 60, |sim| settled(sim, 4, 2, 0b1111));
        assert!(
            settled,
            "seed {seed}, {} s after they met:{}",
            sim.now_s() - start,
            stored(&sim, 4)
        );
        assert!(
            (2..4).all(|node| sim.count(node, "a rival key won; 5 is a member again") == 1),
            "seed {seed}"
        );
    }
}

/// The remover whose removal lost restarts as it learns so, and still removes its member again.
#[test]
fn a_removal_lost_to_a_rival_is_made_again_after_a_restart() {
    let mut sim = group(4, 404);
    phantoms(&mut sim, 4, 2);
    split(&sim);
    sim.command(0, Command::Remove(4));
    sim.command(2, Command::Remove(5));
    sim.run_while_not(60 * 60, |sim| {
        (0..4).all(|node| sim.count(node, "switched to generation 1") == 1)
    });
    sim.link_all(Link::default());
    let lost = sim.run_while_not(3 * 60 * 60, |sim| {
        sim.count(2, "this device's removal lost") == 1
    });
    assert!(lost);
    assert_eq!(sim.count(2, "removing 5: generation 2"), 0, "not yet");
    sim.restart(2);
    let settled = sim.run_while_not(3 * 60 * 60, |sim| settled(sim, 4, 2, 0b1111));
    assert!(settled, "after {} s:{}", sim.now_s(), stored(&sim, 4));
}

/// Two parts apart, one of which switches twice. The part ahead does not go back past its own
/// switches to the rival that won the first, so they stay apart (`LORA-PROTOCOL.md`, "Open").
#[test]
#[ignore = "a part two removals ahead keeps its own keys"]
fn parts_apart_through_two_removals_settle_once_they_meet() {
    let mut sim = group(4, 300);
    phantoms(&mut sim, 4, 3);
    split(&sim);
    sim.command(0, Command::Remove(6));
    sim.command(2, Command::Remove(4));
    sim.run_while_not(60 * 60, |sim| {
        (0..4).all(|node| sim.count(node, "switched to generation 1") == 1)
    });
    sim.command(2, Command::Remove(5));
    sim.run_while_not(60 * 60, |sim| {
        (2..4).all(|node| sim.count(node, "switched to generation 2") == 1)
    });
    sim.link_all(Link::default());
    let settled = sim.run_while_not(12 * 60 * 60, |sim| {
        let first = sim.key(0);
        (1..4).all(|node| sim.key(node) == first && sim.members(node) == sim.members(0))
    });
    assert!(settled, "{}", stored(&sim, 4));
}
