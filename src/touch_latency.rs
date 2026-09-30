//! Times each touch read from the controller's report to the flush that shows it.
//!
//! A pulse counter watches the controller's INT line beside the GPIO interrupt the driver waits
//! on, so every report's edge is timed, including those that arrive while no read waits for one.
//! Each read's times travel with it through `TOUCH_READS`. The frame loop adds its own and hands
//! them to core 1 in the frame's `Timings`, and core 1 logs one line once the frame is flushed.

use core::cell::{Cell, RefCell};

use defmt::info;
use embassy_sync::blocking_mutex::{Mutex as BlockingMutex, raw::CriticalSectionRawMutex};
use embassy_time::Instant;
use esp_hal::{
    gpio::interconnect::InputSignal,
    handler,
    interrupt::Priority,
    pcnt::{Pcnt, channel::EdgeMode},
    peripherals::PCNT,
};
use octowhere::peripherals::touch::TouchData;

type CS = CriticalSectionRawMutex;

#[derive(Clone, Copy, Default)]
struct Edges {
    falls: u32,
    last_fall: u64,
    /// `falls` when the last read began.
    read_falls: u32,
    /// How long INT was held low the last time it rose.
    last_low: u64,
}

static EDGES: BlockingMutex<CS, Cell<Edges>> = BlockingMutex::new(Cell::new(Edges {
    falls: 0,
    last_fall: 0,
    read_falls: 0,
    last_low: 0,
}));
static COUNTER: BlockingMutex<CS, RefCell<Option<Pcnt<'static>>>> =
    BlockingMutex::new(RefCell::new(None));
/// Set as the frame loop takes a read, for the old handoff's wait for room.
#[cfg(feature = "touch-latency-fifo")]
pub static TAKEN: embassy_sync::signal::Signal<CS, ()> = embassy_sync::signal::Signal::new();
static SEQUENCE: BlockingMutex<CS, Cell<u32>> = BlockingMutex::new(Cell::new(0));
/// The edge of the lift report the frame loop took last, while a contact was held.
static LIFT_REPORTED: BlockingMutex<CS, Cell<Option<u64>>> = BlockingMutex::new(Cell::new(None));

/// Counts INT's falling edges on unit 0 and its rising edges on unit 1, one interrupt each.
pub fn watch(pcnt: PCNT<'static>, fall: InputSignal<'static>, rise: InputSignal<'static>) {
    let mut pcnt = Pcnt::new(pcnt);
    pcnt.set_interrupt_handler(on_edge);
    // A microsecond, in APB cycles.
    const FILTER: u16 = 80;
    pcnt.unit0.set_high_limit(Some(1)).unwrap();
    pcnt.unit0.set_filter(Some(FILTER)).unwrap();
    pcnt.unit0.channel0.set_edge_signal(fall);
    pcnt.unit0
        .channel0
        .set_input_mode(EdgeMode::Increment, EdgeMode::Hold);
    pcnt.unit1.set_high_limit(Some(1)).unwrap();
    pcnt.unit1.set_filter(Some(FILTER)).unwrap();
    pcnt.unit1.channel0.set_edge_signal(rise);
    pcnt.unit1
        .channel0
        .set_input_mode(EdgeMode::Hold, EdgeMode::Increment);
    pcnt.unit0.clear();
    pcnt.unit1.clear();
    pcnt.unit0.listen();
    pcnt.unit1.listen();
    pcnt.unit0.resume();
    pcnt.unit1.resume();
    COUNTER.lock(|counter| counter.replace(Some(pcnt)));
}

#[handler(priority = Priority::Priority3)]
fn on_edge() {
    let now = Instant::now().as_micros();
    COUNTER.lock(|counter| {
        let counter = counter.borrow();
        let Some(pcnt) = counter.as_ref() else {
            return;
        };
        let fell = pcnt.unit0.interrupt_is_set();
        let rose = pcnt.unit1.interrupt_is_set();
        if fell {
            pcnt.unit0.reset_interrupt();
        }
        if rose {
            pcnt.unit1.reset_interrupt();
        }
        EDGES.lock(|edges| {
            let mut state = edges.get();
            if fell {
                state.falls += 1;
                state.last_fall = now;
            }
            if rose && state.falls > 0 {
                state.last_low = now - state.last_fall;
            }
            edges.set(state);
        });
    });
}

#[derive(Clone, Copy, Debug)]
pub struct ReadTrace {
    /// Counts reads, so that a gap names those the frame loop never took.
    sequence: u32,
    /// The latest report's edge, if one came since the last read began.
    edge: Option<u64>,
    /// How long before waking INT last fell, whether or not a read took that report already.
    since_fall: u64,
    /// Reports since the last read began. Past one, reads missed some.
    reports: u32,
    last_low: u64,
    woken: u64,
    by_poll: bool,
    read: u64,
}

