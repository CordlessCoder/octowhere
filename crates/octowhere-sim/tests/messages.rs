//! What the screens are shown of messages on simulated nodes: each one's text, and how far it
//! has gone as far as its device can tell.

use octowhere_node::{
    Command,
    view::{Carriage, MessageView, Text},
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

fn send(sim: &Sim, node: usize, to: Option<u8>, text: &str) {
    let text = Text::new(text.as_bytes()).expect("printable text");
    sim.command(node, Command::Send { to, text });
}

/// The message on `node` with this text.
fn message(sim: &Sim, node: usize, text: &str) -> Option<MessageView> {
    sim.messages(node)
        .iter()
        .find(|message| message.text() == text)
        .copied()
}

fn carriage(sim: &Sim, node: usize, text: &str) -> Option<Carriage> {
    message(sim, node, text).map(|message| message.carriage)
}

/// Three nodes in a line, 0 and 2 out of each other's reach, once 1 has heard both.
fn line(seed: u64) -> Sim {
    let mut sim = Sim::new(seed);
    for start in grouped(3, UTC0_S as u32 - 3_600) {
        sim.add(start, Config::default());
    }
    sim.link_all(Link::default());
    sim.link(0, 2, None);
    sim.link(2, 0, None);
    let met = sim.run_while_not(60 * 60, |sim| {
        sim.count(1, "heard id=0 ") >= 1 && sim.count(1, "heard id=2 ") >= 1
    });
    assert!(met, "the middle node heard both ends");
    sim
}

/// The middle node relays a message for the far end, out of its origin's reach, which the
/// origin hears. Two nodes in reach of each other never relay: the origin's own packet covered
/// everyone.
#[test]
fn a_group_message_is_queued_then_sent_then_heard_relayed() {
    let mut sim = line(11);
    send(&sim, 0, None, "Meet at the bridge.");
    // Shorter than any packet's airtime.
    sim.run_to(sim.now_us() + 1_000);
    assert_eq!(
        carriage(&sim, 0, "Meet at the bridge."),
        Some(Carriage::Queued)
    );
    let sent = sim.run_while_not(10 * 60, |sim| {
        carriage(sim, 0, "Meet at the bridge.") != Some(Carriage::Queued)
    });
    assert!(sent, "after {} s", sim.now_s());
    assert_eq!(
        carriage(&sim, 0, "Meet at the bridge."),
        Some(Carriage::Sent)
    );
    let arrived = sim.run_while_not(10 * 60, |sim| {
        message(sim, 1, "Meet at the bridge.").is_some()
    });
    assert!(arrived, "after {} s", sim.now_s());
    let received = message(&sim, 1, "Meet at the bridge.").unwrap();
    assert_eq!(
        (
            received.carriage,
            received.from,
            received.to,
            received.unread,
            received.recovered
        ),
        (Carriage::Received, 0, None, true, false)
    );
    let relayed = sim.run_while_not(10 * 60, |sim| {
        carriage(sim, 0, "Meet at the bridge.") == Some(Carriage::Relayed)
    });
    assert!(relayed, "after {} s", sim.now_s());
    // A group message is never shown delivered.
    sim.run_for(10 * 60);
    assert_eq!(
        carriage(&sim, 0, "Meet at the bridge."),
        Some(Carriage::Relayed)
    );
}

#[test]
fn a_private_message_is_delivered_once_acknowledged() {
    let mut sim = group(3, 12);
    send(&sim, 0, Some(1), "Take the north path.");
    let delivered = sim.run_while_not(20 * 60, |sim| {
        carriage(sim, 0, "Take the north path.") == Some(Carriage::Delivered)
    });
    assert!(delivered, "after {} s", sim.now_s());
    let received = message(&sim, 1, "Take the north path.").expect("its destination reads it");
    assert_eq!((received.from, received.to), (0, Some(1)));
    assert!(
        message(&sim, 2, "Take the north path.").is_none(),
        "another member cannot read it"
    );
}

#[test]
fn reading_a_message_clears_it_unread() {
    let mut sim = group(2, 13);
    send(&sim, 0, None, "On my way.");
    let arrived = sim.run_while_not(10 * 60, |sim| message(sim, 1, "On my way.").is_some());
    assert!(arrived);
    let id = message(&sim, 1, "On my way.").unwrap().id;
    sim.command(1, Command::Read(id));
    sim.run_for(1);
    assert!(!message(&sim, 1, "On my way.").unwrap().unread);
}

#[test]
fn messages_come_back_after_a_restart_neither_new_nor_unread() {
    let mut sim = group(2, 14);
    send(&sim, 0, None, "North path is clear.");
    send(&sim, 1, Some(0), "See you there.");
    let exchanged = sim.run_while_not(20 * 60, |sim| {
        message(sim, 1, "North path is clear.").is_some()
            && carriage(sim, 1, "See you there.") == Some(Carriage::Delivered)
    });
    assert!(exchanged, "after {} s", sim.now_s());
    sim.restart(1);
    let back = sim.run_while_not(60 * 60, |sim| {
        message(sim, 1, "North path is clear.").is_some()
            && message(sim, 1, "See you there.").is_some()
    });
    assert!(back, "after {} s", sim.now_s());
    let theirs = message(&sim, 1, "North path is clear.").unwrap();
    assert!(theirs.recovered && !theirs.unread);
    let own = message(&sim, 1, "See you there.").unwrap();
    assert_eq!((own.from, own.to, own.recovered), (1, Some(0), true));
    assert_eq!(
        own.carriage,
        Carriage::Delivered,
        "its acknowledgement came back too"
    );
}

/// A private message from the screens names its recipient's device as well as its id, and goes
/// only while that id still holds that device: one taken by another device after a removal
/// never receives a draft written to the first.
#[test]
fn a_private_message_goes_only_to_the_device_it_was_written_to() {
    let mut sim = group(2, 12);
    let device = sim
        .view(0)
        .and_then(|view| Some(view.group?.member(1)?.device))
        .expect("node 0 knows node 1");
    let text = |text: &str| Text::new(text.as_bytes()).expect("printable text");
    let mut other = device;
    other[0] ^= 1;
    sim.command(
        0,
        Command::SendDevice {
            id: 1,
            device: other,
            text: text("Not for you."),
        },
    );
    sim.command(
        0,
        Command::SendDevice {
            id: 1,
            device,
            text: text("For you."),
        },
    );
    sim.run_to(sim.now_us() + 1_000);
    assert_eq!(sim.count(0, "is another device now; not sent"), 1);
    assert_eq!(message(&sim, 0, "Not for you."), None);
    assert_eq!(carriage(&sim, 0, "For you."), Some(Carriage::Queued));
}
