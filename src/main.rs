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
    cell::Cell,
    future::Future,
    sync::atomic::{AtomicBool, Ordering},
};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_futures::{
    join::join,
    select::{Either, Either3, Either4, select, select3, select4},
};
use embassy_sync::{
    blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex},
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
    gpio::{Input, InputConfig, Level, Output, OutputConfig},
    i2c::master::I2c,
    peripherals, spi,
    time::Rate,
    timer::timg::TimerGroup,
};
use defmt::{debug, error, info, warn};
use esp_println as _;
use lc76g::{
    GnssError, GnssOperation, GnssState, Lc76g, LowPowerMode, NmeaOutputRate, NmeaParser,
    NmeaSentence, NmeaUpdate, PairCommandBuilder,
};
use octowhere::{
    board,
    chrome::{self, Color, Dirty, FB},
    drivers::{co5300::Co5300Display, framebuffer::Flush as _, qspi_bus::QspiBus},
    fontdue,
    framebuffer::Framebuffer,
    peripherals::{
        magnetometer::{Bmm350, MagnetometerError},
        power::{Axp2101Error, Axp2101Power},
        rtc::{DateTime as RtcDateTime, Pcf85063aRtc, RtcError},
        touch::{Cst9217, Cst9217Config, Cst9217Error, TouchData},
    },
    settings::{self, Store},
    tz::{self, DATABASE},
    ui::{
        clock::{ClockState, ZoneId, ZoneMode, ZoneState},
        compass::{AxisMap, Calibration, CalibrationEvent, CompassView, Holds, Vec3},
        fusion::Fusion,
        imu::{accel_micro_ms2, gyro_micro_rad_s},
        screens::{Battery, DEFAULT_BRIGHTNESS, Gnss, PeripheralState},
        stage::{Input as StageInput, Motion, Sensors, Stage, Store as Choice, Touch},
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
static SWAP: StaticCell<Swap<SwapState<&esp_alloc::EspHeap>>> = StaticCell::new();
type I2cBus = I2c<'static, esp_hal::Async>;
type SharedI2cDevice = I2cDevice<'static, NoopRawMutex, I2cBus>;
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

static I2C_BUS: StaticCell<Mutex<NoopRawMutex, I2cBus>> = StaticCell::new();
static SENSOR_STATE: Signal<CriticalSectionRawMutex, SensorSnapshot> = Signal::new();
static MOTION_STATE: Signal<CriticalSectionRawMutex, Motion> = Signal::new();
/// Set by the frame loop while the compass screen shows; `motion_task` then samples fast.
static COMPASS_ACTIVE: AtomicBool = AtomicBool::new(false);
static COMPASS_RECALIBRATE: AtomicBool = AtomicBool::new(false);
/// Settings for `settings_task` to save. A full queue drops the newest, which the next change of
/// the same setting supersedes.
static SETTINGS_WRITES: Channel<CriticalSectionRawMutex, settings::Write, 4> = Channel::new();
/// A zone choice from the settings panel, for `sensor_task`, which owns the zone.
static ZONE_CHOICE: Signal<CriticalSectionRawMutex, ZoneChoice> = Signal::new();

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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SensorSnapshot {
    battery_present: bool,
    battery_mv: Option<u16>,
    vbus_mv: Option<u16>,
    vsys_mv: Option<u16>,
    gnss_bytes: u16,
    gnss: GnssState,
    lora_irq: u8,
    clock: ClockState,
    zone: ZoneState,
    /// `None` while the power controller does not answer.
    battery: Option<Battery>,
    /// The last fix's latitude and longitude, kept while there is none.
    position: Option<(i32, i32)>,
}

struct SensorTask {
    power: Option<Axp2101Power<SharedI2cDevice>>,
    lora: Option<SensorLora>,
    lora_dio0: Input<'static>,
    lora_path: LoraPath,
    gnss: Lc76g<SharedI2cDevice, embassy_time::Delay>,
    nmea: [u8; 512],
    nmea_parser: NmeaParser,
    rtc: Option<Pcf85063aRtc<SharedI2cDevice>>,
    state: SensorSnapshot,
    rtc_sync_pending: bool,
    zones: ZoneTracker,
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
        let moved = self.looked_up_at.is_none_or(|(last_latitude, last_longitude)| {
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
            let trusted_field =
                new_field.filter(|field| trusted && !calibration.disturbed(*field));
            if trusted && !calibrated {
                calibrated = true;
                if let Some(accel) = accel {
                    fusion.reset(accel, trusted_field);
                }
            }
            fusion.update(gyro, accel, raw_field, trusted_field, dt);
        }
        let compass = CompassView::new(fusion.attitude(), field, &calibration, &mut holds, now.as_micros());
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

/// Saves settings as they change. Each write holds the display core for its length.
#[embassy_executor::task]
async fn settings_task(mut store: Store) {
    loop {
        let write = SETTINGS_WRITES.receive().await;
        let started = Instant::now();
        let saved = settings::with_display_core_held(|| store.save(write)).await;
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
        mut lora,
        mut lora_dio0,
        mut lora_path,
        mut gnss,
        mut nmea,
        mut nmea_parser,
        mut rtc,
        mut state,
        mut rtc_sync_pending,
        mut zones,
    } = task;
    // Whether GNSS has set the clock since the firmware started.
    let mut clock_set = false;
    #[cfg(feature = "gnss-raw-log")]
    let mut raw_nmea_line = [0; 256];
    #[cfg(feature = "gnss-raw-log")]
    let mut raw_nmea_line_len = 0;

    #[cfg(feature = "lora-link-tx")]
    if let Some(lora) = &mut lora {
        lora.map_dio0::<TxDone>().await.unwrap();
    }
    #[cfg(feature = "lora-link-rx")]
    if let Some(lora) = &mut lora {
        lora.map_dio0::<RxDone>().await.unwrap();
    }

    #[cfg(feature = "lora-link-tx")]
    let mut lora_sequence = 0u32;

    loop {
        Timer::after(Duration::from_millis(250)).await;

        if let Some(choice) = ZONE_CHOICE.try_take() {
            zones.choose(choice);
        }
        if let Some(power) = &mut power {
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
            state.battery_present = battery_present;
            state.battery_mv = battery_present.then_some(battery_mv).flatten();
            state.vbus_mv = vbus_mv;
            state.vsys_mv = vsys_mv;
            debug!(
                "[PMIC] sample battery_present={} VBAT={}mV VBUS={}mV VSYS={}mV",
                battery_present, battery_mv, vbus_mv, vsys_mv
            );
        }

        if let Some(lora) = &mut lora
            && let Ok(irq) = lora.read(IRQ_FLAGS).await
        {
            state.lora_irq = irq;
            debug!("[LORA] sample IRQ={=u8:#04x}", irq);
        }
        match gnss.read_nmea_chunk(&mut nmea).await {
            Ok(data) => {
                state.gnss_bytes = data.len() as u16;
                #[cfg(feature = "gnss-raw-log")]
                log_raw_nmea(data, &mut raw_nmea_line, &mut raw_nmea_line_len);
                let mut updates = 0;
                for &byte in data.iter() {
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
                            debug!("[GNSS] NMEA_OUTPUT_RATE sentence={} rate={}", sentence, rate);
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
                state.gnss = nmea_parser.state();
                let signal = state.gnss.signal;
                debug!(
                    "[GNSS] acquisition in_view={} with_signal={} used={} strongest_snr_db={} fix_type={} hdop_milli={}",
                    signal.satellites_in_view.get(),
                    signal.satellites_with_signal.get(),
                    signal.satellites_used.get(),
                    signal.strongest_snr.map(|value| value.get()),
                    signal.fix_type,
                    signal.hdop.map(|value| value.get()),
                );
                if let Some(fix) = state.gnss.fix {
                    debug!(
                        "[GNSS] sample bytes={} updates={} lat={} lon={} alt_mm={} sats={} hdop_milli={}",
                        data.len(),
                        updates,
                        fix.latitude.get(),
                        fix.longitude.get(),
                        fix.altitude.map(|value| value.get()),
                        fix.satellites,
                        fix.hdop.map(|value| value.get()),
                    );
                } else {
                    debug!(
                        "[GNSS] sample bytes={} updates={} no_fix",
                        data.len(),
                        updates
                    );
                }
            }
            Err(GnssError::I2c { operation, error }) => {
                warn!("[GNSS] I2C_FAILED operation={} error={}", operation, error);
            }
            Err(GnssError::BufferTooSmall { .. }) => warn!("[GNSS] NMEA_BUFFER_TOO_SMALL"),
            Err(GnssError::PairCommand(_)) => warn!("[GNSS] PAIR_COMMAND_BUILD_FAILED"),
        }
        if state.gnss.utc.is_none() {
            rtc_sync_pending = true;
        } else if rtc_sync_pending
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
        }
        if let Some(fix) = state.gnss.fix
            && let Some(zone) = zones.follow(fix.latitude.get(), fix.longitude.get()).await
            && SETTINGS_WRITES.try_send(settings::Write::AutomaticZone(zone)).is_err()
        {
            warn!("[SETTINGS] queue full, automatic zone not saved");
        }
        state.zone = zones.state();

        #[cfg(feature = "lora-link-tx")]
        if let Some(lora) = &mut lora {
            let payload = [
                b'O',
                b'W',
                b'L',
                b'K',
                (lora_sequence >> 24) as u8,
                (lora_sequence >> 16) as u8,
                (lora_sequence >> 8) as u8,
                lora_sequence as u8,
            ];
            lora_sequence = lora_sequence.wrapping_add(1);

            if lora_path.transmit().await.is_err() {
                warn!("[LORA] LINK_TX_SWITCH_FAILED");
            } else if lora.tx(&payload).await.is_err() {
                warn!("[LORA] LINK_TX_FAILED");
            } else {
                match select(
                    lora_dio0.wait_for_rising_edge(),
                    Timer::after(Duration::from_secs(2)),
                )
                .await
                {
                    Either::First(_) => {
                        info!("[LORA] LINK_TX_DONE sequence={}", lora_sequence - 1);
                        let _ = lora.clear_interrupt::<TxDone>().await;
                    }
                    Either::Second(_) => warn!("[LORA] LINK_TX_TIMEOUT"),
                }
                let _ = lora_path.receive().await;
            }
        }

        #[cfg(feature = "lora-link-rx")]
        if let Some(lora) = &mut lora {
            info!("[LORA] LINK_RX_START");
            if lora_path.receive().await.is_err() {
                warn!("[LORA] LINK_RX_SWITCH_FAILED");
            } else if lora.rx(None).await.is_err() {
                warn!("[LORA] LINK_RX_START_FAILED");
            } else {
                let mut received = false;
                for _ in 0..100 {
                    if lora.interrupt_flag::<RxDone>().await.unwrap_or(false) {
                        received = true;
                        break;
                    }
                    Timer::after(Duration::from_millis(20)).await;
                }
                if received {
                    match lora.rx_packet().await {
                        Ok(packet) => info!(
                            "[LORA] LINK_RX_OK len={} rssi={} snr_raw={} payload={}",
                            packet.length,
                            packet.rssi,
                            packet.snr_raw,
                            packet.payload(),
                        ),
                        Err(_) => warn!("[LORA] LINK_RX_PACKET_FAILED"),
                    }
                } else {
                    warn!("[LORA] LINK_RX_TIMEOUT");
                }
                let _ = lora.clear_all_interrupts().await;
            }
        }

        SENSOR_STATE.signal(state);
    }
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

fn draw_if_in_bounds<C, D, T>(target: &mut D, dirty: &mut Dirty, thing: T) -> Result<(), D::Error>
where
    T: Drawable<Color = C>,
    T: Dimensions,
    D: DrawTarget<Color = C>,
    D::Error: core::fmt::Debug,
{
    let bbox = thing.bounding_box();
    if bbox.intersection(&target.bounding_box()).is_zero_sized() {
        return Ok(());
    }
    thing.draw(target)?;
    dirty.add(bbox);
    Ok(())
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
const GNSS_DEADLINE: Duration = Duration::from_millis(1500);

/// Each part's outcome as boot decides it, for the self-test.
static BOOT_REPORTS: Channel<CriticalSectionRawMutex, Report, 6> = Channel::new();

/// Runs one part's bring-up against its deadline and reports how it ended. Returns whether the
/// part answered.
async fn probe(part: Part, deadline: Duration, bring_up: impl Future<Output = Result<(), Outcome>>) -> bool {
    let started = Instant::now();
    let outcome = match with_timeout(deadline, bring_up).await {
        Ok(Ok(())) => Outcome::Answered,
        Ok(Err(outcome)) => outcome,
        Err(TimeoutError) => Outcome::NoReply,
    };
    info!("[BOOT] {} {} in {}ms", part, outcome, started.elapsed().as_millis());
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
}

#[embassy_executor::task]
async fn async_main(spawner: Spawner) {
    // esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 72 * 1024);
    esp_alloc::heap_allocator!(size: 240 * 1024);

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
        saved.always_on,
    );
    spawner.spawn(settings_task(store).unwrap());

    // The panel comes up first, so the self-test shows while the parts come up behind it.
    start_display_core!(peripherals, fb_st);

    let stage = Stage::starting(PeripheralState {
        brightness: saved.brightness.unwrap_or(DEFAULT_BRIGHTNESS),
        timeout: saved.timeout.unwrap_or_default(),
        always_on: saved.always_on.unwrap_or(false),
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
    };
    let zones = ZoneTracker {
        mode: saved.zone_mode,
        manual: saved.manual_zone,
        automatic: saved.automatic_zone,
        looked_up_at: None,
    };
    let touch = Cell::new(None);
    join(bring_up(spawner, parts, zones, &touch), frame_loop(stage, fb_st, &touch)).await;
}

/// Brings up every part behind the self-test, each against its deadline, then starts the tasks
/// that own them and hands the touch controller to the frame loop. A part that fails is left
/// out, and its owner runs without it.
async fn bring_up(spawner: Spawner, parts: Parts, zones: ZoneTracker, touch_slot: &Cell<Option<TouchDriver>>) {
    // The I²C pull-ups share VCC3V3 with the secondary board.
    Timer::after(Duration::from_millis(board::I2C_POWER_SETTLE_MS)).await;

    let i2c = I2c::new(
        parts.i2c,
        esp_hal::i2c::master::Config::default().with_frequency(Rate::from_hz(board::I2C_FREQ_HZ)),
    )
    .expect("I2C failed")
    .with_scl(parts.scl)
    .with_sda(parts.sda)
    .into_async();
    let i2c = I2C_BUS.init(Mutex::<NoopRawMutex, _>::new(i2c));
    let i2c = I2cDevice::new(i2c);

    let mut power = Axp2101Power::new(i2c.clone());
    let answered = probe(Part::Power, POWER_DEADLINE, async {
        power.init().await.map_err(|error| match error {
            Axp2101Error::I2c(_) => Outcome::NoReply,
            Axp2101Error::WrongChipId(_) => Outcome::BadReply,
        })
    })
    .await;
    let mut power = answered.then_some(power);
    let mut initial_sensor_state = SensorSnapshot::default();
    if let Some(power) = &mut power {
        Timer::after(Duration::from_millis(100)).await;
        let battery_present = power.is_battery_present().await.unwrap_or(false);
        initial_sensor_state.battery_present = battery_present;
        if battery_present {
            initial_sensor_state.battery_mv = power.get_battery_voltage().await.ok();
        }
        initial_sensor_state.vbus_mv = power.get_vbus_voltage().await.ok();
        initial_sensor_state.vsys_mv = power.get_system_voltage().await.ok();
    }

    if reset_radios(i2c.clone()).await.is_err() {
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
                    Cst9217Error::IDMismatch | Cst9217Error::NoFirmware | Cst9217Error::InvalidCheckcode => {
                        Outcome::BadReply
                    }
                })
            })
            .await;
            let imu_ok = probe(Part::Motion, MOTION_DEADLINE, async {
                let outcome = |error| match error {
                    ph_qmi8658::Error::Bus | ph_qmi8658::Error::NotPresent => Outcome::NoReply,
                    _ => Outcome::BadReply,
                };
                imu.init(&mut embassy_time::Delay).await.map_err(outcome)?;
                imu.set_mode_with_delay(&mut embassy_time::Delay, ph_qmi8658::OperatingMode::AccelGyroOnly)
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
                magnetometer.start_normal_mode().await.map_err(|_| Outcome::NoReply)?;
                Timer::after(Duration::from_millis(15)).await;
                if !magnetometer.data_ready().await.map_err(|_| Outcome::NoReply)? {
                    return Err(Outcome::BadReply);
                }
                let data = magnetometer.read_data().await.map_err(|_| Outcome::NoReply)?;
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
    let gnss_ok = probe(Part::Gnss, GNSS_DEADLINE, configure_gnss(&mut gnss, &mut nmea_parser)).await;
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

    let lora = start_lora(parts.lora_spi, parts.lora_sck, parts.lora_mosi, parts.lora_miso, parts.lora_cs).await;
    let lora_dio0 = Input::new(parts.lora_dio0, InputConfig::default());
    let lora_path = LoraPath::new(
        i2c.clone(),
        !((1 << board::EXIO_LORA_RESET) | (1 << board::EXIO_LORA_TX_SWITCH)),
    );

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
        touch_slot.set(Some(touch));
    }

    spawner.spawn(
        sensor_task(SensorTask {
            power,
            lora,
            lora_dio0,
            lora_path,
            gnss,
            nmea: [0; 512],
            nmea_parser,
            rtc: rtc_ok.then_some(rtc),
            state: initial_sensor_state,
            rtc_sync_pending: true,
            zones,
        })
        .unwrap(),
    );
    // The compass needs the IMU for its tilt. Without it, it shows NO DATA.
    if imu_ok {
        spawner.spawn(
            motion_task(MotionTask {
                magnetometer: magnetometer_ok.then_some(magnetometer),
                imu,
                accel_lsb_per_g: ph_qmi8658::accel_lsb_per_g(accel_range),
                gyro_lsb_per_dps: ph_qmi8658::gyro_lsb_per_dps(gyro_range),
            })
            .unwrap(),
        );
    }
    info!("[MEM] internal_used={} psram_used={}", esp_alloc::HEAP.used(), PSRAM_HEAP.used());
}

/// Holds the GNSS module and the radio in reset, then releases them with the radio listening.
async fn reset_radios(i2c: SharedI2cDevice) -> Result<(), ()> {
    let mut exio = Tca9554::new(i2c, tca9554::Address::standard());
    let gps_reset = 1 << board::EXIO_GPS_RESET;
    let lora_reset = 1 << board::EXIO_LORA_RESET;
    let lora_rx_switch = 1 << board::EXIO_LORA_RX_SWITCH;
    let lora_tx_switch = 1 << board::EXIO_LORA_TX_SWITCH;
    let fail = |_| ();
    exio.init().await.map_err(fail)?;
    exio.write_output(u8::MAX).await.map_err(fail)?;
    let output_mask = gps_reset | lora_reset | lora_rx_switch | lora_tx_switch;
    exio.write_direction(!output_mask).await.map_err(fail)?;
    info!("[TCA9554] OK");
    exio.write_output(!(gps_reset | lora_reset | lora_tx_switch)).await.map_err(fail)?;
    Timer::after(Duration::from_millis(10)).await;
    exio.write_output(!lora_tx_switch).await.map_err(fail)?;
    Timer::after(Duration::from_micros(200)).await;
    exio.write_output(!(lora_reset | lora_tx_switch)).await.map_err(fail)?;
    exio.write_direction(!(lora_reset | lora_rx_switch | lora_tx_switch)).await.map_err(fail)?;
    Timer::after(Duration::from_millis(10)).await;
    exio.write_output(!(lora_reset | lora_tx_switch)).await.map_err(fail)?;
    let direction = exio.read_direction().await.map_err(fail)?;
    debug!("[TCA9554] direction={=u8:#04x}", direction);
    Ok(())
}

/// Sends the receiver its configuration and reads what it has sent. It answers if any command
/// is accepted or a read succeeds.
async fn configure_gnss(
    gnss: &mut Lc76g<SharedI2cDevice, embassy_time::Delay>,
    nmea_parser: &mut NmeaParser,
) -> Result<(), Outcome> {
    let mut answered = false;
    debug!("[GNSS] STARTUP PAIR_SEND command=732 mode={}", GNSS_LOW_POWER_MODE);
    match gnss.set_low_power_mode(GNSS_LOW_POWER_MODE).await {
        Ok(()) => answered = true,
        Err(error) => warn!("[GNSS] STARTUP PAIR_SEND_RESULT command=732 status=error error={}", error),
    }
    for sentence in [NmeaSentence::Gsa, NmeaSentence::Gsv] {
        match gnss.set_nmea_output_rate(sentence, NmeaOutputRate::EVERY_FIX).await {
            Ok(()) => answered = true,
            Err(error) => warn!("[GNSS] STARTUP PAIR_SEND_RESULT command=062 status=error error={}", error),
        }
        match gnss.query_nmea_output_rate(sentence).await {
            Ok(()) => answered = true,
            Err(error) => warn!("[GNSS] STARTUP PAIR_SEND_RESULT command=063 status=error error={}", error),
        }
    }
    if let Ok(command) = PairCommandBuilder::new(67).and_then(|builder| builder.finish()) {
        match gnss.send_pair_command(&command).await {
            Ok(()) => answered = true,
            Err(error) => warn!("[GNSS] STARTUP PAIR_SEND_RESULT command=067 status=error error={}", error),
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
    if answered { Ok(()) } else { Err(Outcome::NoReply) }
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
/// Times the band's row writes alone, before core 1 flushes anything: a fill, and painting
/// coverage a pixel at a time, two pixels a word, and in runs.
#[cfg(feature = "fault-draw-bench")]
fn row_write_bench() {
    use octowhere::chrome::RgbColorExt as _;
    const W: usize = 466;
    let (color, background) = (chrome::RED, chrome::BLACK);
    let raw = |c: chrome::Color| embedded_graphics::pixelcolor::raw::RawU16::from(c).into_inner().to_be_bytes();
    let (full, empty) = (raw(color), raw(background));
    let pixel = |covered: u8| match covered {
        0 => empty,
        u8::MAX => full,
        _ => raw(background.lerp(&color, covered)),
    };
    let mut text = [0u8; W];
    let pattern: &[u8] = &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 60, 190, 255, 255, 255, 255, 255, 255, 255, 255, 190, 60];
    for (i, c) in text.iter_mut().enumerate() {
        *c = pattern[i % pattern.len()];
    }
    let patterns: [(&str, [u8; W]); 3] = [("zeros", [0; W]), ("full", [u8::MAX; W]), ("text", text)];
    let mut fb = FB::alloc(&PSRAM_HEAP);
    let rows = 198..318usize;
    let time = |f: &mut dyn FnMut()| {
        let mut best = u64::MAX;
        for _ in 0..5 {
            let t = Instant::now();
            f();
            best = best.min(t.elapsed().as_micros());
        }
        best
    };
    let fill = time(&mut || {
        let area = embedded_graphics::primitives::Rectangle::new(Point::new(0, 198), Size::new(W as u32, 120));
        let _ = fb.fill_solid(&area, background);
    });
    info!("[BENCH] rows fill={}", fill);
    for (name, coverage) in &patterns {
        let per_pixel = time(&mut || {
            for y in rows.clone() {
                let pixels = &mut fb.buffer_mut()[y * W * 2..(y + 1) * W * 2];
                for (bytes, &c) in pixels.as_chunks_mut::<2>().0.iter_mut().zip(coverage) {
                    *bytes = pixel(c);
                }
            }
        });
        let pairs = time(&mut || {
            for y in rows.clone() {
                let pixels = &mut fb.buffer_mut()[y * W * 2..(y + 1) * W * 2];
                let (_, words, _) = unsafe { pixels.align_to_mut::<u32>() };
                for (word, pair) in words.iter_mut().zip(coverage.as_chunks::<2>().0) {
                    let ([a, b], [c, d]) = (pixel(pair[0]), pixel(pair[1]));
                    *word = u32::from_ne_bytes([a, b, c, d]);
                }
            }
        });
        let runs = time(&mut || {
            for y in rows.clone() {
                let pixels = &mut fb.buffer_mut()[y * W * 2..(y + 1) * W * 2];
                let mut i = 0;
                while i < W {
                    let c = coverage[i];
                    if c == 0 || c == u8::MAX {
                        let run = coverage[i..].iter().position(|&v| v != c).unwrap_or(W - i);
                        let fill = if c == 0 { empty } else { full };
                        let bytes = &mut pixels[i * 2..(i + run) * 2];
                        let (head, words, tail) = unsafe { bytes.align_to_mut::<u32>() };
                        for end in [head, tail] {
                            end.as_chunks_mut::<2>().0.fill(fill);
                        }
                        words.fill(u32::from_ne_bytes([fill[0], fill[1], fill[0], fill[1]]));
                        i += run;
                    } else {
                        pixels[i * 2..i * 2 + 2].copy_from_slice(&pixel(c));
                        i += 1;
                    }
                }
            }
        });
        let quads = time(&mut || {
            let (empty2, full2) = (
                u32::from_ne_bytes([empty[0], empty[1], empty[0], empty[1]]),
                u32::from_ne_bytes([full[0], full[1], full[0], full[1]]),
            );
            for y in rows.clone() {
                let pixels = &mut fb.buffer_mut()[y * W * 2..(y + 1) * W * 2];
                let (_, words, _) = unsafe { pixels.align_to_mut::<u32>() };
                let (quads, _) = coverage.as_chunks::<4>();
                let (word_pairs, _) = words.as_chunks_mut::<2>();
                for (out, quad) in word_pairs.iter_mut().zip(quads) {
                    match u32::from_ne_bytes(*quad) {
                        0 => *out = [empty2; 2],
                        u32::MAX => *out = [full2; 2],
                        _ => {
                            let ([a, b], [c, d], [e, f], [g, h]) = (pixel(quad[0]), pixel(quad[1]), pixel(quad[2]), pixel(quad[3]));
                            *out = [u32::from_ne_bytes([a, b, c, d]), u32::from_ne_bytes([e, f, g, h])];
                        }
                    }
                }
            }
        });
        let octs = time(&mut || {
            let (empty2, full2) = (
                u32::from_ne_bytes([empty[0], empty[1], empty[0], empty[1]]),
                u32::from_ne_bytes([full[0], full[1], full[0], full[1]]),
            );
            for y in rows.clone() {
                let pixels = &mut fb.buffer_mut()[y * W * 2..(y + 1) * W * 2];
                let (_, words, _) = unsafe { pixels.align_to_mut::<u32>() };
                let (eights, _) = coverage.as_chunks::<8>();
                let (word_quads, _) = words.as_chunks_mut::<4>();
                for (out, eight) in word_quads.iter_mut().zip(eights) {
                    match u64::from_ne_bytes(*eight) {
                        0 => *out = [empty2; 4],
                        u64::MAX => *out = [full2; 4],
                        _ => {
                            for (word, pair) in out.iter_mut().zip(eight.as_chunks::<2>().0) {
                                let ([a, b], [c, d]) = (pixel(pair[0]), pixel(pair[1]));
                                *word = u32::from_ne_bytes([a, b, c, d]);
                            }
                        }
                    }
                }
            }
        });
        info!("[BENCH] rows {=str} per_pixel={} pairs={} runs={} quads={} octs={}", name, per_pixel, pairs, runs, quads, octs);
    }
}

async fn frame_loop(
    mut stage: Stage,
    mut fb_st: SwapThread<'static, SwapState<&'static esp_alloc::EspHeap>>,
    touch_slot: &Cell<Option<TouchDriver>>,
) {
    let mut touch: Option<TouchDriver> = None;
    // The last report the controller wrote, which a stale read repeats.
    let mut touch_data = TouchData::default();
    let mut last_report = Instant::now();
    /// A contact with no fresh report for this long has ended without its lift report. Held
    /// fingers were reported at most 101 ms apart.
    const LIFT_WITHOUT_REPORT: Duration = Duration::from_millis(300);
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
    #[cfg(feature = "fault-draw-bench")]
    // After the reset's USB reconnect, whose first seconds of log are lost.
    #[cfg(feature = "fault-draw-bench")]
    {
        Timer::after(Duration::from_secs(3)).await;
        row_write_bench();
    }
    let (mut bench_part, mut bench_last, mut bench_started) = {
        octowhere_ui::part_timing::install(|| Instant::now().as_micros() as u32);
        (0usize, Instant::now(), false)
    };
    loop {
        if touch.is_none() {
            touch = touch_slot.take();
        }
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
            let touch_repoll_due =
                touch.is_some() && in_contact && last_touch_poll.elapsed() >= TOUCH_REPOLL;
            let mut wait_timeout = if touch_repoll_due || stage.is_animating() {
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
            let touch_wait = async {
                match touch.as_mut() {
                    Some(touch) => touch.wait_for_touch().await.is_ok(),
                    None => core::future::pending().await,
                }
            };
            let (touch_ready, sensor_state, motion_state, boot) = match select4(
                touch_wait,
                select(SENSOR_STATE.wait(), BOOT_REPORTS.receive()),
                MOTION_STATE.wait(),
                Timer::after(wait_timeout),
            )
            .await
            {
                Either4::First(ready) => (ready, None, None, None),
                Either4::Second(Either::First(state)) => (false, Some(state), None, None),
                Either4::Second(Either::Second(report)) => (false, None, None, Some(report)),
                Either4::Third(state) => (false, None, Some(state), None),
                Either4::Fourth(()) => (touch_repoll_due, None, None, None),
            };
            let touch_ready = touch_ready
                && match touch.as_mut().map(|touch| touch.read_touch_data()) {
                    Some(read) => match read.await {
                        Ok(TouchData::Stale) if last_report.elapsed() < LIFT_WITHOUT_REPORT => true,
                        Ok(TouchData::Stale) => {
                            touch_data = TouchData::default();
                            true
                        }
                        Ok(fresh) => {
                            touch_data = fresh;
                            last_report = Instant::now();
                            true
                        }
                        Err(_) => {
                            warn!("[TOUCH] read failed");
                            false
                        }
                    },
                    None => false,
                };
            if touch_ready {
                last_touch_poll = Instant::now();
            }
            #[cfg(feature = "fault-draw-bench")]
            if !stage.starting_up() {
                let part = octowhere::ui::startup::Part::ALL[bench_part % 6];
                bench_part += 1;
                info!("[BENCH] demo {}", part);
                stage.bench_fault(part, Instant::now().as_micros());
                bench_started = true;
            }
            #[cfg(feature = "fault-draw-bench")]
            let step_start = Instant::now();
            let update = stage.step(StageInput {
                now: Instant::now().as_micros(),
                touch: touch_ready.then(|| match &touch_data {
                    TouchData::Points(points) => {
                        let mut positions = [None; 2];
                        for (slot, point) in points.iter().take(2).enumerate() {
                            positions[slot] = Some(Point::new(point.x as i32, point.y as i32));
                        }
                        Touch::Contacts(positions)
                    }
                    TouchData::CoverGesture => Touch::Cover,
                    TouchData::Stale => unreachable!("a stale read keeps the last report"),
                }),
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
            COMPASS_ACTIVE.store(update.samples_fast, Ordering::Relaxed);
            #[cfg(feature = "fault-draw-bench")]
            if core::mem::take(&mut bench_started) {
                stage.bench_mark_full();
            }
            #[cfg(feature = "fault-draw-bench")]
            let step_us = step_start.elapsed().as_micros();
            let changed = stage.changed();
            (*repaint).clone_from(&previous_changed);
            repaint.extend(changed);
            if !*drawn {
                repaint.make_full();
                *drawn = true;
            }
            #[cfg(feature = "fault-draw-bench")]
            let draw_start = Instant::now();
            if repaint.is_full() {
                stage.draw(fb);
            } else if !repaint.is_empty() {
                stage.draw(&mut chrome::Clip::new(fb, &repaint));
            }
            #[cfg(feature = "fault-draw-bench")]
            if let Some(octowhere::ui::startup::View::Fault(frame)) = stage.bench_startup_view() {
                let draw_us = draw_start.elapsed().as_micros();
                let p = octowhere_ui::part_timing::take();
                let period = bench_last.elapsed().as_micros();
                bench_last = start;
                info!(
                    "[BENCH] frame={} period={} step={} draw={} swap={} flush={} vsync={} clear={} field={} dashes={} parts={} band={} name={} strip={} line={} micro={} hatch={} reason={} barcode={} name_raster={} line_raster={} ko_gather={} ko_combine={} ko_write={}",
                    frame,
                    period,
                    step_us,
                    draw_us,
                    prev_swap_draw.as_micros(),
                    timings.spi_time.as_micros(),
                    timings.vsync_wait.as_micros(),
                    p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7], p[8], p[9], p[10], p[11], p[12], p[13], p[14], p[15], p[16],
                );
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
    }
}


fn queue_write(write: settings::Write) {
    if SETTINGS_WRITES.try_send(write).is_err() {
        warn!("[SETTINGS] queue full, {} not saved", write);
    }
}
