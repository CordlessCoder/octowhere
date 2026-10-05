use core::convert::Infallible;
use std::{cell::RefCell, rc::Rc, vec, vec::Vec};

use futures::executor::block_on;

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Event {
    Write(u8, u32, Lanes, Vec<u8>),
    Begin(u8, u32, Lanes),
    Stream(Vec<u8>),
    End,
    Reset(bool),
    DelayUs(u32),
    DelayMs(u32),
}

type Log = Rc<RefCell<Vec<Event>>>;

struct Recorder(Log);

impl Bus for Recorder {
    type Error = Infallible;

    async fn write(
        &mut self,
        instruction: u8,
        address: u32,
        lanes: Lanes,
        data: &[u8],
    ) -> Result<(), Infallible> {
        let event = Event::Write(instruction, address, lanes, data.to_vec());
        self.0.borrow_mut().push(event);
        Ok(())
    }

    async fn begin(
        &mut self,
        instruction: u8,
        address: u32,
        lanes: Lanes,
    ) -> Result<(), Infallible> {
        self.0
            .borrow_mut()
            .push(Event::Begin(instruction, address, lanes));
        Ok(())
    }

    async fn stream(&mut self, fill: impl FnOnce(&mut [u8]) -> usize) -> Result<(), Infallible> {
        let mut buffer = [0; 8];
        let written = fill(&mut buffer);
        self.0
            .borrow_mut()
            .push(Event::Stream(buffer[..written].to_vec()));
        Ok(())
    }

    async fn end(&mut self) -> Result<(), Infallible> {
        self.0.borrow_mut().push(Event::End);
        Ok(())
    }
}

struct Pin(Log);

impl embedded_hal::digital::ErrorType for Pin {
    type Error = Infallible;
}

impl OutputPin for Pin {
    fn set_low(&mut self) -> Result<(), Infallible> {
        self.0.borrow_mut().push(Event::Reset(false));
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), Infallible> {
        self.0.borrow_mut().push(Event::Reset(true));
        Ok(())
    }
}

struct Te;

impl embedded_hal::digital::ErrorType for Te {
    type Error = Infallible;
}

impl Wait for Te {
    async fn wait_for_high(&mut self) -> Result<(), Infallible> {
        Ok(())
    }

    async fn wait_for_low(&mut self) -> Result<(), Infallible> {
        Ok(())
    }

    async fn wait_for_rising_edge(&mut self) -> Result<(), Infallible> {
        Ok(())
    }

    async fn wait_for_falling_edge(&mut self) -> Result<(), Infallible> {
        Ok(())
    }

    async fn wait_for_any_edge(&mut self) -> Result<(), Infallible> {
        Ok(())
    }
}

struct Delay(Log);

impl DelayNs for Delay {
    async fn delay_ns(&mut self, ns: u32) {
        unreachable!("the driver waits in microseconds or milliseconds, not {ns} ns");
    }

    async fn delay_us(&mut self, us: u32) {
        self.0.borrow_mut().push(Event::DelayUs(us));
    }

    async fn delay_ms(&mut self, ms: u32) {
        self.0.borrow_mut().push(Event::DelayMs(ms));
    }
}

const PANEL: Config = Config {
    width: 466,
    height: 466,
    column_offset: 6,
    row_offset: 0,
    te_line: 150,
};

type Display = Co5300<Recorder, Pin, Te, Delay, Rgb565>;

fn started() -> (Display, Log) {
    let log = Log::default();
    let display = block_on(Co5300::new(
        Recorder(log.clone()),
        Pin(log.clone()),
        Te,
        Delay(log.clone()),
        PANEL,
    ))
    .unwrap();
    (display, log)
}

fn taken(log: &Log) -> Vec<Event> {
    core::mem::take(&mut *log.borrow_mut())
}

/// A command and its parameters, as the driver before this crate sent them.
fn command(command: u8, parameters: &[u8]) -> Event {
    Event::Write(
        0x02,
        u32::from(command) << 8,
        Lanes::Single,
        parameters.to_vec(),
    )
}

