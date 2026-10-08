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
    Abandon,
    Read(u8, u32, usize),
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
        assert!(
            data.len() <= MAX_PARAMETERS,
            "{} parameter bytes",
            data.len()
        );
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
        let mut buffer = [0; STREAM_ROOM];
        let written = fill(&mut buffer);
        assert!(written <= STREAM_ROOM, "{written} bytes written");
        self.0
            .borrow_mut()
            .push(Event::Stream(buffer[..written].to_vec()));
        Ok(())
    }

    async fn end(&mut self) -> Result<(), Infallible> {
        self.0.borrow_mut().push(Event::End);
        Ok(())
    }

    fn abandon(&mut self) {
        self.0.borrow_mut().push(Event::Abandon);
    }

    /// Answers every read with [`REPLY`].
    async fn read(
        &mut self,
        instruction: u8,
        address: u32,
        buffer: &mut [u8],
    ) -> Result<(), Infallible> {
        self.0
            .borrow_mut()
            .push(Event::Read(instruction, address, buffer.len()));
        buffer.copy_from_slice(&REPLY[..buffer.len()]);
        Ok(())
    }
}

const REPLY: [u8; 2] = [0x01, 0xC2];

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

/// The reset and start-up the driver before this crate sent.
fn start_up() -> Vec<Event> {
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
}

#[test]
fn the_start_up_resets_then_sends_the_sequence_it_always_sent() {
    let (_, log) = started();
    assert_eq!(taken(&log), start_up());
}

#[test]
fn a_controller_reset_before_starts_without_its_reset_line() {
    let log = Log::default();
    block_on(Display::after_reset(
        Recorder(log.clone()),
        Pin(log.clone()),
        Te,
        Delay(log.clone()),
        PANEL,
    ))
    .unwrap();
    let sequence = start_up();
    let after_reset = sequence
        .iter()
        .position(|event| *event == command(0x11, &[]))
        .unwrap();
    assert_eq!(taken(&log), sequence[after_reset..]);
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
    let window = |x, y, w, h| {
        let Window {
            x,
            y,
            width,
            height,
        } = even_window(&PANEL, x, y, w, h);
        (x, y, width, height)
    };
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
fn a_window_set_is_the_one_returned() {
    let (mut display, log) = started();
    taken(&log);
    let window = block_on(display.set_window(3, 5, 4, 4)).unwrap();
    assert_eq!(
        window,
        Window {
            x: 2,
            y: 4,
            width: 6,
            height: 6
        }
    );
    assert_eq!(
        taken(&log),
        vec![command(0x2A, &[0, 8, 0, 13]), command(0x2B, &[0, 4, 0, 9])]
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

#[test]
fn pixels_dropped_unfinished_close_the_transaction() {
    let (mut display, log) = started();
    taken(&log);
    block_on(async {
        let mut pixels = display.pixels().await?;
        pixels.fill(|_| 0).await
    })
    .unwrap();
    assert_eq!(
        taken(&log),
        vec![
            Event::Begin(0x12, 0x2C00, Lanes::Quad),
            Event::Stream(vec![]),
            Event::Abandon,
        ]
    );
}

/// The commands `act` sends to a started controller.
fn sent(act: impl AsyncFnOnce(&mut Display) -> Result<(), Infallible>) -> Vec<Event> {
    let (mut display, log) = started();
    taken(&log);
    block_on(act(&mut display)).unwrap();
    taken(&log)
}

#[test]
fn the_te_line_turns_off_and_on_in_either_mode_at_any_line() {
    assert_eq!(
        sent(async |display| {
            display.te_off().await?;
            display.te_on(TeMode::VAndHBlank).await?;
            display.te_on(TeMode::VBlank).await?;
            display.set_te_line(300).await
        }),
        vec![
            command(0x34, &[]),
            command(0x35, &[0x01]),
            command(0x35, &[0x00]),
            command(0x44, &[0x01, 0x2C]),
        ]
    );
}

#[test]
fn the_scan_line_is_read_big_endian() {
    let (mut display, log) = started();
    taken(&log);
    assert_eq!(block_on(display.scan_line()), Ok(0x01C2));
    assert_eq!(taken(&log), vec![Event::Read(0x03, 0x4500, 2)]);
}

#[test]
fn the_partial_area_goes_out_offset_and_switches_modes() {
    assert_eq!(
        sent(async |display| {
            display.set_partial_area((10, 99), (20, 29)).await?;
            display.partial_mode().await?;
            display.normal_mode().await
        }),
        vec![
            command(0x31, &[0, 16, 0, 105]),
            command(0x30, &[0, 20, 0, 29]),
            command(0x12, &[]),
            command(0x13, &[]),
        ]
    );
}

#[test]
fn idle_mode_turns_on_and_off() {
    assert_eq!(
        sent(async |display| {
            display.set_idle(true).await?;
            display.set_idle(false).await
        }),
        vec![command(0x39, &[]), command(0x38, &[])]
    );
}

#[test]
fn deep_standby_is_left_by_a_reset_and_a_whole_start_up() {
    let (mut display, log) = started();
    taken(&log);
    block_on(display.deep_standby()).unwrap();
    assert_eq!(taken(&log), vec![command(0x4F, &[0x01])]);
    block_on(display.leave_deep_standby()).unwrap();
    assert_eq!(taken(&log), start_up());
}

#[test]
fn high_brightness_mode_turns_on_and_off_and_takes_its_own_level() {
    assert_eq!(
        sent(async |display| {
            display.set_hbm(true).await?;
            display.set_hbm_brightness(0x80).await?;
            display.set_hbm(false).await
        }),
        vec![
            command(0x66, &[0x02]),
            command(0x63, &[0x80]),
            command(0x66, &[0x00]),
        ]
    );
}

#[test]
fn sunlight_enhancement_sets_its_level_or_stops() {
    assert_eq!(
        sent(async |display| {
            display.set_sunlight(Some(Sunlight::Low)).await?;
            display.set_sunlight(Some(Sunlight::Medium)).await?;
            display.set_sunlight(Some(Sunlight::High)).await?;
            display.set_sunlight(None).await
        }),
        vec![
            command(0x58, &[0x04]),
            command(0x58, &[0x05]),
            command(0x58, &[0x06]),
            command(0x58, &[0x00]),
        ]
    );
}

#[test]
fn the_current_limit_turns_on_and_off() {
    assert_eq!(
        sent(async |display| {
            display.set_current_limit(true).await?;
            display.set_current_limit(false).await
        }),
        vec![command(0x55, &[0x03]), command(0x55, &[0x00])]
    );
}
