#![feature(impl_trait_in_assoc_type)]
#![feature(type_alias_impl_trait)]
#![feature(allocator_api)]
#![no_std]
#![no_main]
// Now defaults to deny in Rust-2024, however abusing statics is necessary in the embedded world.
#![expect(static_mut_refs)]
#![expect(unused)]
#![deny(clippy::mem_forget)]
#![warn(unused_must_use)]
#[cfg(all(feature = "lora-link-tx", feature = "lora-link-rx"))]
compile_error!("lora-link-tx and lora-link-rx are mutually exclusive");
extern crate alloc;

use alloc::{alloc::Allocator, boxed::Box};
use core::{
    cell::{Cell, RefCell},
    future::Future,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};
use defmt::{debug, error, info, warn};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_futures::{
    join::join,
    select::{Either, Either3, Either4, select, select3, select4},
};
use embassy_sync::{
    blocking_mutex::{Mutex as BlockingMutex, raw::CriticalSectionRawMutex},
    channel::Channel,
    mutex::Mutex,
    signal::Signal,
};
use embassy_time::{Duration, Instant, TimeoutError, Timer, with_timeout};
use embedded_graphics::prelude::*;
use embedded_hal_async::i2c::I2c as _;
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::{
    dma_tx_buffer,
    gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull},
    i2c::master::I2c,
    peripherals, spi,
    time::Rate,
    timer::timg::TimerGroup,
};
use esp_println as _;
use lc76g::{
    GnssDateTime, GnssError, GnssOperation, GnssState, Lc76g, LowPowerMode, NmeaOutputRate,
    NmeaParser, NmeaSentence, NmeaUpdate, PairCommandBuilder,
};
use octowhere::{
    board,
    chrome::{self, Color, Dirty, FB},
    drivers::{co5300::Co5300Display, framebuffer::Flush as _, qspi_bus::QspiBus},
    fontdue,
    framebuffer::Framebuffer,
    gnss_time::SecondEstimator,
    motion::{
        compass::{AxisMap, Calibration, CalibrationEvent, CompassView, Holds, Vec3},
        fusion::Fusion,
        imu::{accel_micro_ms2, gyro_micro_rad_s},
    },
    peripherals::{
        magnetometer::{Bmm350, MagnetometerError},
        power::{Axp2101Error, Axp2101Power, PowerKey},
        rtc::{DateTime as RtcDateTime, Pcf85063aRtc, RtcError},
        touch::{Cst9217, Cst9217Config, Cst9217Error, TouchData},
    },
    settings::{self, Store},
    tz::{self, DATABASE},
    ui::{
        clock::{ClockState, ZoneId, ZoneMode, ZoneState},
        screens::{Battery, DEFAULT_BRIGHTNESS, Gnss, PeripheralState},
        second::choice_label,
        stage::{
            Input as StageInput, Key as StageKey, Motion, Sensors, Stage, Store as Choice, Touch,
        },
        startup::{Outcome, Part, Report},
    },
    util::{Swap, SwapThread},
};
use static_cell::StaticCell;
use sx127xlora::{
    Sx1272,
    driver::Sx1272Lora,
    registers::{FRF_MSB, IRQ_FLAGS, OP_MODE, VERSION},
    types::{RxDone, Sx127xLoraConfig, TxDone},
};
use tca9554::Tca9554;

use esp_alloc as _;
use esp_backtrace as _;
esp_bootloader_esp_idf::esp_app_desc!();

struct SecondCore<A: Allocator + 'static = alloc::alloc::Global> {
    gpio4: peripherals::GPIO4<'static>,
    gpio5: peripherals::GPIO5<'static>,
    gpio6: peripherals::GPIO6<'static>,
    gpio7: peripherals::GPIO7<'static>,
    gpio12: peripherals::GPIO12<'static>,
    gpio13: peripherals::GPIO13<'static>,
    gpio38: peripherals::GPIO38<'static>,
    gpio39: peripherals::GPIO39<'static>,
    dma_ch0: peripherals::DMA_CH2<'static>,
    spi2: peripherals::SPI2<'static>,
    swap: SwapThread<'static, SwapState<A>>,
}

static mut CORE1_STACK: esp_hal::system::Stack<8192> = esp_hal::system::Stack::new();
static CORE1_EXECUTOR: StaticCell<esp_rtos::embassy::Executor> = StaticCell::new();
/// Runs every task that uses the I2C bus on core 0 from a software interrupt, so the GNSS and
/// radio timing preempts the frame loop instead of waiting for it to yield. Once `bring_up` has
/// spawned them, nothing in thread mode may take the bus: two tasks here waiting on it wake each
/// other in turn and never let a thread-mode holder run to release it.
static BUS_EXECUTOR: StaticCell<esp_rtos::embassy::InterruptExecutor<2>> = StaticCell::new();
static SWAP: StaticCell<Swap<SwapState<&esp_alloc::EspHeap>>> = StaticCell::new();
type I2cBus = I2c<'static, esp_hal::Async>;
type SharedI2cDevice = I2cDevice<'static, CriticalSectionRawMutex, I2cBus>;
type SensorImu = ph_qmi8658::Qmi8658I2c<SharedI2cDevice, core::convert::Infallible, Input<'static>>;
type LoraSpi = ExclusiveDevice<
    spi::master::Spi<'static, esp_hal::Async>,
    Output<'static>,
    embassy_time::Delay,
>;
type SensorLora = Sx1272Lora<LoraSpi>;

struct LoraPath {
    i2c: SharedI2cDevice,
    output: u8,
}

impl LoraPath {
    const OUTPUT_REGISTER: u8 = 0x01;

    fn new(i2c: SharedI2cDevice, output: u8) -> Self {
        Self { i2c, output }
    }

    async fn write_output(&mut self, output: u8) -> Result<(), ()> {
        self.i2c
            .write(board::TCA9554_I2C_ADDR, &[Self::OUTPUT_REGISTER, output])
            .await
            .map_err(|_| ())?;
        self.output = output;
        Ok(())
    }

    async fn receive(&mut self) -> Result<(), ()> {
        let rx = 1 << board::EXIO_LORA_RX_SWITCH;
        let tx = 1 << board::EXIO_LORA_TX_SWITCH;
        self.write_output((self.output | rx) & !tx).await
    }

    async fn transmit(&mut self) -> Result<(), ()> {
        let rx = 1 << board::EXIO_LORA_RX_SWITCH;
        let tx = 1 << board::EXIO_LORA_TX_SWITCH;
        self.write_output((self.output | tx) & !rx).await
    }
}

static I2C_BUS: StaticCell<Mutex<CriticalSectionRawMutex, I2cBus>> = StaticCell::new();
static SENSOR_STATE: Signal<CriticalSectionRawMutex, SensorSnapshot> = Signal::new();

/// A time a debugger writes for `sensor_task` to set the RTC to; `tools/rtc-inject.py` does.
#[cfg(feature = "rtc-inject")]
mod rtc_inject {
    use core::sync::atomic::{AtomicU32, Ordering};

    /// UTC seconds since 1970.
    #[unsafe(no_mangle)]
    static OCTOWHERE_RTC_INJECT_TIME: AtomicU32 = AtomicU32::new(0);
    /// 0 when none is pending, [`AS_RTC`] or [`AS_GNSS`]. The debugger writes it after the time.
    #[unsafe(no_mangle)]
    static OCTOWHERE_RTC_INJECT_MODE: AtomicU32 = AtomicU32::new(0);

    /// As though the RTC had kept the time: the clock shows it unconfirmed.
    pub const AS_RTC: u32 = 1;
    /// As though GNSS had just set it.
    pub const AS_GNSS: u32 = 2;

    /// Takes a pending time and its mode.
    pub fn take() -> Option<(u32, u32)> {
        let mode = OCTOWHERE_RTC_INJECT_MODE.swap(0, Ordering::Acquire);
        (mode != 0).then(|| (OCTOWHERE_RTC_INJECT_TIME.load(Ordering::Relaxed), mode))
    }
}
static MOTION_STATE: Signal<CriticalSectionRawMutex, Motion> = Signal::new();
/// Set by the frame loop while the compass screen shows; `motion_task` then samples fast.
static COMPASS_ACTIVE: AtomicBool = AtomicBool::new(false);
static COMPASS_RECALIBRATE: AtomicBool = AtomicBool::new(false);
/// Settings for `settings_task` to save. A full queue drops the newest, which the next change of
/// the same setting supersedes.
static SETTINGS_WRITES: Channel<CriticalSectionRawMutex, settings::Write, 4> = Channel::new();
/// How many writes went into `SETTINGS_WRITES`, and how many `settings_task` has finished,
/// saved or not. Equal, nothing queued is left to save.
static SETTINGS_QUEUED: AtomicU32 = AtomicU32::new(0);
static SETTINGS_DONE: AtomicU32 = AtomicU32::new(0);
/// Power key presses, from `sensor_task`, which owns the PMIC, for the frame loop.
static KEY_PRESSES: Channel<CriticalSectionRawMutex, PowerKey, 2> = Channel::new();
/// BOOT key presses, from `boot_key_task`, for the frame loop.
static BOOT_KEY_PRESSES: Channel<CriticalSectionRawMutex, StageKey, 2> = Channel::new();
/// A BOOT key held this long is a long press, as the power key's is.
const BOOT_KEY_LONG: Duration = Duration::from_secs(1);
/// How long the BOOT key's level must hold before a press or a release counts.
const BOOT_KEY_SETTLE: Duration = Duration::from_millis(20);
/// Set by the frame loop once the panel is off after the power-off confirmation, for
/// `gnss_task` to park the GNSS module.
static POWER_OFF: Signal<CriticalSectionRawMutex, ()> = Signal::new();
/// Set by `gnss_task` once the GNSS module is parked, for `sensor_task` to power the board off.
static GNSS_PARKED: Signal<CriticalSectionRawMutex, ()> = Signal::new();
/// The receiver's state after each burst of NMEA, from `gnss_task` for `sensor_task`.
static GNSS_STATE: Signal<CriticalSectionRawMutex, GnssState> = Signal::new();
/// The local timer minus UTC, from `gnss_task`: see [`gps_utc`].
static GPS_TIME: BlockingMutex<CriticalSectionRawMutex, Cell<Option<GpsTime>>> =
    BlockingMutex::new(Cell::new(None));

#[derive(Clone, Copy)]
struct GpsTime {
    /// The local timer minus UTC, in microseconds.
    offset: i64,
    /// When a fix last refined it. The local timer drifts from UTC by its crystal's error after.
    updated: Instant,
}

/// UTC in microseconds now, and when the fixes it comes from last refined it. It is late by the
/// receiver's own latency, which is the same on every board.
fn gps_utc() -> Option<(i64, Instant)> {
    let time = GPS_TIME.lock(Cell::get)?;
    Some((
        Instant::now().as_micros() as i64 - time.offset,
        time.updated,
    ))
}
/// How long powering off waits for the settings queued before it to be saved.
const SETTINGS_SETTLE: Duration = Duration::from_secs(3);
/// A zone choice from the settings panel, for `zone_task`, which owns the zone.
static ZONE_CHOICE: Signal<CriticalSectionRawMutex, ZoneChoice> = Signal::new();
/// The latest fix's latitude and longitude, from `sensor_task` for `zone_task`.
static ZONE_FIX: Signal<CriticalSectionRawMutex, (i32, i32)> = Signal::new();
/// The zone the clocks show, from `zone_task` for `sensor_task`.
static ZONE_STATE: BlockingMutex<CriticalSectionRawMutex, Cell<ZoneState>> =
    BlockingMutex::new(Cell::new(ZoneState {
        mode: ZoneMode::Automatic,
        zone: None,
    }));