/// Called as `touch_task` wakes to read.
pub fn begin_read(by_poll: bool) -> ReadTrace {
    let woken = Instant::now().as_micros();
    EDGES.lock(|edges| {
        let mut state = edges.get();
        let reports = state.falls - state.read_falls;
        state.read_falls = state.falls;
        edges.set(state);
        let sequence = SEQUENCE.lock(|sequence| {
            sequence.set(sequence.get() + 1);
            sequence.get()
        });
        ReadTrace {
            sequence,
            edge: (reports > 0).then_some(state.last_fall),
            since_fall: woken - state.last_fall,
            reports,
            last_low: state.last_low,
            woken,
            by_poll,
            read: 0,
        }
    })
}

/// Called once the read is done, before it is queued for the frame loop.
pub fn end_read(mut trace: ReadTrace) -> ReadTrace {
    trace.read = Instant::now().as_micros();
    trace
}

/// Logs a read as it is made, before the frame loop can drop it, with the bytes of any report it
/// found.
#[cfg(feature = "touch-read-log")]
pub fn log_read(trace: &ReadTrace, data: &Result<TouchData, ()>, raw: &[u8]) {
    let (x, y) = first_point(data);
    let raw = if matches!(data, Ok(TouchData::Stale) | Err(())) {
        &[][..]
    } else {
        raw
    };
    info!(
        "[TOUCH-READ] seq={=u32} src={=str} kind={=str} at={=u64} edge={=i64} reports={=u32} \
         x={=i32} y={=i32} raw={=[u8]:02x}",
        trace.sequence,
        if trace.by_poll { "poll" } else { "int" },
        kind(data),
        trace.read,
        trace.edge.map_or(-1, |edge| edge as i64),
        trace.reports,
        x,
        y,
        raw,
    );
}

fn kind(data: &Result<TouchData, ()>) -> &'static str {
    match data {
        Ok(TouchData::Stale) => "stale",
        Ok(TouchData::Points(points)) if points.is_empty() => "lift",
        Ok(TouchData::Lifted(_)) => "lift",
        Ok(TouchData::Gesture(_)) => "gesture",
        Ok(TouchData::Points(_)) => "contact",
        Ok(TouchData::CoverGesture) => "cover",
        Err(()) => "failed",
    }
}

