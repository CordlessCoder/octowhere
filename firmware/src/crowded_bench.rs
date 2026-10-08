//! Times the stage's step and draw and the display core's flush on the crowded screens two
//! boards cannot make: the group and messages of [`fixture`] stand in for the mesh's, and a
//! debugger can turn the heading and name the phase of the run. `tools/crowded-screens-bench.py`
//! drives it and `tools/crowded-screens-summary.py` reads its log.

mod fixture;

use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};

use defmt::info;
use embassy_time::{Duration, Instant};
use octowhere::ui::{
    drawer::Root,
    group::view::{MeshView, MessagesView, Request},
    screens::Screen,
    stage::{Motion, Stage},
};

/// The phase of the run, which each frame is logged with.
#[unsafe(no_mangle)]
static OCTOWHERE_BENCH_PHASE: AtomicU32 = AtomicU32::new(0);
/// Degrees a second the heading turns clockwise in place of the compass's; 0 gives it back.
#[unsafe(no_mangle)]
static OCTOWHERE_BENCH_TURN: AtomicI32 = AtomicI32::new(0);

/// A frame's numbers, kept with its framebuffer until the display core has flushed it.
#[derive(Debug, Default)]
pub struct Frame {
    /// Counts from 1; 0 holds no frame.
    number: u32,
    phase: u32,
    step: u32,
    draw: u32,
    /// The pixels drawn.
    repaint: u32,
    /// The internal heap in use after the draw, and the most of it in use after the step or the
    /// draw.
    heap: u32,
    peak: u32,
    /// What the display core sent of it.
    pub regions: u32,
    pub pixels: u32,
}

pub struct Bench {
    frames: u32,
    phase: u32,
    ready: bool,
    /// The members are placed anew, their ages counted back from now: at the start and as each
    /// phase starts.
    place: bool,
    placed_at: i64,
    /// This device's id in the group the members were last placed in.
    own: Option<u8>,
    /// The messages need adding again without a copy of the mesh's to prompt it: at the start,
    /// and when this device's id changes.
    mail: bool,
    /// When the last member's message was sent, from when they were first added.
    newest: Option<i64>,
    /// The members whose messages the screens have read, by bit.
    read: u32,
    /// Since when the heading has turned, from what and at what rate.
    turn: Option<(Instant, u16, i32)>,
    heading: Option<u16>,
    /// What the bench spent in the step under way, which is left out of it.
    spent: Duration,
    /// How long the swap after the last frame's draw waited for the display core.
    wait: Duration,
}

fn micros(duration: Duration) -> u32 {
    duration.as_micros() as u32
}

impl Bench {
    pub const fn new() -> Self {
        Self {
            frames: 0,
            phase: 0,
            ready: false,
            place: true,
            placed_at: 0,
            own: None,
            mail: true,
            newest: None,
            read: 0,
            turn: None,
            heading: None,
            spent: Duration::MIN,
            wait: Duration::MIN,
        }
    }

    /// Logs the start-up ending, and a new phase with where the screens are as it starts.
    // Out of line, as are the other calls here that seldom do anything: inlined, their locals
    // grow the frame loop's own frame, which is on core 0's deepest path.
    #[inline(never)]
    pub fn phase(&mut self, stage: &Stage) {
        if !self.ready && !stage.starting_up() {
            self.ready = true;
            info!("[BENCH] ready");
        }
        let phase = OCTOWHERE_BENCH_PHASE.load(Ordering::Relaxed);
        if phase == self.phase {
            return;
        }
        self.phase = phase;
        self.place = true;
        let screen = match stage.screen() {
            Screen::Clock => "clock",
            Screen::Compass => "compass",
            Screen::Members => "members",
        };
        let drawer =
            stage
                .drawer()
                .map_or("closed", |drawer| match (drawer.root(), drawer.child()) {
                    (_, Some(_)) => "child",
                    (Root::Events, None) => "events",
                    (Root::Messages, None) => "messages",
                });
        let heading = stage
            .members_text()
            .find(|text| text.ends_with("FORWARD") || text.starts_with("NORTH UP"))
            .unwrap_or("-");
        let conversations = stage.messages().map_or(0, |messages| {
            fixture::conversations(messages, self.own.unwrap_or(0))
        });
        info!(
            "[BENCH] phase {=u32} screen={=str} own={} member={} drawer={=str} events={=usize} unread={=usize} conversations={=usize} heading={=str}",
            phase,
            screen,
            self.own,
            stage.member(),
            drawer,
            stage.events().len(),
            stage.events().unread(),
            conversations,
            heading,
        );
    }