/// Asks `touch_task` to read the controller without waiting for its report.
static TOUCH_POLL: Signal<CriticalSectionRawMutex, ()> = Signal::new();
/// Reads of the touch controller the frame loop has not taken, oldest first: see
/// [`put_touch_read`].
static TOUCH_READS: BlockingMutex<CriticalSectionRawMutex, RefCell<heapless::Deque<TouchRead, 2>>> =
    BlockingMutex::new(RefCell::new(heapless::Deque::new()));
static TOUCH_READY: Signal<CriticalSectionRawMutex, ()> = Signal::new();

type TouchRead = Result<TouchData, ()>;

/// A contact, a lift or a cover, rather than a stale or failed read.
fn is_report(read: &TouchRead) -> bool {
    matches!(
        read,
        Ok(TouchData::Points(_) | TouchData::Lifted(_) | TouchData::CoverGesture)
    )
}

fn is_contact(read: &TouchRead) -> bool {
    matches!(read, Ok(TouchData::Points(points)) if !points.is_empty())
}

/// Queues a read for the frame loop without waiting for it to be taken. A newer contact
/// replaces one the frame loop has not taken, and a stale or failed read never displaces a
/// report. A lift or a cover stays, and the next report queues behind it.
fn put_touch_read(read: TouchRead) {
    TOUCH_READS.lock(|reads| {
        let mut reads = reads.borrow_mut();
        let replace = match reads.back() {
            None => false,
            Some(back) if !is_report(&read) && is_report(back) => return,
            Some(back) => {
                !is_report(&read) || is_contact(back) || !is_report(back) || reads.is_full()
            }
        };
        if replace && let Some(back) = reads.back_mut() {
            *back = read;
        } else {
            _ = reads.push_back(read);
        }
    });
    TOUCH_READY.signal(());
}

/// Waits for the oldest read the frame loop has not taken.
async fn take_touch_read() -> TouchRead {
    loop {
        if let Some(read) = TOUCH_READS.lock(|reads| reads.borrow_mut().pop_front()) {
            return read;
        }
        TOUCH_READY.wait().await;
    }
}

#[derive(Clone, Copy)]
enum ZoneChoice {
    Manual(ZoneId),
    Automatic,
    /// Settings were cleared: automatic, with no zone chosen by hand.
    Cleared,
}
pub static PSRAM_HEAP: esp_alloc::EspHeap = esp_alloc::EspHeap::empty();

#[cfg(feature = "gnss-full-power")]
const GNSS_LOW_POWER_MODE: LowPowerMode = LowPowerMode::Disabled;
#[cfg(not(feature = "gnss-full-power"))]
const GNSS_LOW_POWER_MODE: LowPowerMode = LowPowerMode::Adaptive;
/// GSA and GSV, which give the satellites in view and the signal, go out every fourth fix.
/// GSV alone carries every satellite in view, so at every fix it is most of the traffic.
const GNSS_SATELLITE_RATE: NmeaOutputRate = match NmeaOutputRate::every(4) {
    Some(rate) => rate,
    None => panic!("an output rate is 1 to 20 fixes"),
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SensorSnapshot {
    gnss: GnssState,
    clock: ClockState,
    zone: ZoneState,
    /// `None` while the power controller does not answer.
    battery: Option<Battery>,
    /// The last fix's latitude and longitude, kept while there is none.
    position: Option<(i32, i32)>,
}

struct SensorTask {
    power: Option<Axp2101Power<SharedI2cDevice>>,
    rtc: Option<Pcf85063aRtc<SharedI2cDevice>>,
    state: SensorSnapshot,
    rtc_sync_pending: bool,
}

/// Moving this far from where the zone was last looked up, in 1e-7 degrees on either axis, looks
/// it up again. The boundaries are simplified to about this.
const ZONE_LOOKUP_DISTANCE_E7: i32 = 100_000;

/// The zone the clocks show, and where GNSS last placed the device for the automatic one.
struct ZoneTracker {
    mode: ZoneMode,
    manual: Option<ZoneId>,
    automatic: Option<ZoneId>,
    looked_up_at: Option<(i32, i32)>,
}

impl ZoneTracker {
    fn choose(&mut self, choice: ZoneChoice) {
        match choice {
            ZoneChoice::Manual(zone) => {
                self.mode = ZoneMode::Manual;
                self.manual = Some(zone);
            }
            ZoneChoice::Automatic | ZoneChoice::Cleared => {
                self.mode = ZoneMode::Automatic;
                // Look the zone up at the next fix, wherever it is.
                self.looked_up_at = None;
                if matches!(choice, ZoneChoice::Cleared) {
                    self.manual = None;
                }
            }
        }
    }

    fn state(&self) -> ZoneState {
        ZoneState {
            mode: self.mode,
            zone: match self.mode {
                ZoneMode::Automatic => self.automatic,
                ZoneMode::Manual => self.manual,
            },
        }
    }

    /// Looks up the zone at a fix, if automatic mode wants it, a step at a time so the other
    /// tasks on this core keep running. Returns a zone it newly found.
    async fn follow(&mut self, latitude: i32, longitude: i32) -> Option<ZoneId> {
        let moved = self
            .looked_up_at
            .is_none_or(|(last_latitude, last_longitude)| {
                latitude.abs_diff(last_latitude) >= ZONE_LOOKUP_DISTANCE_E7 as u32
                    || longitude.abs_diff(last_longitude) >= ZONE_LOOKUP_DISTANCE_E7 as u32
            });
        if self.mode != ZoneMode::Automatic || !moved {
            return None;
        }
        let started = Instant::now();
        let mut search = DATABASE.locate(latitude, longitude);
        let (mut steps, mut longest) = (0, Duration::MIN);
        let found = loop {
            let step = Instant::now();
            let progress = search.step();
            longest = longest.max(step.elapsed());
            steps += 1;
            match progress {
                tz::Progress::Searching => embassy_futures::yield_now().await,
                tz::Progress::Found(zone) => break Some(zone),
                tz::Progress::Nowhere => break None,
            }
        };
        self.looked_up_at = Some((latitude, longitude));
        info!(
            "[ZONE] {=str} after {} steps in {}us, longest {}us",
            found.map_or("none", |zone| zone.name),
            steps,
            started.elapsed().as_micros(),
            longest.as_micros(),
        );
        // A fix in a sliver between the simplified boundaries keeps the zone it had.
        let found = found?.id;
        (self.automatic != Some(found)).then(|| {
            self.automatic = Some(found);
            found
        })
    }
}

struct MotionTask {
    magnetometer: Option<Bmm350<SharedI2cDevice>>,
    imu: SensorImu,
    accel_lsb_per_g: i32,
    gyro_lsb_per_dps: i32,
}

#[cfg(feature = "gnss-raw-log")]
fn log_raw_nmea(data: &[u8], line: &mut [u8; 256], line_len: &mut usize) {
    for &byte in data {
        if *line_len == line.len() {
            warn!("[GNSS] RAW_TRUNCATED");
            *line_len = 0;
        }
        line[*line_len] = byte;
        *line_len += 1;

        if byte == b'\n' {
            let mut end = *line_len;
            while end != 0 && matches!(line[end - 1], b'\r' | b'\n') {
                end -= 1;
            }
            match core::str::from_utf8(&line[..end]) {
                Ok(sentence) => info!("[GNSS] RAW {=str}", sentence),
                Err(_) => warn!("[GNSS] RAW_NON_UTF8 len={}", end),
            }
            *line_len = 0;
        }
    }
}

macro_rules! start_display_core {
    ($peripherals:ident, $framebuffer_thread:ident) => {
        let swap: &'static mut Swap<SwapState<_>> = SWAP.init_with(|| {
            Swap::new(
                SwapState {
                    fb: FB::alloc(&PSRAM_HEAP),
                    dirty: Dirty::new(),
                    drawn: false,
                    brightness: None,
                    display_on: None,
                    #[cfg(feature = "damage-debug")]
                    debug_changed: Dirty::new(),
                    timings: Timings::default(),
                },
                SwapState {
                    fb: FB::alloc(&PSRAM_HEAP),
                    dirty: Dirty::new(),
                    drawn: false,
                    brightness: None,
                    display_on: None,
                    #[cfg(feature = "damage-debug")]
                    debug_changed: Dirty::new(),
                    timings: Timings::default(),
                },
            )
        });
        let (mut $framebuffer_thread, second_core_swap) = swap.split();

        esp_rtos::start_second_core(
            $peripherals.CPU_CTRL.reborrow(),
            $peripherals.FROM_CPU_INTR1,
            // SAFETY: This static mut value must not be accessed ever again, anywhere
            unsafe { &mut CORE1_STACK },
            || {
                let executor = CORE1_EXECUTOR.init_with(esp_rtos::embassy::Executor::new);
                let io = SecondCore {
                    gpio4: $peripherals.GPIO4,
                    gpio5: $peripherals.GPIO5,
                    gpio6: $peripherals.GPIO6,
                    gpio7: $peripherals.GPIO7,
                    gpio12: $peripherals.GPIO12,
                    gpio13: $peripherals.GPIO13,
                    gpio38: $peripherals.GPIO38,
                    gpio39: $peripherals.GPIO39,
                    dma_ch0: $peripherals.DMA_CH2,
                    spi2: $peripherals.SPI2,
                    swap: second_core_swap,
                };
                executor.run(|spawner| {
                    spawner.spawn(second_core(spawner, io).unwrap());
                })
            },
        );
    };
}

#[esp_hal::main]
fn main() -> ! {
    let mut executor = esp_rtos::embassy::Executor::new();
    let executor: &'static mut esp_rtos::embassy::Executor =
        unsafe { core::mem::transmute(&mut executor) };
    executor.run(|spawner| {
        spawner.spawn(async_main(spawner).unwrap());
    })
}

