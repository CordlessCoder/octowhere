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
use core::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_futures::select::{Either, Either3, Either4, select, select3, select4};
use embassy_sync::{
    blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex},
    channel::Channel,
    mutex::Mutex,
    signal::Signal,
};
use embassy_time::{Duration, Instant, Timer};
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
        magnetometer::Bmm350,
        power::Axp2101Power,
        rtc::{DateTime as RtcDateTime, Pcf85063aRtc},
        touch::{Cst9217, Cst9217Config, TouchData},
    },
    settings::{self, Store},
    tz::{self, DATABASE},
    ui::{
        clock::{ClockState, ZoneId, ZoneMode, ZoneState},
        compass::{AxisMap, Calibration, CalibrationEvent, CompassView, Vec3},
        fusion::Fusion,
        imu::{accel_micro_ms2, gyro_micro_rad_s},
        screens::{Battery, DEFAULT_BRIGHTNESS, Gnss, PeripheralState},
        stage::{Input as StageInput, Motion, Sensors, Stage, Store as Choice, Touch},
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
/// A display level for the display core to apply before its next flush, or `NO_BRIGHTNESS`.
static BRIGHTNESS: AtomicU16 = AtomicU16::new(NO_BRIGHTNESS);
const NO_BRIGHTNESS: u16 = u16::MAX;

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
    power: Axp2101Power<SharedI2cDevice>,
    lora: SensorLora,
    lora_dio0: Input<'static>,
    lora_path: LoraPath,
    gnss: Lc76g<SharedI2cDevice, embassy_time::Delay>,
    nmea: [u8; 512],
    nmea_parser: NmeaParser,
    rtc: Pcf85063aRtc<SharedI2cDevice>,
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
                    #[cfg(feature = "damage-debug")]
                    debug_changed: Dirty::new(),
                    timings: Timings::default(),
                },
                SwapState {
                    fb: FB::alloc(&PSRAM_HEAP),
                    dirty: Dirty::new(),
                    drawn: false,
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
    let level = match BRIGHTNESS.swap(NO_BRIGHTNESS, Ordering::Relaxed) {
        NO_BRIGHTNESS => DEFAULT_BRIGHTNESS,
        level => level as u8,
    };
    display
        .set_brightness(level)
        .expect("brightness command failed");

    info!("[DISPLAY] OK");

    let mut prev_swap_spi = Duration::MIN;
    let mut first_flush = true;
    loop {
        settings::hold_display_core_if_asked();
        let level = BRIGHTNESS.swap(NO_BRIGHTNESS, Ordering::Relaxed);
        if level != NO_BRIGHTNESS && display.set_brightness(level as u8).is_err() {
            warn!("[DISPLAY] brightness command failed");
        }
        let state = swap.get();
        let SwapState {
            fb,
            timings,
            dirty,
            drawn: _,
            #[cfg(feature = "damage-debug")]
            debug_changed,
        } = state;

        let start = Instant::now();

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
                    Timer::after(Duration::from_millis(17)),
                )
                .await
                {
                    Either::First(()) => {}
                    Either::Second(()) => warn!("[DISPLAY] te_timeout"),
                }
            }
            timings.vsync_wait = start.elapsed();
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
        let compass = CompassView::new(fusion.attitude(), field, &calibration);
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
    lora.map_dio0::<TxDone>().await.unwrap();
    #[cfg(feature = "lora-link-rx")]
    lora.map_dio0::<RxDone>().await.unwrap();

    #[cfg(feature = "lora-link-tx")]
    let mut lora_sequence = 0u32;

    loop {
        Timer::after(Duration::from_millis(250)).await;

        if let Some(choice) = ZONE_CHOICE.try_take() {
            zones.choose(choice);
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
        state.battery_present = battery_present;
        state.battery_mv = battery_present.then_some(battery_mv).flatten();
        state.vbus_mv = vbus_mv;
        state.vsys_mv = vsys_mv;
        debug!(
            "[PMIC] sample battery_present={} VBAT={}mV VBUS={}mV VSYS={}mV",
            battery_present, battery_mv, vbus_mv, vsys_mv
        );

        if let Ok(irq) = lora.read(IRQ_FLAGS).await {
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
        } else if rtc_sync_pending && let Some(utc) = state.gnss.utc {
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
        let utc = rtc.get_time().await.ok().map(|time| {
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
            stopped: rtc.oscillator_stopped(),
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
        {
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
        {
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
    /// The step's own damage, which the display core outlines.
    #[cfg(feature = "damage-debug")]
    debug_changed: Dirty,
    timings: Timings,
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

    // The I²C pull-ups share VCC3V3 with the secondary board.
    Timer::after(Duration::from_millis(board::I2C_POWER_SETTLE_MS)).await;

    let i2c = I2c::new(
        peripherals.I2C0,
        esp_hal::i2c::master::Config::default().with_frequency(Rate::from_hz(board::I2C_FREQ_HZ)),
    )
    .expect("I2C failed")
    .with_scl(peripherals.GPIO14)
    .with_sda(peripherals.GPIO15)
    .into_async();
    let i2c = I2C_BUS.init(Mutex::<NoopRawMutex, _>::new(i2c));
    let i2c = I2cDevice::new(i2c);

    let gyro_range = ph_qmi8658::GyroRange::Dps512;
    let accel_range = ph_qmi8658::AccelRange::G2;
    let accel_lsb_per_g = ph_qmi8658::accel_lsb_per_g(accel_range);
    let gyro_lsb_per_dps = ph_qmi8658::gyro_lsb_per_dps(gyro_range);

    let mut power = Axp2101Power::new(i2c.clone());
    power.init().await.unwrap();
    Timer::after(Duration::from_millis(100)).await;
    let pmic_readings = (
        power.get_battery_voltage().await,
        power.get_vbus_voltage().await,
        power.get_system_voltage().await,
    );
    let battery_present = power.is_battery_present().await.unwrap_or(false);

    let mut exio = Tca9554::new(i2c.clone(), tca9554::Address::standard());

    debug!("[TCA9554] init_start");
    exio.init().await.unwrap();
    exio.write_output(u8::MAX).await.unwrap();
    let exio_output_mask = (1 << board::EXIO_GPS_RESET)
        | (1 << board::EXIO_LORA_RESET)
        | (1 << board::EXIO_LORA_RX_SWITCH)
        | (1 << board::EXIO_LORA_TX_SWITCH);
    exio.write_direction(!exio_output_mask).await.unwrap();
    info!("[TCA9554] OK");

    let gps_reset = 1 << board::EXIO_GPS_RESET;
    let lora_reset = 1 << board::EXIO_LORA_RESET;
    let lora_rx_switch = 1 << board::EXIO_LORA_RX_SWITCH;
    let lora_tx_switch = 1 << board::EXIO_LORA_TX_SWITCH;

    debug!("[GNSS] STARTUP RESET_SEQUENCE_BEGIN");
    exio.write_output(!(gps_reset | lora_reset | lora_tx_switch))
        .await
        .unwrap();
    debug!("[GNSS] STARTUP RESET_OUTPUT phase=initial value={=u8:#04x}",
        !(gps_reset | lora_reset | lora_tx_switch)
    );
    Timer::after(Duration::from_millis(10)).await;
    exio.write_output(!lora_tx_switch)
        .await
        .unwrap();
    debug!("[GNSS] STARTUP RESET_OUTPUT phase=release_gps_pulse_lora value={=u8:#04x}",
        !lora_tx_switch
    );
    Timer::after(Duration::from_micros(200)).await;
    exio.write_output(!(lora_reset | lora_tx_switch))
        .await
        .unwrap();
    debug!("[GNSS] STARTUP RESET_OUTPUT phase=release_lora value={=u8:#04x}",
        !(lora_reset | lora_tx_switch)
    );
    exio.write_direction(!(lora_reset | lora_rx_switch | lora_tx_switch))
        .await
        .unwrap();
    debug!("[GNSS] STARTUP RESET_DIRECTION value={=u8:#04x}",
        !(lora_reset | lora_rx_switch | lora_tx_switch)
    );
    Timer::after(Duration::from_millis(10)).await;
    exio.write_output(!(lora_reset | lora_tx_switch)).await.unwrap();
    debug!("[GNSS] STARTUP RESET_OUTPUT phase=select_rx value={=u8:#04x}",
        !(lora_reset | lora_tx_switch)
    );
    let exio_direction = exio.read_direction().await.unwrap();
    debug!("[TCA9554] direction={=u8:#04x}", exio_direction);
    {
        debug!("[GNSS] STARTUP settle_begin");
        Timer::after(Duration::from_secs(1)).await;
        debug!("[GNSS] STARTUP settle_complete");
    }
    drop(exio);

    let mut gnss = Lc76g::new(i2c.clone(), embassy_time::Delay);
    debug!("[GNSS] STARTUP PAIR_SEND command=732 mode={}", GNSS_LOW_POWER_MODE);
    match gnss.set_low_power_mode(GNSS_LOW_POWER_MODE).await {
        Ok(()) => debug!("[GNSS] STARTUP PAIR_SEND_RESULT command=732 status=accepted"),
        Err(error) => warn!("[GNSS] STARTUP PAIR_SEND_RESULT command=732 status=error error={}", error),
    }
    for sentence in [NmeaSentence::Gsa, NmeaSentence::Gsv] {
        debug!("[GNSS] STARTUP PAIR_SEND command=062 sentence={} rate=1", sentence);
        match gnss
            .set_nmea_output_rate(sentence, NmeaOutputRate::EVERY_FIX)
            .await
        {
            Ok(()) => debug!("[GNSS] STARTUP PAIR_SEND_RESULT command=062 status=accepted"),
            Err(error) => {
                warn!("[GNSS] STARTUP PAIR_SEND_RESULT command=062 status=error error={}", error)
            }
        }
        debug!("[GNSS] STARTUP PAIR_SEND command=063 sentence={}", sentence);
        match gnss.query_nmea_output_rate(sentence).await {
            Ok(()) => debug!("[GNSS] STARTUP PAIR_SEND_RESULT command=063 status=accepted"),
            Err(error) => {
                warn!("[GNSS] STARTUP PAIR_SEND_RESULT command=063 status=error error={}", error)
            }
        }
    }
    if let Ok(command) = PairCommandBuilder::new(67).and_then(|builder| builder.finish()) {
        debug!("[GNSS] STARTUP PAIR_SEND command=067");
        match gnss.send_pair_command(&command).await {
            Ok(()) => debug!("[GNSS] STARTUP PAIR_SEND_RESULT command=067 status=accepted"),
            Err(error) => {
                warn!("[GNSS] STARTUP PAIR_SEND_RESULT command=067 status=error error={}", error)
            }
        }
    }
    let mut nmea = [0u8; 512];
    let mut nmea_parser = NmeaParser::new();
    #[cfg(feature = "gnss-raw-log")]
    let mut raw_nmea_line = [0; 256];
    #[cfg(feature = "gnss-raw-log")]
    let mut raw_nmea_line_len = 0;
    let mut gnss_parse_ok = false;
    match gnss.read_nmea_chunk(&mut nmea).await {
        Ok(data) if !data.is_empty() => {
            debug!("[GNSS] NMEA_CHUNK_OK bytes={}", data.len());
            #[cfg(feature = "gnss-raw-log")]
            log_raw_nmea(data, &mut raw_nmea_line, &mut raw_nmea_line_len);
            for &byte in data {
                match nmea_parser.push(byte) {
                    Ok(Some(NmeaUpdate::PairAck(ack))) => {
                        debug!("[GNSS] STARTUP PAIR_ACK command={} status={}", ack.command, ack.status)
                    }
                    Ok(Some(NmeaUpdate::NmeaOutputRate { sentence, rate })) => {
                        debug!("[GNSS] STARTUP PAIR_RESPONSE command=063 sentence={} rate={}", sentence, rate)
                    }
                    Ok(Some(NmeaUpdate::Pair(message))) => debug!("[GNSS] STARTUP PAIR_RESPONSE command={} fields={=[u8]:a}",
                        message.command(),
                        message.fields()
                    ),
                    Ok(_) => {}
                    Err(error) => warn!("[GNSS] NMEA_PARSE_ERROR {=str}", error),
                }
            }
            gnss_parse_ok = nmea_parser.state().fix.is_some();
            let signal = nmea_parser.state().signal;
            debug!(
                "[GNSS] acquisition in_view={} with_signal={} used={} strongest_snr_db={} fix_type={} hdop_milli={}",
                signal.satellites_in_view.get(),
                signal.satellites_with_signal.get(),
                signal.satellites_used.get(),
                signal.strongest_snr.map(|value| value.get()),
                signal.fix_type,
                signal.hdop.map(|value| value.get()),
            );
        }
        Ok(_) => debug!("[GNSS] NMEA_EMPTY bytes=0"),
        Err(GnssError::I2c { operation, error }) => {
            warn!("[GNSS] I2C_FAILED operation={} error={}", operation, error);
        }
        Err(GnssError::BufferTooSmall { .. }) => warn!("[GNSS] NMEA_BUFFER_TOO_SMALL"),
        Err(GnssError::PairCommand(_)) => warn!("[GNSS] PAIR_COMMAND_BUILD_FAILED"),
    }

    let lora_spi_config = spi::master::Config::default()
        .with_frequency(Rate::from_mhz(8))
        .with_mode(spi::Mode::_0);
    let lora_spi = spi::master::Spi::new(peripherals.SPI3, lora_spi_config)
        .expect("LoRa SPI failed")
        .with_sck(peripherals.GPIO16)
        .with_mosi(peripherals.GPIO43)
        .with_miso(peripherals.GPIO17)
        .into_async();
    let lora_cs = Output::new(peripherals.GPIO18, Level::High, OutputConfig::default());
    let lora_spi =
        ExclusiveDevice::new(lora_spi, lora_cs, embassy_time::Delay).expect("LoRa CS failed");
    let mut lora_config = Sx127xLoraConfig::for_variant::<Sx1272>();
    lora_config.auto_optimize = true;
    lora_config.use_crc = true;
    debug!("[LORA] init_start");
    let mut lora = match Sx1272Lora::new_with_config(lora_spi, lora_config).await {
        Ok(lora) => {
            info!("[LORA] init_ok");
            lora
        }
        Err(sx127xlora::driver::Sx127xError::InvalidVersion) => {
            error!("[LORA] init_failed reason=invalid_version");
            loop {
                Timer::after(Duration::from_secs(1)).await;
            }
        }
        Err(sx127xlora::driver::Sx127xError::SPI(_)) => {
            error!("[LORA] init_failed reason=spi");
            loop {
                Timer::after(Duration::from_secs(1)).await;
            }
        }
        Err(_) => {
            error!("[LORA] init_failed reason=configuration");
            loop {
                Timer::after(Duration::from_secs(1)).await;
            }
        }
    };
    let lora_probe = {
        let op_mode = lora.read(OP_MODE).await;
        let frf_msb = lora.read(FRF_MSB).await;
        let irq_flags = lora.read(IRQ_FLAGS).await;
        let version = lora.read(VERSION).await;
        match (op_mode, frf_msb, irq_flags, version) {
            (Ok(op_mode), Ok(frf_msb), Ok(irq_flags), Ok(version)) => {
                Ok([op_mode, frf_msb, irq_flags, version])
            }
            _ => Err(()),
        }
    };
    let lora_result = 0;
    let lora_dio0 = Input::new(peripherals.GPIO44, InputConfig::default());
    let lora_path = LoraPath::new(i2c.clone(), !(lora_reset | lora_tx_switch));

    let mut rtc = Pcf85063aRtc::new(i2c.clone());
    rtc.init().await.unwrap();
    match rtc.get_time().await {
        Ok(time) => info!(
            "[RTC] OK 20{:02}-{:02}-{:02} {:02}:{:02}:{:02}",
            time.year, time.month, time.day, time.hours, time.minutes, time.seconds
        ),
        Err(_) => warn!("[RTC] TIME_INVALID"),
    }

    let mut magnetometer = Bmm350::new(i2c.clone());
    let mut bmm_ready = false;
    let mut bmm_compensated = false;
    match magnetometer.init().await {
        Ok(()) => {
            magnetometer.start_normal_mode().await.unwrap();
            Timer::after(Duration::from_millis(15)).await;
            bmm_ready = magnetometer.data_ready().await.unwrap();
            match magnetometer.read_data().await {
                Ok(data) => {
                    debug!(
                        "[BMM350] RAW x={} y={} z={} temp={} sensor_time={}",
                        data.raw.x, data.raw.y, data.raw.z, data.raw.temperature, data.sensor_time
                    );
                    if let Some(compensated) = magnetometer.compensate(&data) {
                        debug!(
                            "[BMM350] COMP x={}uT y={}uT z={}uT temp={}C",
                            compensated.x_microtesla,
                            compensated.y_microtesla,
                            compensated.z_microtesla,
                            compensated.temperature_celsius
                        );
                        bmm_compensated = true;
                    }
                }
                Err(error) => warn!("[BMM350] burst read failed: {}", error),
            }
        }
        Err(error) => error!("[BMM350] unavailable: {}", error),
    }
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

    let imu_int2 = Input::new(peripherals.GPIO21, InputConfig::default());

    let mut imu = ph_qmi8658::Qmi8658::with_i2c_config(
        i2c.clone(),
        None::<core::convert::Infallible>,
        Some(imu_int2),
        config,
        // The QMI8658 I2C output registers are low-byte first.
        ph_qmi8658::I2cConfig::new(board::IMU_I2C_ADDR).with_big_endian(false),
    );
    imu.init(&mut embassy_time::Delay).await.unwrap();
    // imu.apply_interrupt_config(ph_qmi8658::InterruptConfig {
    //     ctrl9_handshake_statusint: false,
    //     motion_pin: ph_qmi8658::InterruptPin::Int2,
    //     pedometer: false,
    //     significant_motion: false,
    //     no_motion: false,
    //     any_motion: false,
    //     tap: false,
    // })
    // .await
    // .unwrap();
    imu.set_mode_with_delay(
        &mut embassy_time::Delay,
        ph_qmi8658::OperatingMode::AccelGyroOnly,
    )
    .await
    .unwrap();
    imu.set_sync_sample(true).await.unwrap();

    info!("[IMU] INIT");
    info!(
        "[LORA] {=str}",
        match lora_result {
            0 => "SX1272_OK",
            1 => "BAD_VERSION",
            10 => "BAD_VERSION_00",
            11 => "BAD_VERSION_FF",
            2 => "SPI_FAILED",
            _ => "CS_FAILED",
        }
    );
    match lora_probe {
        Ok([op_mode, frf_msb, irq_flags, version]) => debug!(
            "[LORA] REGISTERS OP_MODE={=u8:#04x} FRF_MSB={=u8:#04x} IRQ={=u8:#04x} VERSION={=u8:#04x}",
            op_mode, frf_msb, irq_flags, version
        ),
        Err(_) => warn!("[LORA] REGISTER_PROBE_FAILED"),
    }
    info!(
        "[GNSS] {=str}",
        if gnss_parse_ok {
            "NMEA_PARSE_OK"
        } else {
            "NMEA_NO_COMPLETE_LINE"
        }
    );
    info!("[BMM350] DATA_READY={} COMPENSATION={}", bmm_ready, bmm_compensated);

    let touch_rst = Output::new(peripherals.GPIO40, Level::High, OutputConfig::default());
    let touch_int = Input::new(peripherals.GPIO11, InputConfig::default());
    let mut touch = Cst9217::new(i2c.clone(), touch_rst, touch_int, embassy_time::Delay);
    touch.init().await.unwrap();
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

    // SAFETY: the display core has not started, and `settings_task` holds it for every write.
    // The seed only spreads wear across pages, so the RNG need not be at full entropy.
    let seed = esp_hal::rng::Rng::new().random();
    let mut store = unsafe { Store::new(esp_storage::FlashStorage::new(peripherals.FLASH), seed) };
    let saved = store.load();
    info!(
        "[SETTINGS] zone mode={} manual={} automatic={} brightness={}",
        saved.zone_mode,
        saved.manual_zone.map(|zone| DATABASE.zone(zone).name),
        saved.automatic_zone.map(|zone| DATABASE.zone(zone).name),
        saved.brightness,
    );
    let brightness = saved.brightness.unwrap_or(DEFAULT_BRIGHTNESS);
    // The display core applies it as it starts the panel.
    BRIGHTNESS.store(u16::from(brightness), Ordering::Relaxed);
    spawner.spawn(settings_task(store).unwrap());

    let initial_sensor_state = SensorSnapshot {
        battery_present,
        battery_mv: battery_present.then(|| pmic_readings.0.ok()).flatten(),
        vbus_mv: pmic_readings.1.ok(),
        vsys_mv: pmic_readings.2.ok(),
        gnss: nmea_parser.state(),
        ..SensorSnapshot::default()
    };
    spawner.spawn(
        sensor_task(SensorTask {
            power,
            lora,
            lora_dio0,
            lora_path,
            gnss,
            nmea: [0; 512],
            nmea_parser,
            rtc,
            state: initial_sensor_state,
            rtc_sync_pending: true,
            zones: ZoneTracker {
                mode: saved.zone_mode,
                manual: saved.manual_zone,
                automatic: saved.automatic_zone,
                looked_up_at: None,
            },
        })
        .unwrap(),
    );

    spawner.spawn(
        motion_task(MotionTask {
            magnetometer: (bmm_ready && bmm_compensated).then_some(magnetometer),
            imu,
            accel_lsb_per_g,
            gyro_lsb_per_dps,
        })
        .unwrap(),
    );

    start_display_core!(peripherals, fb_st);

    let mut stage = Stage::new(PeripheralState {
        brightness,
        firmware: env!("CARGO_PKG_VERSION"),
        ..PeripheralState::default()
    });
    // The last report the controller wrote, which a stale read repeats.
    let mut touch_data = TouchData::default();
    let mut last_report = Instant::now();
    /// A contact with no fresh report for this long has ended without its lift report. Held
    /// fingers were reported at most 101 ms apart.
    const LIFT_WITHOUT_REPORT: Duration = Duration::from_millis(300);
    info!(
        "[MEM] internal_used={} psram_used={}",
        esp_alloc::HEAP.used(),
        PSRAM_HEAP.used(),
    );
    let mut prev_swap_draw = Duration::MIN;
    // The buffer drawn next last held the frame before the current one, so it repaints the
    // current step's damage as well as its own.
    let mut previous_changed = Box::new(Dirty::new_full());
    let mut repaint = Box::new(Dirty::new());
    #[cfg(feature = "damage-debug")]
    let mut outlined = Dirty::new();
    let mut last_touch_poll = Instant::now();
    const TOUCH_REPOLL: Duration = Duration::from_micros(16_667);
    loop {
        let start = Instant::now();
        {
            let state = fb_st.get();
            let SwapState {
                fb,
                dirty,
                timings,
                drawn,
                #[cfg(feature = "damage-debug")]
                debug_changed,
            } = state;
            let fb = &mut **fb;
            // Keep reading while either tracker holds a contact, or neither sees it lift.
            let in_contact = stage.in_contact();
            let touch_repoll_due = in_contact && last_touch_poll.elapsed() >= TOUCH_REPOLL;
            let wait_timeout = if touch_repoll_due || stage.is_animating() {
                Duration::from_micros(0)
            } else if in_contact {
                TOUCH_REPOLL
            } else {
                Duration::from_millis(250)
            };
            let (touch_ready, sensor_state, motion_state) = match select4(
                touch.wait_for_touch(),
                SENSOR_STATE.wait(),
                MOTION_STATE.wait(),
                Timer::after(wait_timeout),
            )
            .await
            {
                Either4::First(result) => (result.is_ok(), None, None),
                Either4::Second(state) => (false, Some(state), None),
                Either4::Third(state) => (false, None, Some(state)),
                Either4::Fourth(()) => (touch_repoll_due, None, None),
            };
            if touch_ready {
                match touch.read_touch_data().await.unwrap() {
                    TouchData::Stale if last_report.elapsed() < LIFT_WITHOUT_REPORT => {}
                    TouchData::Stale => touch_data = TouchData::default(),
                    fresh => {
                        touch_data = fresh;
                        last_report = Instant::now();
                    }
                }
                last_touch_poll = Instant::now();
            }
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
            });
            if update.recalibrate {
                info!("[COMPASS] recalibrating on request");
                COMPASS_RECALIBRATE.store(true, Ordering::Relaxed);
            }
            if let Some(level) = update.brightness {
                BRIGHTNESS.store(u16::from(level), Ordering::Relaxed);
            }
            if let Some(choice) = update.store {
                info!("[SETTINGS] chosen {}", choice);
                let write = match choice {
                    Choice::Brightness(level) => settings::Write::Brightness(level),
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
                if SETTINGS_WRITES.try_send(write).is_err() {
                    warn!("[SETTINGS] queue full, {} not saved", write);
                }
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
    }
}