    /// While the heading turns, steps at least as often as the motion task samples on the
    /// compass faces, whether or not the board has one.
    pub fn pace(&self, wait: Duration) -> Duration {
        if OCTOWHERE_BENCH_TURN.load(Ordering::Relaxed) == 0 {
            wait
        } else {
            wait.min(super::COMPASS_PERIOD)
        }
    }

    /// Places the members in `view`, which the mesh's view was copied into if `copied`.
    #[inline(never)]
    pub fn crowd(&mut self, view: &mut MeshView, copied: bool) {
        if !copied && !self.place {
            return;
        }
        let start = Instant::now();
        if core::mem::take(&mut self.place) {
            self.placed_at = start.as_micros() as i64;
        }
        fixture::place(view, self.placed_at);
        let own = view.group.as_ref().map(|group| group.own);
        if own != self.own {
            self.own = own;
            self.mail = true;
        }
        self.spent += start.elapsed();
    }

    /// Adds the members' messages to `messages`, which the mesh's were copied into if `copied`.
    /// Returns whether `messages` changed.
    #[inline(never)]
    pub fn mail(&mut self, messages: &mut MessagesView, copied: bool) -> bool {
        let Some(own) = self.own.filter(|_| copied || self.mail) else {
            return copied;
        };
        let start = Instant::now();
        let newest = *self.newest.get_or_insert(start.as_micros() as i64);
        fixture::add_messages(messages, own, self.read, newest);
        self.mail = false;
        self.spent += start.elapsed();
        true
    }

    /// Keeps a member's message read once the screens have read it, since the mesh never had it
    /// to mark.
    pub fn read(&mut self, request: Option<Request>) {
        if let Some(Request::Read(id)) = request
            && let Some(index) = fixture::sender(id)
        {
            self.read |= 1 << index;
        }
    }

    /// The motion the stage is to see: the compass's, or a turning heading while a debugger asks.
    pub fn motion(&mut self, compass: Option<Motion>) -> Option<Motion> {
        let rate = OCTOWHERE_BENCH_TURN.load(Ordering::Relaxed);
        if rate == 0 {
            self.turn = None;
            if let Some(motion) = compass {
                self.heading = motion.compass.heading_decidegrees.or(self.heading);
            }
            return compass;
        }
        let now = Instant::now();
        let (since, from) = match self.turn {
            Some((since, from, turning)) if turning == rate => (since, from),
            _ => {
                let from = self.heading.unwrap_or(0);
                self.turn = Some((now, from, rate));
                (now, from)
            }
        };
        let compass = fixture::turned(from, rate, (now - since).as_micros());
        self.heading = compass.heading_decidegrees;
        Some(Motion { compass })
    }

    /// Notes the frame just drawn in `frame`, its step less what the bench spent in it.
    pub fn record(
        &mut self,
        frame: &mut Frame,
        step: Duration,
        draw: Duration,
        repaint: u32,
        heap: usize,
        peak: usize,
    ) {
        self.frames += 1;
        let step = step
            .checked_sub(core::mem::take(&mut self.spent))
            .unwrap_or_default();
        *frame = Frame {
            number: self.frames,
            phase: self.phase,
            step: micros(step),
            draw: micros(draw),
            repaint,
            heap: heap as u32,
            peak: peak as u32,
            regions: 0,
            pixels: 0,
        };
    }

    /// Logs the frame `frame` holds, now the display core has flushed it, and forgets it. `wait`
    /// is how long the swap that just finished waited for the display core.
    #[inline(never)]
    pub fn log(
        &mut self,
        frame: &mut Frame,
        total: Duration,
        vsync: Duration,
        flush: Duration,
        wait: Duration,
    ) {
        let done = core::mem::take(frame);
        let waited = core::mem::replace(&mut self.wait, wait);
        if done.number == 0 {
            return;
        }
        info!(
            "[BENCH] frame n={=u32} phase={=u32} step={=u32}us draw={=u32}us repaint={=u32} flush={=u32}us vsync={=u32}us regions={=u32} pixels={=u32} frame={=u32}us swap={=u32}us heap={=u32} peak={=u32}",
            done.number,
            done.phase,
            done.step,
            done.draw,
            done.repaint,
            micros(flush),
            micros(vsync),
            done.regions,
            done.pixels,
            micros(total),
            micros(waited),
            done.heap,
            done.peak,
        );
    }
}