// PERF: The unit of scheduling for embassy is a task, not an async Future - it may be beneficial to
// move some Futures into their own tasks.
#[embassy_executor::task]
async fn second_core(_spawner: Spawner, io: SecondCore<&'static esp_alloc::EspHeap>) {
    info!("[DISPLAY] core_started");
    let SecondCore {
        gpio4,
        gpio5,
        gpio6,
        gpio7,
        gpio12,
        gpio13,
        gpio38,
        gpio39,
        dma_ch0,
        spi2,
        mut swap,
    } = io;
    let spi_config = spi::master::Config::default()
        .with_frequency(Rate::from_mhz(80))
        .with_mode(spi::Mode::_0);

    let mut dma_tx_command = dma_tx_buffer!(64).unwrap();
    let mut dma_tx = dma_tx_buffer!(4095 * 2).unwrap();
    let mut dma_tx_swap = dma_tx_buffer!(4095 * 2).unwrap();
    let dma_burst = esp_hal::dma::BurstConfig {
        external_memory: esp_hal::dma::ExternalBurstConfig::Size64,
        internal_memory: esp_hal::dma::InternalBurstConfig::Enabled,
    };
    dma_tx_command.set_burst_config(dma_burst).unwrap();
    dma_tx.set_burst_config(dma_burst).unwrap();
    dma_tx_swap.set_burst_config(dma_burst).unwrap();

    let reset = Output::new(gpio39, Level::Low, OutputConfig::default());
    let te = Input::new(gpio13, InputConfig::default());
    let cs = Output::new(gpio12, Level::High, OutputConfig::default());

    let spi = spi::master::Spi::new(spi2, spi_config)
        .expect("SPI failed")
        .with_sck(gpio38)
        .with_sio0(gpio4)
        .with_sio1(gpio5)
        .with_sio2(gpio6)
        .with_sio3(gpio7)
        .with_dma(dma_ch0)
        .into_async();
    let spi = QspiBus::new(spi, dma_tx_command, cs);
    let mut display = Co5300Display::new(spi, reset, te, dma_tx, dma_tx_swap)
        .await
        .expect("display init failed");
    info!("[DISPLAY] OK");

    let mut prev_swap_spi = Duration::MIN;
    let mut first_flush = true;
    loop {
        settings::hold_display_core_if_asked();
        let state = swap.get();
        let SwapState {
            fb,
            timings,
            dirty,
            drawn: _,
            brightness,
            display_on,
            #[cfg(feature = "damage-debug")]
            debug_changed,
        } = state;

        let start = Instant::now();

        if *display_on == Some(true) && display.display_on().await.is_err() {
            warn!("[DISPLAY] display on failed");
        }
        let is_first_flush = first_flush;
        if dirty.is_empty() && !is_first_flush {
            timings.vsync_wait = Duration::MIN;
        } else {
            if is_first_flush {
                debug!("[DISPLAY] first_flush_without_te");
                first_flush = false;
            } else {
                match select(
                    display.wait_for_vsync(),
                    // A wait that starts just after a pulse lasts a whole frame.
                    Timer::after(Duration::from_millis(40)),
                )
                .await
                {
                    Either::First(()) => {}
                    Either::Second(()) => warn!("[DISPLAY] te_timeout"),
                }
            }
            timings.vsync_wait = start.elapsed();
        }
        if let Some(level) = brightness.take()
            && display.set_brightness(level).is_err()
        {
            warn!("[DISPLAY] brightness command failed");
        }

        #[cfg(feature = "timing-log")]
        let (mut regions, mut pixels) = (0u32, 0u32);
        if is_first_flush || dirty.is_full() {
            #[cfg(feature = "damage-debug")]
            let debug_full = debug_changed.is_full();
            #[cfg(not(feature = "damage-debug"))]
            let debug_full = false;
            fb.flush(&mut display, debug_full)
                .await
                .expect("display flush failed");
            #[cfg(feature = "timing-log")]
            {
                (regions, pixels) = (1, board::LCD_WIDTH as u32 * board::LCD_HEIGHT as u32);
            }
        } else {
            for region in dirty.rectangles(chrome::FLUSH_OVERHEAD) {
                #[cfg(feature = "damage-debug")]
                let overlay = debug_changed.intersects(&region).then_some(region);
                #[cfg(not(feature = "damage-debug"))]
                let overlay = None;
                #[cfg(feature = "timing-log")]
                {
                    regions += 1;
                    pixels += region.size.width * region.size.height;
                }
                fb.flush_region(
                    &mut display,
                    region.top_left.x as u16,
                    region.top_left.y as u16,
                    region.size.width as u16,
                    region.size.height as u16,
                    overlay,
                )
                .await
                .expect("display region flush failed");
            }
        };

        if display_on.take() == Some(false) && display.display_off().await.is_err() {
            warn!("[DISPLAY] display off failed");
        }
        timings.spi_time = start.elapsed() - timings.vsync_wait;

        timings.swap_spi = prev_swap_spi;

        #[cfg(feature = "timing-log")]
        info!(
            "timing spi: vsync={}us flush={}us swap={}us regions={} pixels={}",
            timings.vsync_wait.as_micros(),
            timings.spi_time.as_micros(),
            timings.swap_spi.as_micros(),
            regions,
            pixels,
        );

        let before_swap = start.elapsed();
        swap.swap().await;
        prev_swap_spi = start.elapsed() - before_swap;
    }
}

/// Sensor axes into the screen frame of `ui::compass`, fitted from twelve logged poses by the
/// axis check screen and its fitting script, which git history keeps. Both chips sit face down on
/// their boards.
const IMU_AXES: AxisMap = AxisMap([(1, 1.0), (0, 1.0), (2, -1.0)]);
const MAG_AXES: AxisMap = AxisMap([(0, -1.0), (1, -1.0), (2, 1.0)]);
const MOTION_PERIOD: Duration = Duration::from_millis(250);
/// Faster than the frame loop redraws, so every frame has a fresh sample.
const COMPASS_PERIOD: Duration = Duration::from_millis(20);
/// The longest step the fusion integrates the gyro over, in seconds, so a stall does not throw
/// the orientation.
const MAX_FUSION_STEP: f32 = 0.3;

#[embassy_executor::task]
async fn motion_task(task: MotionTask) {
    let MotionTask {
        mut magnetometer,
        mut imu,
        accel_lsb_per_g,
        gyro_lsb_per_dps,
    } = task;
    let mut calibration = Calibration::new();
    let mut fusion = Fusion::new();
    // The heading is set straight from the field when calibration completes, not left to
    // converge from wherever the uncalibrated field put it.
    let mut calibrated = false;
    // The latest calibrated field, for the interference check between magnetometer samples.
    let mut field: Option<Vec3> = None;
    let mut holds = Holds::default();
    let mut last_update: Option<Instant> = None;
    let mut last_log = Instant::now();

    loop {
        let compass_active = COMPASS_ACTIVE.load(Ordering::Relaxed);
        Timer::after(if compass_active {
            COMPASS_PERIOD
        } else {
            MOTION_PERIOD
        })
        .await;
        let log = last_log.elapsed() >= MOTION_PERIOD;
        if log {
            last_log = Instant::now();
        }
        if COMPASS_RECALIBRATE.swap(false, Ordering::Relaxed) {
            calibration = Calibration::new();
            calibrated = false;
            field = None;
            info!("[COMPASS] recalibrating");
        }
        let mut raw_field = None;
        let mut new_field = None;

        if let Some(magnetometer) = &mut magnetometer
            && magnetometer.data_ready().await.unwrap_or(false)
            && let Ok(data) = magnetometer.read_data().await
            && let Some(compensated) = magnetometer.compensate(&data)
        {
            if log {
                debug!(
                    "[BMM350] sample raw=({}, {}, {}) comp=({}, {}, {})uT temp={}C",
                    data.raw.x,
                    data.raw.y,
                    data.raw.z,
                    compensated.x_microtesla,
                    compensated.y_microtesla,
                    compensated.z_microtesla,
                    compensated.temperature_celsius
                );
            }
            let sample = [
                compensated.x_microtesla,
                compensated.y_microtesla,
                compensated.z_microtesla,
            ];
            let sample = MAG_AXES.apply(sample);
            let event = calibration.update(sample);
            let offset = calibration.offset();
            if event != CalibrationEvent::None {
                info!(
                    "[COMPASS] calibration {} offset={}uT radius={}uT",
                    event,
                    offset,
                    calibration.radius()
                );
            }
            raw_field = Some(sample);
            new_field = Some(core::array::from_fn(|axis| sample[axis] - offset[axis]));
            field = new_field;
        }
        let mut accel = None;
        let mut gyro = None;
        if let Ok(sample) = imu.read_sync_sample(&mut embassy_time::Delay).await {
            let accel_micro = sample.accel.map(|raw| {
                [raw.x, raw.y, raw.z].map(|axis| accel_micro_ms2(axis, accel_lsb_per_g))
            });
            let gyro_micro = sample.gyro.map(|raw| {
                [raw.x, raw.y, raw.z].map(|axis| gyro_micro_rad_s(axis, gyro_lsb_per_dps))
            });
            accel = accel_micro.map(|micro| IMU_AXES.apply(micro.map(|value| value as f32 / 1e6)));
            gyro = gyro_micro.map(|micro| IMU_AXES.apply(micro.map(|value| value as f32 / 1e6)));
            if log {
                debug!(
                    "[IMU] sample accel={} gyro={} si_accel={} si_gyro={}",
                    sample.accel, sample.gyro, accel_micro, gyro_micro
                );
            }
        }

        let now = Instant::now();
        let dt = last_update.map_or(0.0, |last: Instant| {
            ((now - last).as_micros() as f32 / 1e6).min(MAX_FUSION_STEP)
        });
        last_update = Some(now);
        if let Some(gyro) = gyro {
            let trusted = calibration.progress() >= 1.0;
            let trusted_field = new_field.filter(|field| trusted && !calibration.disturbed(*field));
            if trusted && !calibrated {
                calibrated = true;
                if let Some(accel) = accel {
                    fusion.reset(accel, trusted_field);
                }
            }
            fusion.update(gyro, accel, raw_field, trusted_field, dt);
        }
        let compass = CompassView::new(
            fusion.attitude(),
            field,
            &calibration,
            &mut holds,
            now.as_micros(),
        );
        if log && compass_active {
            debug!(
                "[COMPASS] screen accel={} field={}uT offset={}uT gyro_offset={} candidate={} view={}",
                accel,
                field,
                calibration.offset(),
                fusion.gyro_offset(),
                calibration
                    .candidate()
                    .map(|candidate| (candidate.progress(), candidate.residual())),
                compass
            );
        }
        MOTION_STATE.signal(Motion { compass });
    }
}

/// Reads the BOOT key on GPIO0, which the key pulls to ground, and passes its presses to the
/// frame loop. A long press goes out as soon as the key has been held for it.
#[embassy_executor::task]
async fn boot_key_task(mut key: Input<'static>) {
    loop {
        key.wait_for_low().await;
        Timer::after(BOOT_KEY_SETTLE).await;
        if key.is_high() {
            continue;
        }
        let press = match select(
            key.wait_for_high(),
            Timer::after(BOOT_KEY_LONG - BOOT_KEY_SETTLE),
        )
        .await
        {
            Either::First(()) => StageKey::Short,
            Either::Second(()) => StageKey::Long,
        };
        let name = match press {
            StageKey::Short => "Short",
            StageKey::Long => "Long",
        };
        info!("[KEY] BOOT {}", name);
        if BOOT_KEY_PRESSES.try_send(press).is_err() {
            warn!("[KEY] BOOT queue full, {} dropped", name);
        }
        key.wait_for_high().await;
        Timer::after(BOOT_KEY_SETTLE).await;
    }
}

/// Saves settings as they change. Each write holds the display core for its length.
#[embassy_executor::task]
async fn settings_task(mut store: Store) {
    loop {
        let write = SETTINGS_WRITES.receive().await;
        let started = Instant::now();
        let saved = settings::with_display_core_held(|| store.save(write)).await;
        SETTINGS_DONE.fetch_add(1, Ordering::Release);
        let took = started.elapsed().as_micros();
        if saved {
            info!("[SETTINGS] {} saved in {}us", write, took);
        } else {
            warn!("[SETTINGS] {} not saved, after {}us", write, took);
        }
    }
}

