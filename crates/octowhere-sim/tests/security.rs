//! The open items of the 2026-10-03 security review, staged on simulated nodes. Each ignored
//! scenario fails today; the one beside it runs the same stage without the fault or the attack,
//! so that a failure is the defect's and not the stage's.

use octowhere_mesh::{
    members::Name,
    packet::{Builder, Header, MAX_PACKET, Source, Timebase},
    schedule::{GUARD_US, ROUND_US, SWEEP_EVERY, Schedule, base_of, round_at},
    seal::{self, Key, SIV_LEN},
};
use octowhere_node::Command;
use octowhere_sim::{Config, Link, Sim, Transmission, UTC0_S, grouped};

/// No group member stands at this id, so nothing else is sent in its slot.
const FREE_ID: u8 = 31;

/// `count` nodes of one group, all in reach of each other, recording the air, once each has
/// heard every other. Node `n` starts with `configs(n)`.
fn group(count: u8, seed: u64, configs: impl Fn(usize) -> Config) -> Sim {
    let mut sim = Sim::new(seed);
    sim.record(true);
    for (n, start) in grouped(count, UTC0_S as u32 - 3_600)
        .into_iter()
        .enumerate()
    {
        sim.add(start, configs(n));
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

/// Cuts `node` off from every other, both ways.
fn isolate(sim: &Sim, node: usize, count: usize) {
    for other in (0..count).filter(|&other| other != node) {
        sim.link(node, other, None);
        sim.link(other, node, None);
    }
}

/// The last packet `node` sent, as recorded.
fn last_sent(sim: &Sim, node: usize) -> Transmission {
    sim.recorded()
        .into_iter()
        .rev()
        .find(|sent| sent.sender == node)
        .expect("the node sent")
}

/// The round each of the first `count` nodes last logged it was in, and UTC's round then.
fn clocks(sim: &Sim, count: usize) -> String {
    (0..count)
        .map(|node| match last_round(sim, node) {
            Some((at, round)) => {
                let utc = round_at(UTC0_S * 1_000_000 + at as i64);
                format!("\n  node {node}: round {round}, UTC's {utc}")
            }
            None => format!("\n  node {node}: no round logged"),
        })
        .collect()
}

/// When `node` last logged the round it is in, and that round.
fn last_round(sim: &Sim, node: usize) -> Option<(u64, i64)> {
    sim.lines(node).into_iter().rev().find_map(|line| {
        let round = line.text.strip_prefix("[MESH] round=")?;
        let round = round.split(' ').next()?.parse().ok()?;
        Some((line.at, round))
    })
}

/// Whether `node`'s clock was on UTC's round when it last logged one.
fn on_utc(sim: &Sim, node: usize) -> bool {
    last_round(sim, node)
        .is_some_and(|(at, round)| round == round_at(UTC0_S * 1_000_000 + at as i64))
}

/// The packets that moved `node`'s clock by more than a slot's guard since virtual time `since`.
fn moves(sim: &Sim, node: usize, since: u64) -> String {
    sim.lines(node)
        .into_iter()
        .filter(|line| line.at >= since && line.text.contains("taken=Refined"))
        .filter(|line| {
            line.text
                .split_once("late_us=Some(")
                .and_then(|(_, late)| late.split_once(')'))
                .and_then(|(late, _)| late.parse::<i64>().ok())
                .is_some_and(|late| late.abs() > GUARD_US)
        })
        .map(|line| format!("\n  at {:.1} s: {}", line.at as f64 / 1e6, line.text))
        .collect()
}

/// Runs to `round`'s slot for an id nobody holds under `key`, on a timebase that keeps UTC, a
/// moment after it opens: a time a node in a sweep round hears a packet, and nobody else sends.
fn run_to_free_slot(sim: &mut Sim, key: &Key, round: i64) {
    let start = Schedule::new(key).slot_start(round, FREE_ID) + 100_000;
    sim.run_to((start - UTC0_S * 1_000_000) as u64);
}

/// The next sweep round on a timebase that keeps UTC.
fn next_sweep(sim: &Sim) -> i64 {
    let round = round_at(sim.utc_us()) + 1;
    round + (SWEEP_EVERY - round.rem_euclid(SWEEP_EVERY)) % SWEEP_EVERY
}

/// A packet under `key` that claims to come from `sender`, with nothing in it, sent at timebase
/// time `start`.
fn forged(key: &Key, sender: u8, start: i64) -> Vec<u8> {
    let header = Header {
        sender,
        timebase: Timebase {
            source: Source::Node(0),
            hops: Timebase::MAX_HOPS,
        },
        base: base_of(start),
        phase: 0,
        notice: false,
    };
    let mut packet = [0u8; MAX_PACKET];
    let plain_len = Builder::new(&mut packet[SIV_LEN..], &header).finish();
    let len = seal::seal(key, &mut packet, plain_len);
    packet[..len].to_vec()
}

/// Three nodes, none with a fix. Node 0's RTC holds the time unless `lost`. A rename at node 2
/// reaches the others.
fn rename_with_node_0s_rtc(lost: bool) {
    let mut sim = group(3, 78, |node| Config {
        rtc_error_us: (node != 0 || !lost).then_some(0),
        ..Config::default()
    });
    sim.command(2, Command::Rename(Name::new(b"Roger Saved").unwrap()));
    let learned = sim.run_while_not(30 * 60, |sim| {
        (0..2).all(|node| sim.count(node, "member 2 is Roger Saved now") == 1)
    });
    assert!(learned, "after {} s:{}", sim.now_s(), clocks(&sim, 3));
}

#[test]
fn a_rename_reaches_the_group() {
    rename_with_node_0s_rtc(false);
}

/// Node 0's RTC lost its time. Its first sweep hears nobody, so it starts its clock from its
/// boot, near 1970. Its id is the lowest, but a clock started from UTC outranks one started from
/// a boot, so node 0 moves to the others' clock rather than they to its, and every node judges
/// node 2's rename by UTC. (They once moved to node 0's, and refused the rename as an hour
/// ahead.)
#[test]
fn a_rename_reaches_a_group_whose_lowest_id_lost_its_rtc_time() {
    rename_with_node_0s_rtc(true);
}

/// Node 1 removes a member. Node 0 restarts out of reach after learning of it, with its RTC's
/// time lost unless `kept`, and comes back. Both switch.
fn removal_across_node_0s_restart(kept: bool) {
    let mut sim = group(2, 79, |_| Config::default());
    sim.command(0, Command::Phantom);
    let learned = sim.run_while_not(30 * 60, |sim| sim.count(1, "is Phantom now") == 1);
    assert!(learned, "node 1 holds the phantom");
    sim.command(1, Command::Remove(2));
    let asked = sim.run_while_not(30 * 60, |sim| sim.count(0, "asks to remove 2") == 1);
    assert!(asked, "node 0 learned of the removal");
    isolate(&sim, 0, 2);
    sim.set_rtc(0, kept.then_some(0));
    sim.restart(0);
    sim.run_for(5 * 60);
    sim.link_all(Link::default());
    let switched = sim.run_while_not(3 * 60 * 60, |sim| {
        (0..2).all(|node| sim.count(node, "switched to generation 1") == 1)
    });
    assert!(switched, "after {} s:{}", sim.now_s(), clocks(&sim, 2));
}

#[test]
fn a_removal_switches_across_a_restart_out_of_reach() {
    removal_across_node_0s_restart(true);
}

/// As above, but node 0's battery ran flat and its RTC lost the time. It restarts its clock from
/// its boot, near 1970, where the switch round it stored, counted in 2026, never comes. Node 1
/// has switched, so node 0 hears it only under the key to switch to, and takes its clock from
/// that, which outranks its own; its switch then comes at once.
#[test]
fn a_removal_switches_across_a_restart_that_lost_the_rtc_time() {
    removal_across_node_0s_restart(false);
}

/// How node 2's absence through a removal is exploited.
#[derive(Clone, Copy, PartialEq)]
enum Absence {
    Quiet,
    /// Someone replays a packet node 2 sent before it went, which needs no key.
    Replayed,
    /// A member sends under the new key with node 2's id.
    Forged,
}

/// Node 2 is out of reach while node 0 removes a member, and comes back after the switch. It is
/// caught up.
fn catch_up_after(absence: Absence) {
    let mut sim = group(3, 80, |_| Config::default());
    sim.command(0, Command::Phantom);
    let learned = sim.run_while_not(30 * 60, |sim| {
        (1..3).all(|node| sim.count(node, "is Phantom now") == 1)
    });
    assert!(learned, "every node holds the phantom");
    let recorded = last_sent(&sim, 2);
    isolate(&sim, 2, 3);
    sim.command(0, Command::Remove(3));
    let switched = sim.run_while_not(60 * 60, |sim| {
        (0..2).all(|node| sim.count(node, "switched to generation 1") == 1)
    });
    assert!(switched, "nodes 0 and 1 switched");
    let radio = sim.add_radio();
    for node in 0..2 {
        sim.link(radio, node, Some(Link::default()));
    }
    let (key, _) = sim.key(0).expect("node 0 is in the group");
    if absence != Absence::Quiet {
        for _ in 0..6 {
            let round = next_sweep(&sim);
            run_to_free_slot(&mut sim, &key, round);
            let bytes = match absence {
                Absence::Replayed => recorded.bytes.clone(),
                _ => forged(&key, 2, round * ROUND_US),
            };
            sim.transmit(radio, &bytes, recorded.channel);
            sim.run_for(ROUND_US as u64 / 1_000_000);
        }
    }
    sim.link(radio, 0, None);
    sim.link(radio, 1, None);
    let start = sim.now_s();
    for node in 0..2 {
        sim.link(node, 2, Some(Link::default()));
        sim.link(2, node, Some(Link::default()));
    }
    let caught = sim.run_while_not(2 * 60 * 60, |sim| {
        sim.count(2, "switched to generation 1") == 1
    });
    let gave_up = (0..2)
        .map(|node| {
            format!(
                "\n  node {node}: gave up sending {} times, dropped the old key {} times",
                sim.count(node, "has been sent its key message 3 times"),
                sim.count(node, "the old one is dropped"),
            )
        })
        .collect::<String>();
    assert!(
        caught,
        "node 2, {} s after it came back:{gave_up}",
        sim.now_s() - start
    );
}

#[test]
fn a_member_back_after_a_removal_is_caught_up() {
    catch_up_after(Absence::Quiet);
}

/// A replayed packet of node 2's, under the old key, makes each node send node 2 its key message
/// in a sweep round. A node sends it three times at most, so the replays spend every one before
/// node 2 is back.
#[test]
#[ignore = "replayed packets spend a member's catch-ups"]
fn replayed_packets_do_not_spend_a_members_catch_ups() {
    catch_up_after(Absence::Replayed);
}

/// A packet under the new key from node 2's id tells each node node 2 switched, so they stop
/// waiting for it and drop the old key, which node 2's key message would have gone under.
#[test]
#[ignore = "header sender ids are not authenticated within the group"]
fn a_member_cannot_end_the_wait_for_another() {
    catch_up_after(Absence::Forged);
}

#[derive(Clone, Copy)]
enum Replay {
    None,
    /// While node 1 is its own root, which node 0's clock outranks.
    ToARoot,
    /// In a sweep round, while node 1 still times from node 0.
    InASweep,
    /// As `InASweep`, of node 0's last packet before the sweep round, minutes old.
    RecentInASweep,
}

/// Node 0, the root, sends a packet that is recorded; later it is replayed to node 1, as
/// `replay` says. Node 1's RTC holds the time if `rtc`. Node 1's clock stays on UTC, which node
/// 0's RTC holds.
fn clock_after_replay(replay: Replay, rtc: bool) {
    let mut sim = group(2, 81, |node| Config {
        rtc_error_us: (node == 0 || rtc).then_some(0),
        ..Config::default()
    });
    let on_node_0 = sim.run_while_not(60 * 60, |sim| sim.count(1, "timebase root=0 hops=1") >= 1);
    assert!(on_node_0, "node 1 took node 0's clock");
    let mut recorded = last_sent(&sim, 0);
    let radio = sim.add_radio();
    sim.link(radio, 1, Some(Link::default()));
    let (key, _) = sim.key(1).expect("node 1 is in the group");
    match replay {
        Replay::None => sim.run_for(60 * 60),
        Replay::ToARoot => {
            sim.power_off(0);
            sim.run_for(60 * 60);
            assert!(
                sim.count(1, "timebase root=1 hops=0") >= 1,
                "node 1 lost node 0 and became its own root"
            );
            // In node 0's slot, which node 1 listens to.
            let round = round_at(sim.utc_us()) + 1;
            let start = Schedule::new(&key).slot_start(round, 0) + 100_000;
            sim.run_to((start - UTC0_S * 1_000_000) as u64);
        }
        Replay::InASweep | Replay::RecentInASweep => {
            sim.run_for(60 * 60);
            let round = next_sweep(&sim);
            run_to_free_slot(&mut sim, &key, round);
            if matches!(replay, Replay::RecentInASweep) {
                recorded = last_sent(&sim, 0);
            }
        }
    }
    let at = sim.now_us();
    if !matches!(replay, Replay::None) {
        sim.transmit(radio, &recorded.bytes, recorded.channel);
    }
    sim.run_for(12 * ROUND_US as u64 / 1_000_000);
    let moved = moves(&sim, 1, at);
    assert!(
        moved.is_empty() && on_utc(&sim, 1),
        "{:.0} s after the packet was recorded:{}{moved}",
        at as f64 / 1e6 - recorded.start as f64 / 1e6,
        clocks(&sim, 2)
    );
}

#[test]
fn a_clock_keeps_to_utc() {
    clock_after_replay(Replay::None, true);
    clock_after_replay(Replay::None, false);
}

/// A packet from a timebase ranked above the node's own was adopted whenever it came, with no
/// check that it fit. Node 1 then lost node 0 again, swept, and became its own root on the
/// clock the replay gave it, an hour behind. Now node 1's RTC refuses a clock an hour off it,
/// and without RTC time a move that large waits for a second packet, which never comes.
#[test]
fn a_replayed_packet_from_a_higher_root_does_not_move_a_clock() {
    clock_after_replay(Replay::ToARoot, true);
    clock_after_replay(Replay::ToARoot, false);
}

/// In a sweep round, a packet closer to the node's root refined its clock however far off it
/// was, and node 1 lost node 0 until a later sweep round. Now the RTC refuses it, and without
/// RTC time, or for a replay minutes old that the RTC lets by, it waits for a second packet
/// that agrees: node 0's own live packets do not.
#[test]
fn a_replayed_packet_in_a_sweep_does_not_set_a_clock() {
    clock_after_replay(Replay::InASweep, true);
    clock_after_replay(Replay::InASweep, false);
    clock_after_replay(Replay::RecentInASweep, true);
}
