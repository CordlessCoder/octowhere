//! A simulated node's radio, clock, random source, device, store and commands: the seams
//! `octowhere_node` runs on, over the shared [`World`].

use std::{
    cell::RefCell,
    future::{Future, poll_fn},
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};

use octowhere_node::{
    Command, Commands, Device, Fix, GpsTime, GroupStore, GroupWrite, Radio, Random, Received, Time,
    view::{MeshView, MessagesView},
};

use crate::{
    rng::SplitMix,
    world::{Micros, Mode, World},
};

/// How long a wait already over still takes: about a turn of a loop on the board. Time stands
/// still while a node runs, so without it a loop waiting on a deadline it has passed would never
/// see that deadline's effects, as the board's does a moment later.
const BUSY_US: Micros = 100;

/// Waits until virtual time `at`.
pub struct Sleep {
    world: Rc<World>,
    at: Micros,
    waiting: bool,
}

impl Sleep {
    pub fn new(world: &Rc<World>, at: Micros) -> Self {
        Self {
            world: world.clone(),
            at,
            waiting: false,
        }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<()> {
        let now = self.world.now();
        if !self.waiting {
            self.waiting = true;
            self.at = self.at.max(now + BUSY_US);
            self.world.wake_at(self.at, cx.waker());
            return Poll::Pending;
        }
        if now >= self.at {
            return Poll::Ready(());
        }
        Poll::Pending
    }
}

pub struct SimTime {
    pub world: Rc<World>,
    pub node: usize,
}

impl Time for SimTime {
    fn now(&self) -> i64 {
        self.world.nodes.borrow()[self.node].local(self.world.now())
    }

    async fn until(&self, at: i64) {
        let at = self.world.nodes.borrow()[self.node].global(at);
        Sleep::new(&self.world, at).await;
    }
}

pub struct SimRadio {
    pub world: Rc<World>,
    pub node: usize,
}

impl SimRadio {
    fn set_mode(&self, mode: Mode) {
        self.world.nodes.borrow_mut()[self.node].radio.mode = mode;
    }
}

impl Radio for SimRadio {
    async fn tune(&mut self, frequency: u32, sync_word: u8, _power: u8) -> bool {
        self.world.nodes.borrow_mut()[self.node].radio.channel = (frequency, sync_word);
        self.idle_receive().await;
        true
    }

    async fn idle_receive(&mut self) {
        let mut nodes = self.world.nodes.borrow_mut();
        let radio = &mut nodes[self.node].radio;
        radio.mode = Mode::Standby;
        radio.received = None;
    }

    async fn standby(&mut self) {
        self.set_mode(Mode::Standby);
    }

    async fn sleep(&mut self) {
        self.set_mode(Mode::Sleep);
    }

    async fn start_receiving(&mut self) -> bool {
        let now = self.world.now();
        let mut nodes = self.world.nodes.borrow_mut();
        let radio = &mut nodes[self.node].radio;
        if !matches!(radio.mode, Mode::Receiving(_)) {
            radio.mode = Mode::Receiving(now);
        }
        true
    }

    async fn wait_received(&mut self, deadline: i64) -> bool {
        let mut until = None;
        poll_fn(|cx| {
            let mut nodes = self.world.nodes.borrow_mut();
            let node = &mut nodes[self.node];
            if node.radio.received.is_some() {
                return Poll::Ready(true);
            }
            let now = self.world.now();
            let at = *until.get_or_insert_with(|| {
                let at = node.global(deadline).max(now + BUSY_US);
                self.world.wake_at(at, cx.waker());
                at
            });
            if now >= at {
                return Poll::Ready(false);
            }
            node.radio.waker = Some(cx.waker().clone());
            Poll::Pending
        })
        .await
    }

    async fn read_packet(&mut self) -> Option<(Received, i64)> {
        let mut nodes = self.world.nodes.borrow_mut();
        let node = &mut nodes[self.node];
        let (packet, done) = node.radio.received.take()?;
        Some((packet, node.local(done)))
    }

    fn seen_late_us(&self) -> i64 {
        0
    }

    async fn transmit(&mut self, packet: &[u8], at: Option<i64>) -> Option<bool> {
        if let Some(at) = at {
            let at = self.world.nodes.borrow()[self.node].global(at);
            Sleep::new(&self.world, at).await;
        }
        let end = self.world.send(self.node, packet);
        Sleep::new(&self.world, end).await;
        self.world.sent(self.node);
        self.idle_receive().await;
        Some(true)
    }
}

pub struct SimRandom(pub SplitMix);

impl Random for SimRandom {
    fn fill(&mut self, out: &mut [u8]) -> bool {
        self.0.fill(out);
        true
    }
}

pub struct SimDevice {
    pub world: Rc<World>,
    pub node: usize,
}

impl Device for SimDevice {
    fn fix(&self) -> Option<Fix> {
        self.world.nodes.borrow()[self.node].fix
    }

    fn gps_time(&self) -> Option<GpsTime> {
        let nodes = self.world.nodes.borrow();
        let node = &nodes[self.node];
        node.gps.then(|| {
            let now = self.world.now();
            let local = node.local(now);
            GpsTime {
                offset: local - (self.world.utc0 + now as i64),
                updated: local,
            }
        })
    }

    fn rtc_utc(&self, now: i64) -> Option<i64> {
        let nodes = self.world.nodes.borrow();
        let node = &nodes[self.node];
        node.rtc_error_us
            .map(|error| self.world.utc0 + node.global(now) as i64 + error)
    }

    fn publish(&self, view: &mut Box<MeshView>) {
        let mut nodes = self.world.nodes.borrow_mut();
        let node = &mut nodes[self.node];
        if node.view.as_deref() != Some(&**view) {
            node.view = Some(view.clone());
            node.views += 1;
        }
    }

    fn publish_messages(&self, messages: &MessagesView) -> bool {
        let mut nodes = self.world.nodes.borrow_mut();
        let node = &mut nodes[self.node];
        node.messages.copy_from(messages);
        node.message_views += 1;
        true
    }
}

pub struct SimStore {
    pub world: Rc<World>,
    pub node: usize,
    pub results: RefCell<Vec<bool>>,
}

impl GroupStore for SimStore {
    fn queue(&self, write: GroupWrite) -> Option<u32> {
        let mut nodes = self.world.nodes.borrow_mut();
        let node = &mut nodes[self.node];
        let stored = !node.fail_writes;
        if stored {
            node.stored.apply(&write);
        }
        let mut results = self.results.borrow_mut();
        results.push(stored);
        Some(results.len() as u32 - 1)
    }

    async fn send(&self, write: GroupWrite) -> u32 {
        self.queue(write).expect("the queue has room")
    }

    fn result(&self, number: u32) -> Option<bool> {
        self.results.borrow().get(number as usize).copied()
    }

    async fn saved(&self, number: u32) -> bool {
        self.result(number).unwrap_or(false)
    }
}

pub struct SimCommands {
    pub world: Rc<World>,
    pub node: usize,
}

impl Commands for SimCommands {
    async fn receive(&self) -> Command {
        poll_fn(|cx| {
            let mut nodes = self.world.nodes.borrow_mut();
            let node = &mut nodes[self.node];
            match node.commands.pop_front() {
                Some(command) => Poll::Ready(command),
                None => {
                    node.commands_waker = Some(cx.waker().clone());
                    Poll::Pending
                }
            }
        })
        .await
    }
}