#[embassy_executor::task]
async fn sensor_task(task: SensorTask) {
    let SensorTask {
        mut power,
        mut rtc,
        mut state,
        mut rtc_sync_pending,
    } = task;
    // Whether GNSS has set the clock since the firmware started.
    let mut clock_set = false;
    // Once a time is injected, GNSS no longer sets the clock, so a fix cannot undo it.
    #[cfg(feature = "rtc-inject")]
    let mut injected = false;
    #[cfg(not(feature = "rtc-inject"))]
    let injected = false;

    loop {
        Timer::after(Duration::from_millis(250)).await;

        if GNSS_PARKED.try_take().is_some()
            && let Some(power) = &mut power
        {
            power_off(power).await;
        }
        if let Some(power) = &mut power {
            match power.take_key_press().await {
                Ok(Some(key)) => {
                    info!("[PMIC] power key {}", key);
                    if KEY_PRESSES.try_send(key).is_err() {
                        warn!("[PMIC] key queue full, {} dropped", key);
                    }
                }
                Ok(None) => {}
                Err(_) => warn!("[PMIC] key read failed"),
            }
            let present = power.is_battery_present().await.ok();
            let battery_present = present.unwrap_or(false);
            let battery_mv = power.get_battery_voltage().await.ok();
            let vbus_mv = power.get_vbus_voltage().await.ok();
            let vsys_mv = power.get_system_voltage().await.ok();
            let percent = power.get_battery_percent().await.ok();
            let charging = power.is_charging().await.ok();
            let usb = power.is_vbus_in().await.ok();
            state.battery = present.map(|present| Battery {
                present,
                percent: percent.unwrap_or(0).min(100),
                millivolts: battery_mv.unwrap_or(0),
                charging: charging.unwrap_or(false),
                usb: usb.unwrap_or(false),
            });
            debug!(
                "[PMIC] sample battery_present={} VBAT={}mV VBUS={}mV VSYS={}mV",
                battery_present, battery_mv, vbus_mv, vsys_mv
            );
        }

        if let Some(gnss) = GNSS_STATE.try_take() {
            state.gnss = gnss;
        }
        #[cfg(feature = "rtc-inject")]
        if let Some((seconds, mode)) = rtc_inject::take()
            && let Some(rtc) = &mut rtc
        {
            let time = tz::DateTime::from_unix(seconds.into());
            let as_gnss = mode == rtc_inject::AS_GNSS;
            if !(2000..=2099).contains(&time.year) || !(mode == rtc_inject::AS_RTC || as_gnss) {
                warn!("[RTC] injection refused: {} mode={}", seconds, mode);
            } else {
                let rtc_time = RtcDateTime::with_weekday(
                    (time.year % 100) as u8,
                    time.month,
                    time.day,
                    time.weekday(),
                    time.hour,
                    time.minute,
                    time.second,
                );
                match rtc.set_time(&rtc_time).await {
                    Ok(()) => {
                        clock_set = as_gnss;
                        injected = true;
                        info!(
                            "[RTC] injected {:04}-{:02}-{:02} {:02}:{:02}:{:02} as {}",
                            time.year,
                            time.month,
                            time.day,
                            time.hour,
                            time.minute,
                            time.second,
                            if as_gnss { "GNSS" } else { "RTC" },
                        );
                    }
                    Err(_) => warn!("[RTC] injection failed"),
                }
            }
        }
        if state.gnss.utc.is_none() {
            rtc_sync_pending = true;
        } else if rtc_sync_pending
            && !injected
            && let Some(utc) = state.gnss.utc
            && let Some(rtc) = &mut rtc
        {
            if !(2000..=2099).contains(&utc.year) {
                warn!("[RTC] GNSS year outside RTC range: {}", utc.year);
            } else {
                let rtc_time = RtcDateTime::with_weekday(
                    (utc.year % 100) as u8,
                    utc.month,
                    utc.day,
                    utc.weekday,
                    utc.hours,
                    utc.minutes,
                    utc.seconds,
                );
                match rtc.set_time(&rtc_time).await {
                    Ok(()) => {
                        rtc_sync_pending = false;
                        clock_set = true;
                        info!(
                            "[RTC] synchronized from GNSS {:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
                            utc.year,
                            utc.month,
                            utc.day,
                            utc.hours,
                            utc.minutes,
                            utc.seconds,
                            utc.milliseconds,
                        );
                    }
                    Err(_) => warn!("[RTC] GNSS synchronization failed"),
                }
            }
        }
        let utc = match &mut rtc {
            Some(rtc) => rtc.get_time().await.ok(),
            None => None,
        };
        let utc = utc.map(|time| {
            tz::DateTime {
                year: 2000 + i32::from(time.year),
                month: time.month,
                day: time.day,
                hour: time.hours,
                minute: time.minutes,
                second: time.seconds,
            }
            .to_unix()
        });
        state.clock = ClockState {
            utc,
            set_from_gnss: clock_set,
            stopped: rtc.as_ref().is_some_and(|rtc| rtc.oscillator_stopped()),
        };

        if let Some(fix) = state.gnss.fix {
            state.position = Some((fix.latitude.get(), fix.longitude.get()));
            ZONE_FIX.signal((fix.latitude.get(), fix.longitude.get()));
        }
        state.zone = ZONE_STATE.lock(Cell::get);

        SENSOR_STATE.signal(state);
    }
}

/// How often the receiver fixes, which start-up leaves at the module's default.
const FIX_PERIOD: Duration = Duration::from_secs(1);
/// How long before a burst is due the reads start. Bursts come up to about 30 ms either side of
/// a second after the last.
const BURST_EARLY: Duration = Duration::from_millis(100);
/// Each burst's reads start this much later than the last one's, over [`DITHER_STEPS`] bursts,
/// which together span one read and [`POLL_GAP`]. A burst is seen at the first read after it
/// arrives, so a grid that kept its phase to the second would see every burst equally late.
const DITHER_STEP: Duration = Duration::from_micros(1_600);
const DITHER_STEPS: u32 = 8;
/// The pause after a read that found nothing. Straight after a read, the module refuses the
/// next and the driver's retry waits 10 ms.
const POLL_GAP: Duration = Duration::from_millis(2);

struct GnssTask {
    gnss: Lc76g<SharedI2cDevice, embassy_time::Delay>,
    nmea_parser: NmeaParser,
}

/// Reads the GNSS module around each second's burst of NMEA, publishes its state, and times
/// UTC from when each burst is first seen. Between bursts it leaves the bus alone.
#[embassy_executor::task]
async fn gnss_task(task: GnssTask) {
    let GnssTask {
        mut gnss,
        mut nmea_parser,
    } = task;
    let mut nmea = [0u8; 512];
    let mut seconds = SecondEstimator::new();
    // When the last burst was seen, which times the reads before a fix.
    let mut last_burst: Option<Instant> = None;
    let mut bursts = 0u32;
    // The last burst was found waiting, so the reads start too late to see the next one arrive.
    let mut missed = true;
    // When the module last copied its navigation data to its flash.
    let mut navigation_saved: Option<Instant> = None;
    #[cfg(feature = "gnss-raw-log")]
    let mut raw_nmea_line = [0; 256];
    #[cfg(feature = "gnss-raw-log")]
    let mut raw_nmea_line_len = 0;

    loop {
        let due = seconds
            .next_burst(
                (Instant::now() + BURST_EARLY).as_micros(),
                FIX_PERIOD.as_micros(),
            )
            .map(Instant::from_micros)
            .or_else(|| last_burst.map(|seen| seen + FIX_PERIOD));
        if let Some(due) = due.filter(|_| !missed) {
            let start = due - BURST_EARLY + DITHER_STEP * (bursts % DITHER_STEPS);
            if let Either::Second(()) = select(Timer::at(start), POWER_OFF.wait()).await {
                park_gnss(&mut gnss, navigation_saved.is_some()).await;
            }
        }

        // A burst counts as seen only after a read found the buffer empty, so bytes left over
        // from the last one are not taken for the next.
        let mut emptied = false;
        let mut empty_reads = 0u32;
        let mut slowest_read = Duration::MIN;
        let (seen, mut available) = loop {
            if POWER_OFF.try_take().is_some() {
                park_gnss(&mut gnss, navigation_saved.is_some()).await;
            }
            let read_started = Instant::now();
            let length = gnss.nmea_length().await;
            slowest_read = slowest_read.max(read_started.elapsed());
            match length {
                Ok(0) => {
                    emptied = true;
                    empty_reads += 1;
                    Timer::after(POLL_GAP).await;
                }
                Ok(available) => break (emptied.then(Instant::now), available),
                Err(error) => {
                    log_gnss_error(error);
                    emptied = false;
                    Timer::after(Duration::from_millis(250)).await;
                }
            }
        };

        let before = nmea_parser.state().utc;
        let mut bytes = 0;
        let mut updates = 0;
        while available > 0 {
            let chunk = &mut nmea[..available.min(512)];
            if let Err(error) = gnss.read_buffered_nmea(chunk).await {
                log_gnss_error(error);
                break;
            }
            bytes += chunk.len();
            #[cfg(feature = "gnss-raw-log")]
            log_raw_nmea(chunk, &mut raw_nmea_line, &mut raw_nmea_line_len);
            for &byte in chunk.iter() {
                match nmea_parser.push(byte) {
                    Ok(Some(NmeaUpdate::PairAck(ack))) => {
                        updates += 1;
                        debug!(
                            "[GNSS] PAIR_ACK command={} status={}",
                            ack.command, ack.status
                        );
                    }
                    Ok(Some(NmeaUpdate::NmeaOutputRate { sentence, rate })) => {
                        updates += 1;
                        debug!(
                            "[GNSS] NMEA_OUTPUT_RATE sentence={} rate={}",
                            sentence, rate
                        );
                    }
                    Ok(Some(NmeaUpdate::Pair(message))) => {
                        updates += 1;
                        debug!(
                            "[GNSS] PAIR_RESPONSE command={} fields={=[u8]:a}",
                            message.command(),
                            message.fields()
                        );
                    }
                    Ok(Some(_)) => updates += 1,
                    Ok(None) => {}
                    Err(error) => warn!("[GNSS] NMEA_PARSE_ERROR {=str}", error),
                }
            }
            available = gnss.nmea_length().await.unwrap_or(0);
        }

        let state = nmea_parser.state();
        GNSS_STATE.signal(state);
        // A burst found waiting says nothing of when bursts arrive: the reads run on until one
        // is seen, at most a period later.
        missed = seen.is_none();
        let previous = last_burst;
        if let Some(seen) = seen {
            last_burst = Some(seen);
            debug!(
                "[GNSS] burst after {}us, {} empty reads before it, slowest {}us",
                previous.map(|last| (seen - last).as_micros()),
                empty_reads,
                slowest_read.as_micros()
            );
            bursts = bursts.wrapping_add(1);
            // Only a valid RMC sets the time, so a new one names this burst's fix.
            if let Some(utc) = state.utc
                && state.utc != before
            {
                let fix = unix_micros(utc);
                let offset = seconds.add(seen.as_micros(), fix);
                GPS_TIME.lock(|time| {
                    time.set(Some(GpsTime {
                        offset,
                        updated: seen,
                    }))
                });
                let late = seen.as_micros() as i64 - fix - offset;
                if seconds.samples() == 1 {
                    info!("[GNSS] UTC anchored to a burst");
                }
                debug!(
                    "[GNSS] burst late={}us over {} fixes",
                    late,
                    seconds.samples()
                );
            }
        }
        let signal = state.signal;
        debug!(
            "[GNSS] acquisition in_view={} with_signal={} used={} strongest_snr_db={} fix_type={} hdop_milli={}",
            signal.satellites_in_view.get(),
            signal.satellites_with_signal.get(),
            signal.satellites_used.get(),
            signal.strongest_snr.map(|value| value.get()),
            signal.fix_type,
            signal.hdop.map(|value| value.get()),
        );
        if let Some(fix) = state.fix {
            debug!(
                "[GNSS] sample bytes={} updates={} lat={} lon={} alt_mm={} sats={} hdop_milli={}",
                bytes,
                updates,
                fix.latitude.get(),
                fix.longitude.get(),
                fix.altitude.map(|value| value.get()),
                fix.satellites,
                fix.hdop.map(|value| value.get()),
            );
        } else {
            debug!("[GNSS] sample bytes={} updates={} no_fix", bytes, updates);
        }

        if state.fix.is_some()
            && navigation_saved.is_none_or(|at| at.elapsed() >= NAVIGATION_SAVE_INTERVAL)
        {
            let saved = gnss.save_navigation_data().await;
            info!("[GNSS] navigation data saved={}", saved.is_ok());
            navigation_saved = Some(Instant::now());
        }
    }
}

fn log_gnss_error(
    error: GnssError<embassy_embedded_hal::shared_bus::I2cDeviceError<esp_hal::i2c::master::Error>>,
) {
    match error {
        GnssError::I2c { operation, error } => {
            warn!("[GNSS] I2C_FAILED operation={} error={}", operation, error)
        }
        GnssError::BufferTooSmall { .. } => warn!("[GNSS] NMEA_BUFFER_TOO_SMALL"),
        GnssError::PairCommand(_) => warn!("[GNSS] PAIR_COMMAND_BUILD_FAILED"),
    }
}

