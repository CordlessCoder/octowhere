//! What the screens are shown of removals on simulated nodes: the request under way, its switch
//! and whether it can still be declined, a decline, and the notice to the device removed.

use octowhere_node::{
    Command,
    view::{Answer, Decline, RemovalStage, RemovalView, Request, Unremovable},
};
use octowhere_sim::{Config, Link, Sim, UTC0_S, grouped};

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

fn current(sim: &Sim, node: usize) -> Option<RemovalView> {
    sim.view(node).and_then(|view| view.removals.current)
}

#[test]
fn a_removal_shows_pending_then_switched_and_tells_the_device_removed() {
    let mut sim = group(3, 21);
    sim.command(0, Command::Remove(2));
    sim.run_for(1);
    let mine = current(&sim, 0).expect("the remover shows its request");
    assert_eq!((mine.remover, mine.removed), (0, 2));
    let RemovalStage::Pending { switch, .. } = mine.stage else {
        panic!("{:?}", mine.stage);
    };
    assert!(switch.is_some(), "its switch has a time");
    let learned = sim.run_while_not(20 * 60, |sim| {
        current(sim, 1).is_some_and(|theirs| theirs.key == mine.key)
    });
    assert!(learned, "after {} s", sim.now_s());
    let switched = sim.run_while_not(60 * 60, |sim| {
        matches!(
            current(sim, 1).map(|view| view.stage),
            Some(RemovalStage::Switched { .. })
        )
    });
    assert!(switched, "after {} s", sim.now_s());
    let Some(RemovalStage::Switched { decline, .. }) = current(&sim, 1).map(|view| view.stage)
    else {
        unreachable!();
    };
    assert!(matches!(decline, Decline::Until(_)), "{decline:?}");
    let own = sim.run_while_not(10 * 60, |sim| {
        matches!(
            current(sim, 0).map(|view| view.stage),
            Some(RemovalStage::Switched {
                decline: Decline::Own,
                ..
            })
        )
    });
    assert!(own, "the remover cannot decline its own");
    let told = sim.run_while_not(30 * 60, |sim| {
        sim.view(2)
            .is_some_and(|view| view.removals.removed_by.is_some_and(|(by, ..)| by == 0))
    });
    assert!(told, "after {} s", sim.now_s());
}

#[test]
fn a_removal_declined_before_its_switch_shows_declined() {
    let mut sim = group(3, 22);
    sim.command(0, Command::Remove(2));
    let learned = sim.run_while_not(20 * 60, |sim| current(sim, 1).is_some());
    assert!(learned, "after {} s", sim.now_s());
    let key = current(&sim, 1).unwrap().key;
    sim.command(1, Request::Keep { key }.into());
    sim.run_for(1);
    assert!(matches!(
        current(&sim, 1).map(|view| view.stage),
        Some(RemovalStage::Declined { .. })
    ));
}

#[test]
fn a_removal_names_its_target_by_device() {
    let mut sim = group(3, 23);
    // A fingerprint that is no member's removes nobody.
    sim.command(
        0,
        Request::Remove {
            id: 2,
            device: [0; 8],
        }
        .into(),
    );
    sim.run_for(5);
    assert!(current(&sim, 0).is_none());
    assert_eq!(sim.count(0, "is another device now"), 1);
}

#[test]
fn a_removal_answers_whether_it_started() {
    let mut sim = group(3, 24);
    let answer = |sim: &Sim| sim.view(0).and_then(|view| view.answer);
    sim.command(
        0,
        Request::Remove {
            id: 2,
            device: [0; 8],
        }
        .into(),
    );
    sim.run_for(1);
    assert_eq!(
        answer(&sim),
        Some(Answer::Removing(Err(Unremovable::Changed)))
    );
    sim.command(0, Command::Remove(2));
    sim.run_for(1);
    assert_eq!(answer(&sim), Some(Answer::Removing(Ok(()))));
    sim.command(0, Command::Remove(1));
    sim.run_for(1);
    assert!(
        matches!(
            answer(&sim),
            Some(Answer::Removing(Err(Unremovable::Underway { .. })))
        ),
        "{:?}",
        answer(&sim)
    );
}

/// Shown only in RAM, a removal this device can still decline comes back after a restart from
/// what the node stored, and declining it then still works.
#[test]
fn a_restart_within_the_day_shows_the_switch_to_decline_again() {
    let mut sim = group(3, 25);
    sim.command(0, Command::Remove(2));
    let switched = sim.run_while_not(60 * 60, |sim| {
        matches!(
            current(sim, 1).map(|view| view.stage),
            Some(RemovalStage::Switched { .. })
        )
    });
    assert!(switched, "after {} s", sim.now_s());
    let before = current(&sim, 1).unwrap();
    sim.restart(1);
    let shown = sim.run_while_not(10 * 60, |sim| current(sim, 1).is_some());
    assert!(shown, "after {} s", sim.now_s());
    let after = current(&sim, 1).unwrap();
    assert_eq!(
        (after.key, after.remover, after.removed, after.device),
        (before.key, before.remover, before.removed, before.device)
    );
    assert!(
        matches!(
            after.stage,
            RemovalStage::Switched {
                decline: Decline::Until(_),
                ..
            }
        ),
        "{:?}",
        after.stage
    );
    sim.command(1, Request::Keep { key: after.key }.into());
    sim.run_for(1);
    assert!(matches!(
        current(&sim, 1).map(|view| view.stage),
        Some(RemovalStage::Declined { .. })
    ));
    // Its record was forgotten with the switch, and comes back from the member itself, still
    // on the key this device went back to.
    let back = sim.run_while_not(20 * 60, |sim| {
        sim.view(1).is_some_and(|view| {
            view.group
                .as_ref()
                .is_some_and(|group| group.member(2).is_some())
        })
    });
    assert!(back, "after {} s", sim.now_s());
}

/// A node restarted out of everyone's reach hears nobody: the switch it can still decline comes
/// back as soon as its own timebase can time it.
#[test]
fn a_restart_out_of_reach_shows_the_switch_once_it_has_a_timebase() {
    let mut sim = group(3, 26);
    sim.command(0, Command::Remove(2));
    let switched = sim.run_while_not(60 * 60, |sim| {
        matches!(
            current(sim, 1).map(|view| view.stage),
            Some(RemovalStage::Switched { .. })
        )
    });
    assert!(switched, "after {} s", sim.now_s());
    for other in [0, 2] {
        sim.link(1, other, None);
        sim.link(other, 1, None);
    }
    let before = sim.count(1, "[MESH] timebase");
    sim.restart(1);
    let timed = sim.run_while_not(10 * 60, |sim| sim.count(1, "[MESH] timebase") > before);
    assert!(timed, "after {} s", sim.now_s());
    sim.run_for(1);
    assert!(current(&sim, 1).is_some(), "shown with its timebase");
}