#[test]
fn the_start_up_resets_then_sends_the_sequence_it_always_sent() {
    let (_, log) = started();
    assert_eq!(
        taken(&log),
        vec![
            Event::Reset(false),
            Event::DelayMs(10),
            Event::Reset(true),
            Event::Reset(false),
            Event::DelayUs(10),
            Event::Reset(true),
            Event::DelayMs(120),
            command(0x11, &[]),
            Event::DelayMs(120),
            command(0xFE, &[0x00]),
            command(0xC4, &[0x80]),
            command(0x53, &[0x20]),
            command(0x63, &[0xFF]),
            command(0x51, &[0x00]),
            command(0x3A, &[0x55]),
            command(0x29, &[]),
            command(0x58, &[0x00]),
            command(0x36, &[0x00]),
            Event::DelayMs(10),
            command(0x20, &[]),
            command(0x35, &[0x00]),
            command(0x44, &[0x00, 150]),
        ]
    );
}

#[test]
fn each_colour_mode_sends_its_format() {
    fn format<C: ColorMode>() -> Vec<u8> {
        let log = Log::default();
        block_on(Co5300::<_, _, _, _, C>::new(
            Recorder(log.clone()),
            Pin(log.clone()),
            Te,
            Delay(log.clone()),
            PANEL,
        ))
        .unwrap();
        taken(&log)
            .into_iter()
            .find_map(|event| match event {
                Event::Write(0x02, 0x3A00, Lanes::Single, data) => Some(data),
                _ => None,
            })
            .unwrap()
    }
    assert_eq!(format::<Rgb565>(), [0x55]);
    assert_eq!(format::<Rgb888>(), [0x77]);
    assert_eq!(format::<Gray8>(), [0x11]);
}

#[test]
fn sleep_turns_the_panel_off_before_sleeping_and_on_after_waking() {
    let (mut display, log) = started();
    taken(&log);
    block_on(display.display_off()).unwrap();
    block_on(display.display_on()).unwrap();
    assert_eq!(
        taken(&log),
        vec![
            command(0x28, &[]),
            Event::DelayMs(20),
            command(0x10, &[]),
            Event::DelayMs(120),
            command(0x11, &[]),
            Event::DelayMs(120),
            command(0x29, &[]),
            Event::DelayMs(20),
        ]
    );
}

#[test]
fn the_brightness_is_one_command() {
    let (mut display, log) = started();
    taken(&log);
    block_on(display.set_brightness(0xA0)).unwrap();
    assert_eq!(taken(&log), vec![command(0x51, &[0xA0])]);
}

#[test]
fn a_window_goes_out_offset_with_inclusive_ends() {
    let (mut display, log) = started();
    taken(&log);
    block_on(display.set_window(10, 20, 100, 50)).unwrap();
    assert_eq!(
        taken(&log),
        vec![
            command(0x2A, &[0, 16, 0, 115]),
            command(0x2B, &[0, 20, 0, 69]),
        ]
    );
}

#[test]
fn windows_widen_to_the_grain_and_stay_on_the_panel() {
    let window = |x, y, w, h| even_window(&PANEL, x, y, w, h);
    assert_eq!(window(0, 0, 466, 466), (0, 0, 466, 466));
    assert_eq!(
        window(3, 5, 4, 4),
        (2, 4, 6, 6),
        "odd edges widen both ways"
    );
    assert_eq!(
        window(7, 7, 0, 0),
        (6, 6, 2, 2),
        "an empty window takes one cell"
    );
    assert_eq!(
        window(460, 460, 50, 50),
        (460, 460, 6, 6),
        "clipped at the far edge"
    );
    assert_eq!(
        window(500, 500, 10, 10),
        (464, 464, 2, 2),
        "a corner past the panel"
    );
}

#[test]
fn pixels_open_with_a_quad_ramwr_and_pass_each_fill_through() {
    let (mut display, log) = started();
    taken(&log);
    block_on(async {
        let mut pixels = display.pixels().await?;
        pixels
            .fill(|buffer| {
                buffer[..2].copy_from_slice(&[0xF8, 0x00]);
                2
            })
            .await?;
        pixels.finish().await
    })
    .unwrap();
    assert_eq!(
        taken(&log),
        vec![
            Event::Begin(0x12, 0x2C00, Lanes::Quad),
            Event::Stream(vec![0xF8, 0x00]),
            Event::End,
        ]
    );
}