/// A receiver time in microseconds since 1970.
fn unix_micros(utc: GnssDateTime) -> i64 {
    let seconds = tz::DateTime {
        year: i32::from(utc.year),
        month: utc.month,
        day: utc.day,
        hour: utc.hours,
        minute: utc.minutes,
        second: utc.seconds,
    }
    .to_unix();
    seconds * 1_000_000 + i64::from(utc.milliseconds) * 1_000
}

/// Has the GNSS module copy its navigation data to its flash when it has had a fix, for a hot
/// start, then tells `sensor_task` to power off. The module is not read again.
async fn park_gnss(gnss: &mut Lc76g<SharedI2cDevice, embassy_time::Delay>, fixed: bool) -> ! {
    if fixed {
        let saved = gnss.save_navigation_data().await;
        info!("[GNSS] navigation data saved={}", saved.is_ok());
    }
    GNSS_PARKED.signal(());
    core::future::pending().await
}

/// Follows the zone under each fix, and the zone chosen in the settings panel. It stays in thread
/// mode: a lookup yields between its steps, and a yield on `BUS_EXECUTOR` would run it again at
/// once, starving the frame loop for the whole lookup.
#[embassy_executor::task]
async fn zone_task(mut zones: ZoneTracker) {
    loop {
        match select(ZONE_FIX.wait(), ZONE_CHOICE.wait()).await {
            Either::First((latitude, longitude)) => {
                if let Some(zone) = zones.follow(latitude, longitude).await {
                    queue_write(settings::Write::AutomaticZone(zone));
                }
            }
            Either::Second(choice) => zones.choose(choice),
        }
        ZONE_STATE.lock(|state| state.set(zones.state()));
    }
}

/// Reads the touch controller when it signals a report, or when the frame loop asks through
/// `TOUCH_POLL`, and queues each read for the frame loop.
#[embassy_executor::task]
async fn touch_task(mut touch: TouchDriver) {
    loop {
        if let Either::First(Err(_)) = select(touch.wait_for_touch(), TOUCH_POLL.wait()).await {
            continue;
        }
        let read = touch.read_touch_data().await.map_err(|_| ());
        // A poll asked for while this read ran is answered by it.
        TOUCH_POLL.reset();
        put_touch_read(read);
    }
}

/// Moves a value to another executor on core 0. esp-hal's async drivers are not `Send`, since
/// each binds its interrupt to the core that made it, but a move between executors on the same
/// core keeps that binding.
struct OnCore0<T>(T);

// SAFETY: an `OnCore0` is only sent to `BUS_EXECUTOR`, which runs on core 0, where every
// value it carries was made.
unsafe impl<T> Send for OnCore0<T> {}

/// The tasks that use the I2C bus, for `BUS_EXECUTOR`.
struct BusTasks {
    sensor: SensorTask,
    gnss: GnssTask,
    motion: Option<MotionTask>,
    touch: Option<TouchDriver>,
    radio: Option<RadioTask>,
}

/// Spawns the bus tasks from inside `BUS_EXECUTOR`, whose own spawner takes tasks that are not
/// `Send`.
#[embassy_executor::task]
async fn start_bus_tasks(tasks: OnCore0<BusTasks>) {
    // SAFETY: this runs as an embassy task, polled with the executor's own context.
    let spawner = unsafe { Spawner::for_current_executor() }.await;
    let OnCore0(BusTasks {
        sensor,
        gnss,
        motion,
        touch,
        radio,
    }) = tasks;
    spawner.spawn(sensor_task(sensor).unwrap());
    spawner.spawn(gnss_task(gnss).unwrap());
    if let Some(motion) = motion {
        spawner.spawn(motion_task(motion).unwrap());
    }
    if let Some(touch) = touch {
        spawner.spawn(touch_task(touch).unwrap());
    }
    if let Some(radio) = radio {
        spawner.spawn(radio_task(radio).unwrap());
    }
}

struct RadioTask {
    lora: SensorLora,
    dio0: Input<'static>,
    path: LoraPath,
}

/// Owns the radio. Without a link test it leaves the radio as start-up configured it.
#[embassy_executor::task]
async fn radio_task(task: RadioTask) {
    let RadioTask {
        mut lora,
        mut dio0,
        mut path,
    } = task;

    #[cfg(feature = "lora-link-tx")]
    {
        lora.map_dio0::<TxDone>().await.unwrap();
        let mut sequence = 0u32;
        loop {
            Timer::after(Duration::from_millis(250)).await;
            let mut payload = *b"OWLK\0\0\0\0";
            payload[4..].copy_from_slice(&sequence.to_be_bytes());
            if path.transmit().await.is_err() {
                warn!("[LORA] LINK_TX_SWITCH_FAILED");
                continue;
            }
            if lora.tx(&payload).await.is_err() {
                warn!("[LORA] LINK_TX_FAILED");
            } else {
                match select(
                    dio0.wait_for_rising_edge(),
                    Timer::after(Duration::from_secs(2)),
                )
                .await
                {
                    Either::First(()) => {
                        info!("[LORA] LINK_TX_DONE sequence={}", sequence);
                        let _ = lora.clear_interrupt::<TxDone>().await;
                    }
                    Either::Second(()) => warn!("[LORA] LINK_TX_TIMEOUT"),
                }
            }
            sequence = sequence.wrapping_add(1);
            let _ = path.receive().await;
        }
    }

    #[cfg(feature = "lora-link-rx")]
    {
        lora.map_dio0::<RxDone>().await.unwrap();
        if path.receive().await.is_err() {
            warn!("[LORA] LINK_RX_SWITCH_FAILED");
        }
        // Continuous receive stays listening between packets, so none is missed while one is
        // read out.
        if lora.rx(None).await.is_err() {
            warn!("[LORA] LINK_RX_START_FAILED");
        }
        info!("[LORA] LINK_RX_START");
        loop {
            // DIO0 stays high until RxDone is cleared, so a level wait cannot miss a packet.
            match select(dio0.wait_for_high(), Timer::after(Duration::from_secs(2))).await {
                Either::First(()) => match lora.rx_packet().await {
                    Ok(packet) => info!(
                        "[LORA] LINK_RX_OK len={} rssi={} snr_raw={} payload={}",
                        packet.length,
                        packet.rssi,
                        packet.snr_raw,
                        packet.payload(),
                    ),
                    Err(_) => warn!("[LORA] LINK_RX_PACKET_FAILED"),
                },
                Either::Second(()) => warn!("[LORA] LINK_RX_TIMEOUT"),
            }
            let _ = lora.clear_all_interrupts().await;
        }
    }

    #[cfg(not(any(feature = "lora-link-tx", feature = "lora-link-rx")))]
    core::future::pending::<()>().await;
}

fn bench_repeat<R>(mut the_thing: impl FnMut() -> R, name: &str) -> (R, Duration) {
    const ITERS: u32 = 10;
    let start = Instant::now();
    let mut ret;
    let mut iter = 0;
    loop {
        ret = core::hint::black_box(the_thing());
        iter += 1;
        if iter >= ITERS {
            break;
        }
    }
    let took = start.elapsed() / ITERS;
    info!("{=str}: {}ms", name, took.as_micros() as f32 / 1_000.);
    (ret, took)
}

#[cfg(feature = "fontdue-target-bench")]
async fn run_fontdue_target_benchmark(font: &'static dyn fontdue::FontRepr) -> ! {
    const REPEATS: u32 = 8;
    const SAMPLES: usize = 5;
    const SIZES: [f32; 3] = [12.0, 32.0, 64.0];
    const GLYPHS: &[u8] = b" !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~";
    const ARM: &str = if cfg!(feature = "fontdue-target-bench-gcc") {
        "B"
    } else {
        "A"
    };
    const POINT_BYTES: usize = core::mem::size_of::<fontdue::math::Point>();
    let mut results = [(0_u32, 0_u32); 3];

    for (result, px) in results.iter_mut().zip(SIZES) {
        let mut raster = fontdue::raster::Raster::empty();
        let mut samples = [0_u32; SAMPLES];
        for sample in &mut samples {
            let start = esp_hal::xtensa_lx::timer::get_cycle_count();
            for _ in 0..REPEATS {
                for &character in GLYPHS {
                    let _ = font.rasterize(&mut raster, character as char, px);
                }
            }
            let cycles = esp_hal::xtensa_lx::timer::get_cycle_count().wrapping_sub(start);
            *sample = cycles / (REPEATS * GLYPHS.len() as u32);
        }
        let mut checksum = 0usize;
        for _ in 0..REPEATS {
            for &character in GLYPHS {
                let (_, bitmap) = font.rasterize(&mut raster, character as char, px);
                checksum =
                    checksum.wrapping_add(bitmap.map(|coverage| coverage as usize).sum::<usize>());
            }
        }
        samples.sort_unstable();
        *result = (samples[SAMPLES / 2], checksum as u32);
    }
    info!(
        "[FONTDUE-BENCH] arm={=str} point_bytes={} px12={} px32={} px64={} checksums={},{},{}",
        ARM,
        POINT_BYTES,
        results[0].0,
        results[1].0,
        results[2].0,
        results[0].1,
        results[1].1,
        results[2].1
    );
    loop {
        info!(
            "[FONTDUE-BENCH] repeat arm={=str} point_bytes={} px12={} px32={} px64={} checksums={},{},{}",
            ARM,
            POINT_BYTES,
            results[0].0,
            results[1].0,
            results[2].0,
            results[0].1,
            results[1].1,
            results[2].1
        );
        Timer::after(Duration::from_secs(1)).await;
    }
}

#[derive(Debug, Default)]
struct Timings {
    vsync_wait: Duration,
    spi_time: Duration,
    swap_draw: Duration,
    swap_spi: Duration,
    frametime: Duration,
}

/// Marks the one-pixel border of `region`.
#[cfg(feature = "damage-debug")]
fn add_outline(damage: &mut Dirty, region: embedded_graphics::primitives::Rectangle) {
    let Some(corner) = region.bottom_right() else {
        return;
    };
    let (left, top) = (region.top_left.x, region.top_left.y);
    for (from, to) in [
        ((left, top), (corner.x, top)),
        ((left, corner.y), (corner.x, corner.y)),
        ((left, top), (left, corner.y)),
        ((corner.x, top), (corner.x, corner.y)),
    ] {
        damage.add(embedded_graphics::primitives::Rectangle::with_corners(
            Point::new(from.0, from.1),
            Point::new(to.0, to.1),
        ));
    }
}

struct SwapState<A: Allocator = alloc::alloc::Global> {
    fb: Box<chrome::FB, A>,
    /// What the display core flushes from `fb`.
    dirty: Dirty,
    /// `fb` holds a whole frame. Until it does, it is drawn in full.
    drawn: bool,
    /// The display level to set as this frame goes out.
    brightness: Option<u8>,
    /// Switch the panel on before this frame goes out, or off once it has.
    display_on: Option<bool>,
    /// The step's own damage, which the display core outlines.
    #[cfg(feature = "damage-debug")]
    debug_changed: Dirty,
    timings: Timings,
}

type TouchDriver = Cst9217<SharedI2cDevice, Input<'static>, Output<'static>, embassy_time::Delay>;