/// The first point of a contact, or -1.
fn first_point(data: &Result<TouchData, ()>) -> (i32, i32) {
    match data {
        Ok(TouchData::Points(points)) => points
            .first()
            .map_or((-1, -1), |point| (i32::from(point.x), i32::from(point.y))),
        _ => (-1, -1),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FrameTrace {
    read: ReadTrace,
    kind: &'static str,
    /// The first point of a contact, or -1.
    x: i32,
    y: i32,
    /// No contact was held before this read.
    first: bool,
    /// The frame loop was waiting when the read was done, rather than stepping or drawing.
    idle: bool,
    compass: bool,
    received: u64,
    stepped: u64,
    drawn: u64,
}

/// Called as the frame loop takes a read. `waiting_since` is when it began waiting.
pub fn received(
    read: ReadTrace,
    data: &Result<TouchData, ()>,
    in_contact: bool,
    compass: bool,
    waiting_since: Instant,
) -> FrameTrace {
    let received = Instant::now().as_micros();
    let kind = kind(data);
    let (x, y) = first_point(data);
    if kind == "lift" && in_contact {
        let edge = read.edge.unwrap_or(read.woken);
        LIFT_REPORTED.lock(|reported| {
            if reported.get().is_none() {
                reported.set(Some(edge));
            }
        });
    }
    FrameTrace {
        read,
        kind,
        x,
        y,
        first: !in_contact,
        idle: read.read >= waiting_since.as_micros(),
        compass,
        received,
        stepped: 0,
        drawn: 0,
    }
}

impl FrameTrace {
    pub fn stepped(&mut self) {
        self.stepped = Instant::now().as_micros();
    }

    pub fn drawn(&mut self) {
        self.drawn = Instant::now().as_micros();
    }

    /// Logs the trace once core 1 has flushed its frame. `taken` is when core 1 took the frame,
    /// `te` when its wait for the scan ended, and `damage` what it flushed.
    pub fn log(&self, taken: Instant, te: Instant, damage: &'static str) {
        let flushed = Instant::now().as_micros();
        let (taken, te) = (taken.as_micros(), te.as_micros());
        let read = &self.read;
        let origin = read.edge.unwrap_or(read.woken);
        info!(
            "[TOUCH-LATENCY] seq={=u32} src={=str} kind={=str} first={=bool} idle={=bool} \
             compass={=bool} x={=i32} y={=i32} at={=u64} reports={=u32} low={=u64} since_fall={=u64} damage={=str} edge_wake={=i64} wake_read={=u64} read_recv={=u64} \
             recv_step={=u64} step_draw={=u64} draw_take={=u64} take_te={=u64} te_flush={=u64} \
             total={=u64}",
            read.sequence,
            if read.by_poll { "poll" } else { "int" },
            self.kind,
            self.first,
            self.idle,
            self.compass,
            self.x,
            self.y,
            origin,
            read.reports,
            read.last_low,
            read.since_fall,
            damage,
            read.edge.map_or(-1, |edge| (read.woken - edge) as i64),
            read.read - read.woken,
            self.received - read.read,
            self.stepped - self.received,
            self.drawn - self.stepped,
            taken - self.drawn,
            te - taken,
            flushed - te,
            flushed - origin,
        );
    }
}

/// Records a synthetic report's edge, in place of the pulse counter.
#[cfg(feature = "touch-latency-synthetic")]
pub fn synthetic_edge(at: Instant) {
    EDGES.lock(|edges| {
        let mut state = edges.get();
        state.falls += 1;
        state.last_fall = at.as_micros();
        edges.set(state);
    });
}

#[cfg(feature = "touch-latency-synthetic")]
pub use synthetic::Finger;

/// A finger that strokes the panel on a fixed script, for timing the chain without a hand on it.
#[cfg(feature = "touch-latency-synthetic")]
mod synthetic {
    use embassy_time::{Duration, Instant};
    use octowhere::peripherals::touch::{TouchData, TouchPoint};

    /// INT's period while a finger is down.
    const PULSE: Duration = Duration::from_millis(10);
    /// A finger held still is reported once in this many pulses; INT still falls on the others.
    const STILL_EVERY: u32 = 10;
    const REST: Duration = Duration::from_millis(1_500);

    struct Stroke {
        from: (i32, i32),
        to: (i32, i32),
        /// Pulses on the way from `from` to `to`.
        travel: u32,
        /// Pulses held still at `to` before the lift.
        hold: u32,
    }

    /// Clock to compass, a drag on the compass that springs back, compass to clock, and a drag
    /// on the clock that springs back. Each is followed by `REST`.
    const SCRIPT: [Stroke; 4] = [
        Stroke {
            from: (420, 233),
            to: (60, 233),
            travel: 30,
            hold: 0,
        },
        Stroke {
            from: (233, 233),
            to: (313, 233),
            travel: 80,
            hold: 30,
        },
        Stroke {
            from: (60, 233),
            to: (420, 233),
            travel: 30,
            hold: 0,
        },
        Stroke {
            from: (233, 233),
            to: (153, 233),
            travel: 80,
            hold: 30,
        },
    ];

    pub struct Finger {
        stroke: usize,
        /// The next pulse, counted from `start`.
        step: u32,
        start: Instant,
        /// The report the controller holds until a read takes it.
        pending: Option<TouchData>,
        seed: u32,
    }

    impl Finger {
        pub fn new(start: Instant) -> Self {
            Self {
                stroke: 0,
                step: 0,
                start,
                pending: None,
                seed: 0x9e37_79b9,
            }
        }

        /// When INT next falls.
        pub fn next_pulse(&self) -> Instant {
            self.start + PULSE * self.step
        }

        /// INT falls: the controller writes this pulse's report, if it has one.
        pub fn pulse(&mut self) {
            let stroke = &SCRIPT[self.stroke];
            let step = self.step;
            let report = if step <= stroke.travel {
                let (x, y) = (
                    stroke.from.0
                        + (stroke.to.0 - stroke.from.0) * step as i32 / stroke.travel as i32,
                    stroke.from.1
                        + (stroke.to.1 - stroke.from.1) * step as i32 / stroke.travel as i32,
                );
                Some(contact(x, y))
            } else if step <= stroke.travel + stroke.hold {
                (step - stroke.travel)
                    .is_multiple_of(STILL_EVERY)
                    .then(|| contact(stroke.to.0, stroke.to.1))
            } else {
                Some(TouchData::Points(heapless::Vec::new()))
            };
            if report.is_some() {
                self.pending = report;
            }
            if step > stroke.travel + stroke.hold {
                // Varies where the next stroke lands against the frame loop and the scan.
                let jitter = Duration::from_micros(u64::from(self.random() % 10_000));
                self.start = self.next_pulse() + REST + jitter;
                self.stroke = (self.stroke + 1) % SCRIPT.len();
                self.step = 0;
            } else {
                self.step += 1;
            }
        }

        /// A read: the report the controller holds, or stale if the last read took it.
        pub fn read(&mut self) -> TouchData {
            self.pending.take().unwrap_or(TouchData::Stale)
        }

        fn random(&mut self) -> u32 {
            self.seed ^= self.seed << 13;
            self.seed ^= self.seed >> 17;
            self.seed ^= self.seed << 5;
            self.seed
        }
    }

    fn contact(x: i32, y: i32) -> TouchData {
        let mut points = heapless::Vec::new();
        _ = points.push(TouchPoint {
            x: x as u16,
            y: y as u16,
        });
        TouchData::Points(points)
    }
}

/// Called after each step. Logs how long the stage took to count a contact lifted, from the
/// edge of its first lift report, or that it lifted without one.
pub fn contact_step(was_in_contact: bool, in_contact: bool) {
    if in_contact {
        return;
    }
    let reported = LIFT_REPORTED.lock(|reported| reported.take());
    if !was_in_contact {
        return;
    }
    let now = Instant::now().as_micros();
    match reported {
        Some(edge) => info!("[TOUCH-LIFT] report_to_lift={=u64}", now - edge),
        None => info!("[TOUCH-LIFT] without_report at={=u64}", now),
    }
}