/// What each part gets before the self-test fails it. The touch controller's own start-up
/// settles for 220 ms, and the magnetometer's reads its trim in steps with waits between.
const POWER_DEADLINE: Duration = Duration::from_millis(200);
const CLOCK_DEADLINE: Duration = Duration::from_millis(200);
const TOUCH_DEADLINE: Duration = Duration::from_millis(600);
const MOTION_DEADLINE: Duration = Duration::from_millis(500);
const MAGNET_DEADLINE: Duration = Duration::from_millis(500);
/// With a fix, how often the GNSS module copies its navigation data to its flash, so a loss of
/// power keeps the satellites' orbits and the last position. Its RTC RAM keeps them otherwise.
const NAVIGATION_SAVE_INTERVAL: Duration = Duration::from_secs(30 * 60);
/// How long the GNSS gets to answer before it is reset, and in all, a reset and its settle
/// included.
const GNSS_ANSWER: Duration = Duration::from_millis(1500);
const GNSS_DEADLINE: Duration = Duration::from_millis(4500);

/// Each part's outcome as boot decides it, for the self-test.
static BOOT_REPORTS: Channel<CriticalSectionRawMutex, Report, 6> = Channel::new();

/// Runs one part's bring-up against its deadline and reports how it ended. Returns whether the
/// part answered.
async fn probe(
    part: Part,
    deadline: Duration,
    bring_up: impl Future<Output = Result<(), Outcome>>,
) -> bool {
    let started = Instant::now();
    let outcome = match with_timeout(deadline, bring_up).await {
        Ok(Ok(())) => Outcome::Answered,
        Ok(Err(outcome)) => outcome,
        Err(TimeoutError) => Outcome::NoReply,
    };
    info!(
        "[BOOT] {} {} in {}ms",
        part,
        outcome,
        started.elapsed().as_millis()
    );
    if BOOT_REPORTS.try_send(Report { part, outcome }).is_err() {
        warn!("[BOOT] report queue full, {} not shown", part);
    }
    outcome == Outcome::Answered
}

/// The peripherals the parts behind the self-test take.
struct Parts {
    i2c: peripherals::I2C0<'static>,
    scl: peripherals::GPIO14<'static>,
    sda: peripherals::GPIO15<'static>,
    lora_spi: peripherals::SPI3<'static>,
    lora_sck: peripherals::GPIO16<'static>,
    lora_mosi: peripherals::GPIO43<'static>,
    lora_miso: peripherals::GPIO17<'static>,
    lora_cs: peripherals::GPIO18<'static>,
    lora_dio0: peripherals::GPIO44<'static>,
    touch_rst: peripherals::GPIO40<'static>,
    touch_int: peripherals::GPIO11<'static>,
    imu_int2: peripherals::GPIO21<'static>,
    bus_interrupt: peripherals::FROM_CPU_INTR2<'static>,
}

#[embassy_executor::task]
async fn async_main(spawner: Spawner) {
    // A third of the heap lives in the RAM the bootloader frees, which is not static memory, so
    // core 0's stack gets the rest of DRAM.
    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 72 * 1024);
    esp_alloc::heap_allocator!(size: 168 * 1024);

    // PERF: How low do we want to drop the clock speed?
    let mut peripherals =
        esp_hal::init(esp_hal::Config::default().with_cpu_clock(esp_hal::clock::CpuClock::_240MHz));

    let psram_config = esp_hal::psram::PsramConfig {
        mode: esp_hal::psram::PsramMode::OctalSpi,
        size: esp_hal::psram::PsramSize::AutoDetect,
        core_clock: None,
        flash_frequency: esp_hal::psram::FlashFreq::FlashFreq80m,
        ram_frequency: esp_hal::psram::SpiRamFreq::Freq80m,
    };
    unsafe {
        let psram = esp_hal::psram::Psram::new(peripherals.PSRAM, psram_config);
        let (start, size) = psram.raw_parts();
        PSRAM_HEAP.add_region(esp_alloc::HeapRegion::new(
            start,
            size,
            esp_alloc::MemoryCapability::External.into(),
        ));
    }
    let timg0 = TimerGroup::new(peripherals.TIMG0);

    esp_rtos::start(timg0.timer0, peripherals.FROM_CPU_INTR0);

    #[cfg(feature = "fontdue-target-bench")]
    {
        run_fontdue_target_benchmark(chrome::FONTS[0]).await;
    }

    // Settings load before the display core starts, since a flash read holds it.
    // SAFETY: the display core has not started, and `settings_task` holds it for every write.
    // The seed only spreads wear across pages, so the RNG need not be at full entropy.
    let seed = esp_hal::rng::Rng::new().random();
    let mut store = unsafe { Store::new(esp_storage::FlashStorage::new(peripherals.FLASH), seed) };
    let saved = store.load();
    info!(
        "[SETTINGS] zone mode={} manual={} automatic={} brightness={} timeout={} always_on={}",
        saved.zone_mode,
        saved.manual_zone.map(|zone| DATABASE.zone(zone).name),
        saved.automatic_zone.map(|zone| DATABASE.zone(zone).name),
        saved.brightness,
        saved.timeout.map(|timeout| timeout.label()),
        saved.always_on.map(choice_label),
    );
    spawner.spawn(settings_task(store).unwrap());
    // The board has no pull-up of its own on GPIO0 past reset.
    let boot_key = Input::new(
        peripherals.GPIO0,
        InputConfig::default().with_pull(Pull::Up),
    );
    spawner.spawn(boot_key_task(boot_key).unwrap());

    // The panel comes up first, so the self-test shows while the parts come up behind it.
    start_display_core!(peripherals, fb_st);

    let stage = Stage::starting(PeripheralState {
        brightness: saved.brightness.unwrap_or(DEFAULT_BRIGHTNESS),
        timeout: saved.timeout.unwrap_or_default(),
        always_on: saved.always_on.unwrap_or_default(),
        firmware: env!("CARGO_PKG_VERSION"),
        ..PeripheralState::default()
    });
    let parts = Parts {
        i2c: peripherals.I2C0,
        scl: peripherals.GPIO14,
        sda: peripherals.GPIO15,
        lora_spi: peripherals.SPI3,
        lora_sck: peripherals.GPIO16,
        lora_mosi: peripherals.GPIO43,
        lora_miso: peripherals.GPIO17,
        lora_cs: peripherals.GPIO18,
        lora_dio0: peripherals.GPIO44,
        touch_rst: peripherals.GPIO40,
        touch_int: peripherals.GPIO11,
        imu_int2: peripherals.GPIO21,
        bus_interrupt: peripherals.FROM_CPU_INTR2,
    };
    let zones = ZoneTracker {
        mode: saved.zone_mode,
        manual: saved.manual_zone,
        automatic: saved.automatic_zone,
        looked_up_at: None,
    };
    join(bring_up(spawner, parts, zones), frame_loop(stage, fb_st)).await;
}

/// Brings up every part behind the self-test, each against its deadline, then starts the tasks
/// that own them and hands the touch controller to the frame loop. A part that fails is left
/// out, and its owner runs without it.
async fn bring_up(spawner: Spawner, parts: Parts, zones: ZoneTracker) {
    let bus = BUS_EXECUTOR
        .init(esp_rtos::embassy::InterruptExecutor::new(
            parts.bus_interrupt,
        ))
        .start(esp_hal::interrupt::Priority::Priority1);

    // The I²C pull-ups share VCC3V3 with the secondary board.
    Timer::after(Duration::from_millis(board::I2C_POWER_SETTLE_MS)).await;

    // A device can end a read early, which leaves the peripheral waiting for commands that never
    // run; without a deadline the driver yields for ever, and a yield on `BUS_EXECUTOR` starves
    // the core. The timeout is about nine byte times at the bus clock.
    let i2c = I2c::new(
        parts.i2c,
        esp_hal::i2c::master::Config::default()
            .with_frequency(Rate::from_hz(board::I2C_FREQ_HZ))
            .with_software_timeout(esp_hal::i2c::master::SoftwareTimeout::PerByte(
                esp_hal::time::Duration::from_micros(200),
            )),
    )
    .expect("I2C failed")
    .with_scl(parts.scl)
    .with_sda(parts.sda)
    .into_async();
    let i2c = I2C_BUS.init(Mutex::<CriticalSectionRawMutex, _>::new(i2c));
    let i2c = I2cDevice::new(i2c);

    let mut power = Axp2101Power::new(i2c.clone());
    let answered = probe(Part::Power, POWER_DEADLINE, async {
        power.init().await.map_err(|error| match error {
            Axp2101Error::I2c(_) => Outcome::NoReply,
            Axp2101Error::WrongChipId(_) => Outcome::BadReply,
        })?;
        power
            .configure_power_key()
            .await
            .map_err(|_| Outcome::NoReply)
    })
    .await;
    if answered && let Ok(sources) = power.power_sources().await {
        info!(
            "[PMIC] powered on by {=u8:#04x}, last off by {=u8:#04x}",
            sources.on, sources.off
        );
    }
    let mut power = answered.then_some(power);
    let mut initial_sensor_state = SensorSnapshot::default();
    if reset_lora(i2c.clone()).await.is_err() {
        error!("[TCA9554] unavailable; the GNSS and LoRa resets and the RF switch are not driven");
    }

    // The parts on the I²C bus come up while the GNSS module settles out of reset.
    let mut rtc = Pcf85063aRtc::new(i2c.clone());
    let touch_rst = Output::new(parts.touch_rst, Level::High, OutputConfig::default());
    let touch_int = Input::new(parts.touch_int, InputConfig::default());
    let mut touch = Cst9217::new(i2c.clone(), touch_rst, touch_int, embassy_time::Delay);
    let gyro_range = ph_qmi8658::GyroRange::Dps512;
    let accel_range = ph_qmi8658::AccelRange::G2;
    let lpf = ph_qmi8658::LowPassFilterMode::OdrPercent2_62;
    let config = ph_qmi8658::Config {
        accel: Some(ph_qmi8658::AccelConfig {
            range: accel_range,
            odr: ph_qmi8658::AccelOutputDataRate::Hz125,
            lpf: Some(lpf),
        }),
        gyro: Some(ph_qmi8658::GyroConfig {
            range: gyro_range,
            odr: ph_qmi8658::GyroOutputDataRate::Hz125,
            lpf: Some(lpf),
        }),
    };
    let mut imu = ph_qmi8658::Qmi8658::with_i2c_config(
        i2c.clone(),
        None::<core::convert::Infallible>,
        Some(Input::new(parts.imu_int2, InputConfig::default())),
        config,
        // The QMI8658 I2C output registers are low-byte first.
        ph_qmi8658::I2cConfig::new(board::IMU_I2C_ADDR).with_big_endian(false),
    );
    let mut magnetometer = Bmm350::new(i2c.clone());
    let ((), (rtc_ok, touch_ok, imu_ok, magnetometer_ok)) = join(
        async {
            debug!("[GNSS] STARTUP settle_begin");
            Timer::after(Duration::from_secs(1)).await;
            debug!("[GNSS] STARTUP settle_complete");
        },
        async {
            let rtc_ok = probe(Part::Clock, CLOCK_DEADLINE, async {
                rtc.init().await.map_err(|_| Outcome::NoReply)?;
                match rtc.get_time().await {
                    Ok(time) => info!(
                        "[RTC] OK 20{:02}-{:02}-{:02} {:02}:{:02}:{:02}",
                        time.year, time.month, time.day, time.hours, time.minutes, time.seconds
                    ),
                    // It read, and holds no valid time. The clock face shows that.
                    Err(RtcError::InvalidDateTime) => warn!("[RTC] TIME_INVALID"),
                    Err(RtcError::I2c(_)) => return Err(Outcome::NoReply),
                }
                Ok(())
            })
            .await;
            let touch_ok = probe(Part::Touch, TOUCH_DEADLINE, async {
                touch.init().await.map_err(|error| match error {
                    Cst9217Error::I2CError(_) | Cst9217Error::ResetError(_) => Outcome::NoReply,
                    Cst9217Error::IDMismatch
                    | Cst9217Error::NoFirmware
                    | Cst9217Error::InvalidCheckcode => Outcome::BadReply,
                })
            })
            .await;
            let imu_ok = probe(Part::Motion, MOTION_DEADLINE, async {
                let outcome = |error| match error {
                    ph_qmi8658::Error::Bus | ph_qmi8658::Error::NotPresent => Outcome::NoReply,
                    _ => Outcome::BadReply,
                };
                imu.init(&mut embassy_time::Delay).await.map_err(outcome)?;
                imu.set_mode_with_delay(
                    &mut embassy_time::Delay,
                    ph_qmi8658::OperatingMode::AccelGyroOnly,
                )
                .await
                .map_err(outcome)?;
                imu.set_sync_sample(true).await.map_err(outcome)
            })
            .await;
            let magnetometer_ok = probe(Part::Magnet, MAGNET_DEADLINE, async {
                let outcome = |error| match error {
                    MagnetometerError::I2c(_) => Outcome::NoReply,
                    _ => Outcome::BadReply,
                };
                magnetometer.init().await.map_err(outcome)?;
                magnetometer
                    .start_normal_mode()
                    .await
                    .map_err(|_| Outcome::NoReply)?;
                Timer::after(Duration::from_millis(15)).await;
                if !magnetometer
                    .data_ready()
                    .await
                    .map_err(|_| Outcome::NoReply)?
                {
                    return Err(Outcome::BadReply);
                }
                let data = magnetometer
                    .read_data()
                    .await
                    .map_err(|_| Outcome::NoReply)?;
                let compensated = magnetometer.compensate(&data).ok_or(Outcome::BadReply)?;
                debug!(
                    "[BMM350] COMP x={}uT y={}uT z={}uT temp={}C",
                    compensated.x_microtesla,
                    compensated.y_microtesla,
                    compensated.z_microtesla,
                    compensated.temperature_celsius
                );
                Ok(())
            })
            .await;
            (rtc_ok, touch_ok, imu_ok, magnetometer_ok)
        },
    )
    .await;

    let mut gnss = Lc76g::new(i2c.clone(), embassy_time::Delay);
    let mut nmea_parser = NmeaParser::new();
    // Resetting the module clears its time, so it is reset only when it does not answer: a module
    // left stuck by the firmware before this one needs it.
    let mut gnss_reset = false;
    let gnss_ok = probe(Part::Gnss, GNSS_DEADLINE, async {
        let answered = with_timeout(GNSS_ANSWER, configure_gnss(&mut gnss, &mut nmea_parser)).await;
        if matches!(answered, Ok(Ok(()))) {
            return Ok(());
        }
        warn!("[GNSS] no answer; resetting it");
        gnss_reset = true;
        pulse_gnss_reset(&mut i2c.clone())
            .await
            .map_err(|()| Outcome::NoReply)?;
        Timer::after(Duration::from_secs(1)).await;
        configure_gnss(&mut gnss, &mut nmea_parser).await
    })
    .await;
    // The module starts without a time after a reset or a power-on. A reset of the chip alone,
    // by the button or a debugger, reads as a power-on too, since both drive `CHIP_PU`.
    let powered_on = matches!(
        esp_hal::system::reset_reason(),
        Some(esp_hal::rtc_cntl::SocResetReason::ChipPowerOn)
    );
    if gnss_ok && rtc_ok && (gnss_reset || powered_on) {
        match rtc.get_time().await {
            Ok(time) if !rtc.oscillator_stopped() => {
                let reference = GnssDateTime {
                    year: 2000 + u16::from(time.year),
                    month: time.month,
                    day: time.day,
                    weekday: time.weekday,
                    hours: time.hours,
                    minutes: time.minutes,
                    seconds: time.seconds,
                    milliseconds: 0,
                };
                let sent = gnss.set_reference_time(&reference).await;
                info!(
                    "[GNSS] reference time 20{:02}-{:02}-{:02} {:02}:{:02}:{:02} sent={}",
                    time.year,
                    time.month,
                    time.day,
                    time.hours,
                    time.minutes,
                    time.seconds,
                    sent.is_ok()
                );
            }
            _ => info!("[GNSS] no reference time: the RTC has none"),
        }
    }
    info!(
        "[BOOT] answered: power={} clock={} touch={} motion={} magnet={} gnss={}",
        power.is_some(),
        rtc_ok,
        touch_ok,
        imu_ok,
        magnetometer_ok,
        gnss_ok
    );
    initial_sensor_state.gnss = nmea_parser.state();

    let lora = start_lora(
        parts.lora_spi,
        parts.lora_sck,
        parts.lora_mosi,
        parts.lora_miso,
        parts.lora_cs,
    )
    .await;
    let radio = lora.map(|lora| RadioTask {
        lora,
        dio0: Input::new(parts.lora_dio0, InputConfig::default()),
        path: LoraPath::new(
            i2c.clone(),
            !((1 << board::EXIO_GPS_RESET)
                | (1 << board::EXIO_LORA_RESET)
                | (1 << board::EXIO_LORA_TX_SWITCH)),
        ),
    });

    if touch_ok {
        touch.set_config(Cst9217Config {
            mirror_x: true,
            mirror_y: true,
            scale_x: None,
            scale_y: None,
            swap_xy: false,
        });
        let (resolution, (firmware, checksum)) = (touch.resolution(), touch.firmware());
        info!(
            "[TOUCH] OK resolution={}x{} firmware={=u32:#x} checksum={=u32:#x}",
            resolution.width, resolution.height, firmware, checksum
        );
    }

    ZONE_STATE.lock(|state| state.set(zones.state()));
    spawner.spawn(zone_task(zones).unwrap());
    // From here on the bus belongs to `BUS_EXECUTOR`'s tasks.
    bus.spawn(
        start_bus_tasks(OnCore0(BusTasks {
            sensor: SensorTask {
                power,
                rtc: rtc_ok.then_some(rtc),
                state: initial_sensor_state,
                rtc_sync_pending: true,
            },
            gnss: GnssTask { gnss, nmea_parser },
            // The compass needs the IMU for its tilt. Without it, it shows NO DATA.
            motion: imu_ok.then(|| MotionTask {
                magnetometer: magnetometer_ok.then_some(magnetometer),
                imu,
                accel_lsb_per_g: ph_qmi8658::accel_lsb_per_g(accel_range),
                gyro_lsb_per_dps: ph_qmi8658::gyro_lsb_per_dps(gyro_range),
            }),
            touch: touch_ok.then_some(touch),
            radio,
        }))
        .unwrap(),
    );
    info!(
        "[MEM] internal_used={} psram_used={}",
        esp_alloc::HEAP.used(),
        PSRAM_HEAP.used()
    );
}

/// Holds the radio in reset, then releases it listening. The GNSS reset is left released, since a
/// reset clears the module's time; `pulse_gnss_reset` resets it when it has to be.
async fn reset_lora(i2c: SharedI2cDevice) -> Result<(), ()> {
    let mut exio = Tca9554::new(i2c, tca9554::Address::standard());
    let gps_reset = 1 << board::EXIO_GPS_RESET;
    let lora_reset = 1 << board::EXIO_LORA_RESET;
    let lora_rx_switch = 1 << board::EXIO_LORA_RX_SWITCH;
    let lora_tx_switch = 1 << board::EXIO_LORA_TX_SWITCH;
    let fail = |_| ();
    exio.init().await.map_err(fail)?;
    // Set while every pin is still an input, so no output starts high and then falls. The GNSS
    // reset's bit stays low in every write, for `pulse_gnss_reset`.
    exio.write_output(!(gps_reset | lora_reset | lora_tx_switch))
        .await
        .map_err(fail)?;
    let output_mask = lora_reset | lora_rx_switch | lora_tx_switch;
    exio.write_direction(!output_mask).await.map_err(fail)?;
    info!("[TCA9554] OK");
    Timer::after(Duration::from_millis(10)).await;
    exio.write_output(!(gps_reset | lora_tx_switch))
        .await
        .map_err(fail)?;
    Timer::after(Duration::from_micros(200)).await;
    exio.write_output(!(gps_reset | lora_reset | lora_tx_switch))
        .await
        .map_err(fail)?;
    Timer::after(Duration::from_millis(10)).await;
    let direction = exio.read_direction().await.map_err(fail)?;
    debug!("[TCA9554] direction={=u8:#04x}", direction);
    Ok(())
}

/// Holds the GNSS module in reset for 10 ms, which also clears its time.
///
/// The module's `RESET_N` is pulled up inside it to 1.8 V and must be driven open-drain, so the
/// pin's output bit stays low and the pin is an output only while it holds the reset.
async fn pulse_gnss_reset(i2c: &mut SharedI2cDevice) -> Result<(), ()> {
    const OUTPUT: u8 = 0x01;
    const DIRECTION: u8 = 0x03;
    let reset = 1 << board::EXIO_GPS_RESET;
    let mut direction = [0];
    i2c.write_read(board::TCA9554_I2C_ADDR, &[DIRECTION], &mut direction)
        .await
        .map_err(|_| ())?;
    let mut output = [0];
    i2c.write_read(board::TCA9554_I2C_ADDR, &[OUTPUT], &mut output)
        .await
        .map_err(|_| ())?;
    i2c.write(board::TCA9554_I2C_ADDR, &[OUTPUT, output[0] & !reset])
        .await
        .map_err(|_| ())?;
    i2c.write(board::TCA9554_I2C_ADDR, &[DIRECTION, direction[0] & !reset])
        .await
        .map_err(|_| ())?;
    Timer::after(Duration::from_millis(10)).await;
    i2c.write(board::TCA9554_I2C_ADDR, &[DIRECTION, direction[0] | reset])
        .await
        .map_err(|_| ())
}

/// Sends the receiver its configuration and reads what it has sent. It answers if any command
/// is accepted or a read succeeds.
async fn configure_gnss(
    gnss: &mut Lc76g<SharedI2cDevice, embassy_time::Delay>,
    nmea_parser: &mut NmeaParser,
) -> Result<(), Outcome> {
    let mut answered = false;
    debug!(
        "[GNSS] STARTUP PAIR_SEND command=732 mode={}",
        GNSS_LOW_POWER_MODE
    );
    match gnss.set_low_power_mode(GNSS_LOW_POWER_MODE).await {
        Ok(()) => answered = true,
        Err(error) => warn!(
            "[GNSS] STARTUP PAIR_SEND_RESULT command=732 status=error error={}",
            error
        ),
    }
    for (command, result) in [
        (410, gnss.set_sbas(true).await),
        (411, gnss.query_sbas().await),
    ] {
        match result {
            Ok(()) => answered = true,
            Err(error) => warn!(
                "[GNSS] STARTUP PAIR_SEND_RESULT command={} status=error error={}",
                command, error
            ),
        }
    }
    // GLL and VTG repeat what GGA and RMC carry, and nothing reads them.
    for (sentence, rate) in [
        (NmeaSentence::Gsa, GNSS_SATELLITE_RATE),
        (NmeaSentence::Gsv, GNSS_SATELLITE_RATE),
        (NmeaSentence::Gll, NmeaOutputRate::DISABLED),
        (NmeaSentence::Vtg, NmeaOutputRate::DISABLED),
    ] {
        match gnss.set_nmea_output_rate(sentence, rate).await {
            Ok(()) => answered = true,
            Err(error) => warn!(
                "[GNSS] STARTUP PAIR_SEND_RESULT command=062 status=error error={}",
                error
            ),
        }
        match gnss.query_nmea_output_rate(sentence).await {
            Ok(()) => answered = true,
            Err(error) => warn!(
                "[GNSS] STARTUP PAIR_SEND_RESULT command=063 status=error error={}",
                error
            ),
        }
    }
    if let Ok(command) = PairCommandBuilder::new(67).and_then(|builder| builder.finish()) {
        match gnss.send_pair_command(&command).await {
            Ok(()) => answered = true,
            Err(error) => warn!(
                "[GNSS] STARTUP PAIR_SEND_RESULT command=067 status=error error={}",
                error
            ),
        }
    }
    let mut nmea = [0u8; 512];
    match gnss.read_nmea_chunk(&mut nmea).await {
        Ok(data) => {
            answered = true;
            for &byte in data {
                if let Err(error) = nmea_parser.push(byte) {
                    warn!("[GNSS] NMEA_PARSE_ERROR {=str}", error);
                }
            }
        }
        Err(GnssError::I2c { operation, error }) => {
            warn!("[GNSS] I2C_FAILED operation={} error={}", operation, error);
        }
        Err(GnssError::BufferTooSmall { .. }) => warn!("[GNSS] NMEA_BUFFER_TOO_SMALL"),
        Err(GnssError::PairCommand(_)) => warn!("[GNSS] PAIR_COMMAND_BUILD_FAILED"),
    }
    if answered {
        Ok(())
    } else {
        Err(Outcome::NoReply)
    }
}

async fn start_lora(
    spi: peripherals::SPI3<'static>,
    sck: peripherals::GPIO16<'static>,
    mosi: peripherals::GPIO43<'static>,
    miso: peripherals::GPIO17<'static>,
    cs: peripherals::GPIO18<'static>,
) -> Option<SensorLora> {
    let config = spi::master::Config::default()
        .with_frequency(Rate::from_mhz(8))
        .with_mode(spi::Mode::_0);
    let spi = spi::master::Spi::new(spi, config)
        .expect("LoRa SPI failed")
        .with_sck(sck)
        .with_mosi(mosi)
        .with_miso(miso)
        .into_async();
    let cs = Output::new(cs, Level::High, OutputConfig::default());
    let spi = ExclusiveDevice::new(spi, cs, embassy_time::Delay).expect("LoRa CS failed");
    let mut config = Sx127xLoraConfig::for_variant::<Sx1272>();
    config.auto_optimize = true;
    config.use_crc = true;
    match Sx1272Lora::new_with_config(spi, config).await {
        Ok(lora) => {
            info!("[LORA] init_ok");
            Some(lora)
        }
        Err(sx127xlora::driver::Sx127xError::InvalidVersion) => {
            error!("[LORA] init_failed reason=invalid_version");
            None
        }
        Err(sx127xlora::driver::Sx127xError::SPI(_)) => {
            error!("[LORA] init_failed reason=spi");
            None
        }
        Err(_) => {
            error!("[LORA] init_failed reason=configuration");
            None
        }
    }
}

/// Steps and draws the stage for ever, handing each frame to the display core. Touch is read
/// once boot has put the controller in `touch_slot`.
async fn frame_loop(
    mut stage: Stage,
    mut fb_st: SwapThread<'static, SwapState<&'static esp_alloc::EspHeap>>,
) {
    let mut prev_swap_draw = Duration::MIN;
    // The buffer drawn next last held the frame before the current one, so it repaints the
    // current step's damage as well as its own.
    let mut previous_changed = Box::new(Dirty::new_full());
    let mut repaint = Box::new(Dirty::new());
    #[cfg(feature = "damage-debug")]
    let mut outlined = Dirty::new();
    let mut last_touch_poll = Instant::now();
    const TOUCH_REPOLL: Duration = Duration::from_micros(16_667);
    // A write waits for the frame showing its result to reach the panel, since the display
    // freezes while it runs: core 1 has flushed a frame once the swap after the one that
    // handed it over completes.
    let mut pending_write: Option<(settings::Write, u8)> = None;
    // Swaps left until the frame that switched the panel off has reached core 1, and the board
    // can power off.
    let mut power_off_after: Option<u8> = None;
    loop {
        let start = Instant::now();
        {
            let state = fb_st.get();
            let SwapState {
                fb,
                dirty,
                timings,
                drawn,
                brightness,
                display_on,
                #[cfg(feature = "damage-debug")]
                debug_changed,
            } = state;
            let fb = &mut **fb;
            // Keep reading while either tracker holds a contact, or neither sees it lift.
            let in_contact = stage.in_contact();
            if in_contact && last_touch_poll.elapsed() >= TOUCH_REPOLL {
                TOUCH_POLL.signal(());
            }
            let mut wait_timeout = if stage.is_animating() {
                Duration::from_micros(0)
            } else if in_contact {
                TOUCH_REPOLL
            } else {
                Duration::from_millis(250)
            };
            if let Some(due) = stage.next_change() {
                let until = Duration::from_micros(due.saturating_sub(Instant::now().as_micros()));
                wait_timeout = wait_timeout.min(until);
            }
            let (touch_read, sensor_state, motion_state, boot, key, boot_key) = match select4(
                take_touch_read(),
                select4(
                    SENSOR_STATE.wait(),
                    BOOT_REPORTS.receive(),
                    KEY_PRESSES.receive(),
                    BOOT_KEY_PRESSES.receive(),
                ),
                MOTION_STATE.wait(),
                Timer::after(wait_timeout),
            )
            .await
            {
                Either4::First(read) => (Some(read), None, None, None, None, None),
                Either4::Second(Either4::First(state)) => {
                    (None, Some(state), None, None, None, None)
                }
                Either4::Second(Either4::Second(report)) => {
                    (None, None, None, Some(report), None, None)
                }
                Either4::Second(Either4::Third(key)) => (None, None, None, None, Some(key), None),
                Either4::Second(Either4::Fourth(key)) => (None, None, None, None, None, Some(key)),
                Either4::Third(state) => (None, None, Some(state), None, None, None),
                Either4::Fourth(()) => (None, None, None, None, None, None),
            };
            match touch_read {
                Some(Ok(_)) => last_touch_poll = Instant::now(),
                Some(Err(())) => warn!("[TOUCH] read failed"),
                None => {}
            }
            // A stale read found no new report, so the stage hears nothing from it.
            let touch = match touch_read {
                Some(Ok(TouchData::Points(points))) => {
                    let mut positions = [None; 2];
                    for (slot, point) in points.iter().take(2).enumerate() {
                        positions[slot] = Some(Point::new(point.x as i32, point.y as i32));
                    }
                    Some(Touch::Contacts(positions))
                }
                Some(Ok(TouchData::Lifted(point))) => {
                    Some(Touch::Lifted(Point::new(point.x as i32, point.y as i32)))
                }
                Some(Ok(TouchData::CoverGesture)) => Some(Touch::Cover),
                _ => None,
            };
            let update = stage.step(StageInput {
                now: Instant::now().as_micros(),
                touch,
                motion: motion_state,
                sensors: sensor_state.map(|state| {
                    let signal = state.gnss.signal;
                    Sensors {
                        clock: state.clock,
                        zone: state.zone,
                        battery: state.battery,
                        gnss: Gnss {
                            fix: state.gnss.fix.is_some(),
                            in_use: signal.satellites_used.get(),
                            in_view: signal.satellites_in_view.get(),
                            position: state.position,
                        },
                    }
                }),
                boot,
                key: key.map(|key| match key {
                    PowerKey::Short => StageKey::Short,
                    PowerKey::Long => StageKey::Long,
                }),
                boot_key,
            });
            if update.recalibrate {
                info!("[COMPASS] recalibrating on request");
                COMPASS_RECALIBRATE.store(true, Ordering::Relaxed);
            }
            *brightness = update.brightness;
            *display_on = update.display_on;
            if let Some(on) = update.display_on {
                info!("[DISPLAY] panel {=str}", if on { "on" } else { "off" });
            }
            if let Some(choice) = update.store {
                info!("[SETTINGS] chosen {}", choice);
                let write = match choice {
                    Choice::Brightness(level) => settings::Write::Brightness(level),
                    Choice::Timeout(timeout) => settings::Write::Timeout(timeout),
                    Choice::AlwaysOn(on) => settings::Write::AlwaysOn(on),
                    Choice::ManualZone(zone) => {
                        ZONE_CHOICE.signal(ZoneChoice::Manual(zone));
                        settings::Write::ManualZone(zone)
                    }
                    Choice::AutomaticZone => {
                        ZONE_CHOICE.signal(ZoneChoice::Automatic);
                        settings::Write::Automatic
                    }
                    Choice::Clear => {
                        ZONE_CHOICE.signal(ZoneChoice::Cleared);
                        settings::Write::Clear
                    }
                };
                if let Some((earlier, _)) = pending_write.replace((write, 2)) {
                    queue_write(earlier);
                }
            }
            if update.power_off {
                // Nothing more will show, so a write need not wait for its frame.
                if let Some((write, _)) = pending_write.take() {
                    queue_write(write);
                }
                power_off_after = Some(2);
            }
            COMPASS_ACTIVE.store(update.samples_fast, Ordering::Relaxed);
            let changed = stage.changed();
            (*repaint).clone_from(&previous_changed);
            repaint.extend(changed);
            if !*drawn {
                repaint.make_full();
                *drawn = true;
            }
            if repaint.is_full() {
                stage.draw(fb);
            } else if !repaint.is_empty() {
                stage.draw(&mut chrome::Clip::new(fb, &repaint));
            }
            // The panel already shows the step before, so only this step's pixels change on it.
            dirty.clone_from(changed);
            #[cfg(feature = "damage-debug")]
            {
                // Erase the outlines the last flush drew, and outline this step's damage.
                dirty.extend(&outlined);
                outlined.clear();
                for region in dirty.rectangles(chrome::FLUSH_OVERHEAD) {
                    if changed.intersects(&region) {
                        add_outline(&mut outlined, region);
                    }
                }
                debug_changed.clone_from(changed);
            }
            (*previous_changed).clone_from(changed);

            timings.frametime = start.elapsed();
            timings.swap_draw = prev_swap_draw;

            #[cfg(feature = "timing-log")]
            info!(
                "timing draw: frame={}us swap={}us",
                timings.frametime.as_micros(),
                timings.swap_draw.as_micros(),
            );
        }

        let start = Instant::now();
        fb_st.swap().await;
        prev_swap_draw = start.elapsed();
        if let Some((write, swaps)) = pending_write.take() {
            if swaps > 1 {
                pending_write = Some((write, swaps - 1));
            } else {
                queue_write(write);
            }
        }
        match power_off_after {
            Some(swaps) if swaps > 1 => power_off_after = Some(swaps - 1),
            Some(_) => {
                POWER_OFF.signal(());
                power_off_after = None;
            }
            None => {}
        }
    }
}

fn queue_write(write: settings::Write) {
    if SETTINGS_WRITES.try_send(write).is_ok() {
        SETTINGS_QUEUED.fetch_add(1, Ordering::Relaxed);
    } else {
        warn!("[SETTINGS] queue full, {} not saved", write);
    }
}

/// Powers the board off once the settings queued before it are saved. `gnss_task` has parked
/// the GNSS module first.
async fn power_off(power: &mut Axp2101Power<SharedI2cDevice>) {
    let settled = with_timeout(SETTINGS_SETTLE, async {
        while SETTINGS_DONE.load(Ordering::Acquire) != SETTINGS_QUEUED.load(Ordering::Relaxed) {
            Timer::after(Duration::from_millis(10)).await;
        }
    })
    .await;
    if settled.is_err() {
        warn!("[SETTINGS] still saving; powering off anyway");
    }
    info!("[POWER] powering off");
    // The log line gets out before the rails drop.
    Timer::after(Duration::from_millis(50)).await;
    if power.power_off().await.is_err() {
        error!("[POWER] power off failed");
    }
}
