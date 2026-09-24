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
use core::sync::atomic::{AtomicBool, Ordering};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_futures::select::{Either, Either3, Either4, select, select3, select4};
use embassy_sync::{
    blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex},
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
use esp_println::println;
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
    ui::{
        compass::{AxisMap, Calibration, CalibrationEvent, CompassView, Vec3},
        fusion::Fusion,
        imu::{accel_micro_ms2, gyro_micro_rad_s},
        prototypes::{self, Screen},
        stage::{Input as StageInput, Motion, Sensors, Stage, Touch},
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
    clock: prototypes::ClockState,
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
            println!("[GNSS] RAW_TRUNCATED");
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
                Ok(sentence) => println!("[GNSS] RAW {sentence}"),
                Err(_) => println!("[GNSS] RAW_NON_UTF8 len={end}"),
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
    println!("[DISPLAY] core_started");
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
    display
        .set_brightness(120)
        .expect("brightness command failed");

    println!("[DISPLAY] OK");

    let mut prev_swap_spi = Duration::MIN;
    let mut first_flush = true;
    loop {
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
                println!("[DISPLAY] first_flush_without_te");
                first_flush = false;
            } else {
                match select(
                    display.wait_for_vsync(),
                    Timer::after(Duration::from_millis(17)),
                )
                .await
                {
                    Either::First(()) => {}
                    Either::Second(()) => println!("[DISPLAY] te_timeout"),
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
        defmt::info!(
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

/// Sensor axes into the screen frame of `ui::compass`, fitted from the axis check screen's twelve
/// poses by `tools/fit-sensor-axes.py`. Both chips sit face down on their boards.
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
    let mut state = Motion::default();
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
            println!("[COMPASS] recalibrating");
        }
        let mut raw_field = None;
        let mut new_field = None;

        if let Some(magnetometer) = &mut magnetometer
            && magnetometer.data_ready().await.unwrap_or(false)
            && let Ok(data) = magnetometer.read_data().await
            && let Some(compensated) = magnetometer.compensate(&data)
        {
            if log {
                println!(
                    "[BMM350] sample raw=({}, {}, {}) comp=({:.3}, {:.3}, {:.3})uT temp={:.3}C",
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
            state.magnetic_microtesla = sample.map(|value| (value * 1_000.0) as i32);
            let sample = MAG_AXES.apply(sample);
            let event = calibration.update(sample);
            let offset = calibration.offset();
            if event != CalibrationEvent::None {
                println!(
                    "[COMPASS] calibration {:?} offset={:?}uT radius={}uT",
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
            if let Some(raw) = sample.accel {
                state.accel_micro_ms2 = [
                    accel_micro_ms2(raw.x, accel_lsb_per_g),
                    accel_micro_ms2(raw.y, accel_lsb_per_g),
                    accel_micro_ms2(raw.z, accel_lsb_per_g),
                ];
                accel = Some(IMU_AXES.apply(
                    state.accel_micro_ms2.map(|value| value as f32 / 1e6),
                ));
            }
            if let Some(raw) = sample.gyro {
                state.gyro_micro_rad_s = [
                    gyro_micro_rad_s(raw.x, gyro_lsb_per_dps),
                    gyro_micro_rad_s(raw.y, gyro_lsb_per_dps),
                    gyro_micro_rad_s(raw.z, gyro_lsb_per_dps),
                ];
                gyro = Some(IMU_AXES.apply(
                    state.gyro_micro_rad_s.map(|value| value as f32 / 1e6),
                ));
            }
            state.imu_valid = sample.accel.is_some() || sample.gyro.is_some();
            if log {
                println!(
                    "[IMU] sample accel={:?} gyro={:?} si_accel={:?} si_gyro={:?}",
                    sample.accel, sample.gyro, state.accel_micro_ms2, state.gyro_micro_rad_s
                );
            }
        } else {
            state.imu_valid = false;
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
        state.compass = CompassView::new(fusion.attitude(), field, &calibration);
        if log && compass_active {
            println!(
                "[COMPASS] screen accel={:?} field={:?}uT offset={:?}uT gyro_offset={:?} candidate={:?} view={:?}",
                accel,
                field,
                calibration.offset(),
                fusion.gyro_offset(),
                calibration
                    .candidate()
                    .map(|candidate| (candidate.progress(), candidate.residual())),
                state.compass
            );
        }
        MOTION_STATE.signal(state);
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
    } = task;
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

        let battery_present = power.is_battery_present().await.unwrap_or(false);
        let battery_mv = power.get_battery_voltage().await.ok();
        let vbus_mv = power.get_vbus_voltage().await.ok();
        let vsys_mv = power.get_system_voltage().await.ok();
        state.battery_present = battery_present;
        state.battery_mv = battery_present.then_some(battery_mv).flatten();
        state.vbus_mv = vbus_mv;
        state.vsys_mv = vsys_mv;
        println!(
            "[PMIC] sample battery_present={battery_present} VBAT={battery_mv:?}mV VBUS={vbus_mv:?}mV VSYS={vsys_mv:?}mV"
        );

        if let Ok(irq) = lora.read(IRQ_FLAGS).await {
            state.lora_irq = irq;
            println!("[LORA] sample IRQ=0x{irq:02X}");
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
                            println!(
                                "[GNSS] PAIR_ACK command={} status={:?}",
                                ack.command, ack.status
                            );
                        }
                        Ok(Some(NmeaUpdate::NmeaOutputRate { sentence, rate })) => {
                            updates += 1;
                            println!("[GNSS] NMEA_OUTPUT_RATE sentence={sentence:?} rate={rate:?}");
                        }
                        Ok(Some(NmeaUpdate::Pair(message))) => {
                            updates += 1;
                            println!(
                                "[GNSS] PAIR_RESPONSE command={} fields={:?}",
                                message.command(),
                                message.fields()
                            );
                        }
                        Ok(Some(_)) => updates += 1,
                        Ok(None) => {}
                        Err(error) => println!("[GNSS] NMEA_PARSE_ERROR {error}"),
                    }
                }
                state.gnss = nmea_parser.state();
                let signal = state.gnss.signal;
                println!(
                    "[GNSS] acquisition in_view={} with_signal={} used={} strongest_snr_db={:?} fix_type={:?} hdop_milli={:?}",
                    signal.satellites_in_view.get(),
                    signal.satellites_with_signal.get(),
                    signal.satellites_used.get(),
                    signal.strongest_snr.map(|value| value.get()),
                    signal.fix_type,
                    signal.hdop.map(|value| value.get()),
                );
                if let Some(fix) = state.gnss.fix {
                    println!(
                        "[GNSS] sample bytes={} updates={} lat={} lon={} alt_mm={:?} sats={:?} hdop_milli={:?}",
                        data.len(),
                        updates,
                        fix.latitude.get(),
                        fix.longitude.get(),
                        fix.altitude.map(|value| value.get()),
                        fix.satellites,
                        fix.hdop.map(|value| value.get()),
                    );
                } else {
                    println!(
                        "[GNSS] sample bytes={} updates={} no_fix",
                        data.len(),
                        updates
                    );
                }
            }
            Err(GnssError::I2c { operation, error }) => {
                println!("[GNSS] I2C_FAILED operation={operation:?} error={error:?}");
            }
            Err(GnssError::BufferTooSmall { .. }) => {
                println!("[GNSS] NMEA_BUFFER_TOO_SMALL")
            }
            Err(GnssError::PairCommand(_)) => println!("[GNSS] PAIR_COMMAND_BUILD_FAILED"),
        }
        if state.gnss.utc.is_none() {
            rtc_sync_pending = true;
        } else if rtc_sync_pending && let Some(utc) = state.gnss.utc {
            if !(2000..=2099).contains(&utc.year) {
                println!("[RTC] GNSS year outside RTC range: {}", utc.year);
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
                        println!(
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
                    Err(_) => println!("[RTC] GNSS synchronization failed"),
                }
            }
        }
        if let Ok(time) = rtc.get_time().await {
            state.clock = prototypes::ClockState {
                hours: time.hours,
                minutes: time.minutes,
                seconds: time.seconds,
                day: time.day,
                month: time.month,
                year: time.year,
                valid: true,
            };
        } else {
            state.clock.valid = false;
        }

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
                println!("[LORA] LINK_TX_SWITCH_FAILED");
            } else if lora.tx(&payload).await.is_err() {
                println!("[LORA] LINK_TX_FAILED");
            } else {
                match select(
                    lora_dio0.wait_for_rising_edge(),
                    Timer::after(Duration::from_secs(2)),
                )
                .await
                {
                    Either::First(_) => {
                        println!("[LORA] LINK_TX_DONE sequence={}", lora_sequence - 1);
                        let _ = lora.clear_interrupt::<TxDone>().await;
                    }
                    Either::Second(_) => println!("[LORA] LINK_TX_TIMEOUT"),
                }
                let _ = lora_path.receive().await;
            }
        }

        #[cfg(feature = "lora-link-rx")]
        {
            println!("[LORA] LINK_RX_START");
            if lora_path.receive().await.is_err() {
                println!("[LORA] LINK_RX_SWITCH_FAILED");
            } else if lora.rx(None).await.is_err() {
                println!("[LORA] LINK_RX_START_FAILED");
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
                        Ok(packet) => println!(
                            "[LORA] LINK_RX_OK len={} rssi={} snr_raw={} payload={:?}",
                            packet.length,
                            packet.rssi,
                            packet.snr_raw,
                            packet.payload(),
                        ),
                        Err(_) => println!("[LORA] LINK_RX_PACKET_FAILED"),
                    }
                } else {
                    println!("[LORA] LINK_RX_TIMEOUT");
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
    println!("{name}: {:.1}ms", took.as_micros() as f32 / 1_000.,);
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
    println!(
        "[FONTDUE-BENCH] arm={} point_bytes={} px12={} px32={} px64={} checksums={},{},{}",
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
        println!(
            "[FONTDUE-BENCH] repeat arm={} point_bytes={} px12={} px32={} px64={} checksums={},{},{}",
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
    esp_alloc::heap_allocator!(size: 252 * 1024);

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
    let pmic_valid = pmic_readings.0.is_ok() && pmic_readings.1.is_ok() && pmic_readings.2.is_ok();

    let mut exio = Tca9554::new(i2c.clone(), tca9554::Address::standard());

    println!("[TCA9554] init_start");
    exio.init().await.unwrap();
    exio.write_output(u8::MAX).await.unwrap();
    let exio_output_mask = (1 << board::EXIO_GPS_RESET)
        | (1 << board::EXIO_LORA_RESET)
        | (1 << board::EXIO_LORA_RX_SWITCH)
        | (1 << board::EXIO_LORA_TX_SWITCH);
    exio.write_direction(!exio_output_mask).await.unwrap();
    println!("[TCA9554] OK");

    let gps_reset = 1 << board::EXIO_GPS_RESET;
    let lora_reset = 1 << board::EXIO_LORA_RESET;
    let lora_rx_switch = 1 << board::EXIO_LORA_RX_SWITCH;
    let lora_tx_switch = 1 << board::EXIO_LORA_TX_SWITCH;
    let gnss_trace_start = Instant::now();
    macro_rules! gnss_trace {
        ($($arg:tt)*) => {{
            println!(
                "[GNSS] STARTUP t_us={} {}",
                gnss_trace_start.elapsed().as_micros(),
                format_args!($($arg)*)
            );
        }};
    }

    gnss_trace!("RESET_SEQUENCE_BEGIN");
    exio.write_output(!(gps_reset | lora_reset | lora_tx_switch))
        .await
        .unwrap();
    gnss_trace!(
        "RESET_OUTPUT phase=initial value=0x{:02X}",
        !(gps_reset | lora_reset | lora_tx_switch)
    );
    Timer::after(Duration::from_millis(10)).await;
    exio.write_output(!lora_tx_switch)
        .await
        .unwrap();
    gnss_trace!(
        "RESET_OUTPUT phase=release_gps_pulse_lora value=0x{:02X}",
        !lora_tx_switch
    );
    Timer::after(Duration::from_micros(200)).await;
    exio.write_output(!(lora_reset | lora_tx_switch))
        .await
        .unwrap();
    gnss_trace!(
        "RESET_OUTPUT phase=release_lora value=0x{:02X}",
        !(lora_reset | lora_tx_switch)
    );
    exio.write_direction(!(lora_reset | lora_rx_switch | lora_tx_switch))
        .await
        .unwrap();
    gnss_trace!(
        "RESET_DIRECTION value=0x{:02X}",
        !(lora_reset | lora_rx_switch | lora_tx_switch)
    );
    Timer::after(Duration::from_millis(10)).await;
    exio.write_output(!(lora_reset | lora_tx_switch)).await.unwrap();
    gnss_trace!(
        "RESET_OUTPUT phase=select_rx value=0x{:02X}",
        !(lora_reset | lora_tx_switch)
    );
    let exio_direction = exio.read_direction().await.unwrap();
    println!("[TCA9554] direction=0x{exio_direction:02X}");
    {
        println!("[GNSS] STARTUP settle_begin");
        Timer::after(Duration::from_secs(1)).await;
        println!("[GNSS] STARTUP settle_complete");
    }
    drop(exio);

    let mut gnss = Lc76g::new(i2c.clone(), embassy_time::Delay);
    gnss_trace!("PAIR_SEND command=732 mode={GNSS_LOW_POWER_MODE:?}");
    match gnss.set_low_power_mode(GNSS_LOW_POWER_MODE).await {
        Ok(()) => gnss_trace!("PAIR_SEND_RESULT command=732 status=accepted"),
        Err(error) => gnss_trace!("PAIR_SEND_RESULT command=732 status=error error={error:?}"),
    }
    for sentence in [NmeaSentence::Gsa, NmeaSentence::Gsv] {
        gnss_trace!("PAIR_SEND command=062 sentence={sentence:?} rate=1");
        match gnss
            .set_nmea_output_rate(sentence, NmeaOutputRate::EVERY_FIX)
            .await
        {
            Ok(()) => gnss_trace!("PAIR_SEND_RESULT command=062 status=accepted"),
            Err(error) => {
                gnss_trace!("PAIR_SEND_RESULT command=062 status=error error={error:?}")
            }
        }
        gnss_trace!("PAIR_SEND command=063 sentence={sentence:?}");
        match gnss.query_nmea_output_rate(sentence).await {
            Ok(()) => gnss_trace!("PAIR_SEND_RESULT command=063 status=accepted"),
            Err(error) => {
                gnss_trace!("PAIR_SEND_RESULT command=063 status=error error={error:?}")
            }
        }
    }
    if let Ok(command) = PairCommandBuilder::new(67).and_then(|builder| builder.finish()) {
        gnss_trace!("PAIR_SEND command=067");
        match gnss.send_pair_command(&command).await {
            Ok(()) => gnss_trace!("PAIR_SEND_RESULT command=067 status=accepted"),
            Err(error) => {
                gnss_trace!("PAIR_SEND_RESULT command=067 status=error error={error:?}")
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
            println!("[GNSS] NMEA_CHUNK_OK bytes={}", data.len());
            #[cfg(feature = "gnss-raw-log")]
            log_raw_nmea(data, &mut raw_nmea_line, &mut raw_nmea_line_len);
            for &byte in data {
                match nmea_parser.push(byte) {
                    Ok(Some(NmeaUpdate::PairAck(ack))) => {
                        gnss_trace!("PAIR_ACK command={} status={:?}", ack.command, ack.status)
                    }
                    Ok(Some(NmeaUpdate::NmeaOutputRate { sentence, rate })) => {
                        gnss_trace!("PAIR_RESPONSE command=063 sentence={sentence:?} rate={rate:?}")
                    }
                    Ok(Some(NmeaUpdate::Pair(message))) => gnss_trace!(
                        "PAIR_RESPONSE command={} fields={:?}",
                        message.command(),
                        message.fields()
                    ),
                    Ok(_) => {}
                    Err(error) => println!("[GNSS] NMEA_PARSE_ERROR {error}"),
                }
            }
            gnss_parse_ok = nmea_parser.state().fix.is_some();
            let signal = nmea_parser.state().signal;
            println!(
                "[GNSS] acquisition in_view={} with_signal={} used={} strongest_snr_db={:?} fix_type={:?} hdop_milli={:?}",
                signal.satellites_in_view.get(),
                signal.satellites_with_signal.get(),
                signal.satellites_used.get(),
                signal.strongest_snr.map(|value| value.get()),
                signal.fix_type,
                signal.hdop.map(|value| value.get()),
            );
        }
        Ok(_) => println!("[GNSS] NMEA_EMPTY bytes=0"),
        Err(GnssError::I2c { operation, error }) => {
            println!("[GNSS] I2C_FAILED operation={operation:?} error={error:?}");
        }
        Err(GnssError::BufferTooSmall { .. }) => println!("[GNSS] NMEA_BUFFER_TOO_SMALL"),
        Err(GnssError::PairCommand(_)) => println!("[GNSS] PAIR_COMMAND_BUILD_FAILED"),
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
    println!("[LORA] init_start");
    let mut lora = match Sx1272Lora::new_with_config(lora_spi, lora_config).await {
        Ok(lora) => {
            println!("[LORA] init_ok");
            lora
        }
        Err(sx127xlora::driver::Sx127xError::InvalidVersion) => {
            println!("[LORA] init_failed reason=invalid_version");
            loop {
                Timer::after(Duration::from_secs(1)).await;
            }
        }
        Err(sx127xlora::driver::Sx127xError::SPI(_)) => {
            println!("[LORA] init_failed reason=spi");
            loop {
                Timer::after(Duration::from_secs(1)).await;
            }
        }
        Err(_) => {
            println!("[LORA] init_failed reason=configuration");
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
        Ok(time) => println!(
            "[RTC] OK 20{:02}-{:02}-{:02} {:02}:{:02}:{:02}",
            time.year, time.month, time.day, time.hours, time.minutes, time.seconds
        ),
        Err(_) => println!("[RTC] TIME_INVALID"),
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
                    println!(
                        "[BMM350] RAW x={} y={} z={} temp={} sensor_time={}",
                        data.raw.x, data.raw.y, data.raw.z, data.raw.temperature, data.sensor_time
                    );
                    if let Some(compensated) = magnetometer.compensate(&data) {
                        println!(
                            "[BMM350] COMP x={:.3}uT y={:.3}uT z={:.3}uT temp={:.3}C",
                            compensated.x_microtesla,
                            compensated.y_microtesla,
                            compensated.z_microtesla,
                            compensated.temperature_celsius
                        );
                        bmm_compensated = true;
                    }
                }
                Err(error) => println!("[BMM350] burst read failed: {error:?}"),
            }
        }
        Err(error) => println!("[BMM350] unavailable: {error:?}"),
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

    println!("[IMU] INIT");
    println!(
        "[LORA] {}",
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
        Ok([op_mode, frf_msb, irq_flags, version]) => println!(
            "[LORA] REGISTERS OP_MODE=0x{op_mode:02X} FRF_MSB=0x{frf_msb:02X} IRQ=0x{irq_flags:02X} VERSION=0x{version:02X}"
        ),
        Err(_) => println!("[LORA] REGISTER_PROBE_FAILED"),
    }
    println!(
        "[GNSS] {}",
        if gnss_parse_ok {
            "NMEA_PARSE_OK"
        } else {
            "NMEA_NO_COMPLETE_LINE"
        }
    );
    println!("[BMM350] DATA_READY={bmm_ready} COMPENSATION={bmm_compensated}");

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
    let res = touch.resolution();
    println!("[TOUCH] OK, Resolution: {res}");

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

    let mut stage = Stage::new(prototypes::PeripheralState {
        pmic_valid,
        battery_mv: initial_sensor_state.battery_mv,
        vbus_mv: initial_sensor_state.vbus_mv,
        vsys_mv: initial_sensor_state.vsys_mv,
        tca_valid: true,
        gnss_valid: gnss_parse_ok,
        lora_valid: lora_result == 0,
        ..prototypes::PeripheralState::default()
    });
    let mut touch_data = TouchData::default();
    println!(
        "[MEM] internal_used={} psram_used={}",
        esp_alloc::HEAP.used(),
        PSRAM_HEAP.used(),
    );
    let mut prev_swap_draw = Duration::MIN;
    // The buffer drawn next last held the frame before the current one, so it repaints the
    // current step's damage as well as its own.
    let mut previous_changed = Dirty::new_full();
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
                touch_data = touch.read_touch_data().await.unwrap();
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
                }),
                motion: motion_state,
                sensors: sensor_state.map(|state| Sensors {
                    battery_mv: state.battery_mv,
                    vbus_mv: state.vbus_mv,
                    vsys_mv: state.vsys_mv,
                    gnss_bytes: state.gnss_bytes,
                    gnss_fix: state.gnss.fix.is_some(),
                    lora_irq: state.lora_irq,
                    clock: state.clock,
                }),
            });
            if update.recalibrate {
                println!("[TOUCH] cover accepted, recalibrating");
                COMPASS_RECALIBRATE.store(true, Ordering::Relaxed);
            }
            COMPASS_ACTIVE.store(update.samples_fast, Ordering::Relaxed);
            if let Some(record) = update.pose {
                println!(
                    "[POSE] pose={} mag=({:.2}, {:.2}, {:.2})uT accel=({:.3}, {:.3}, {:.3}) samples={}",
                    record.pose + 1,
                    record.magnetic_microtesla[0],
                    record.magnetic_microtesla[1],
                    record.magnetic_microtesla[2],
                    record.accel_ms2[0],
                    record.accel_ms2[1],
                    record.accel_ms2[2],
                    record.samples
                );
            }
            let changed = update.changed;
            let mut repaint = previous_changed;
            repaint.extend(&changed);
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
            dirty.clone_from(&changed);
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
                *debug_changed = changed.clone();
            }
            previous_changed = changed;

            timings.frametime = start.elapsed();
            timings.swap_draw = prev_swap_draw;

            #[cfg(feature = "timing-log")]
            defmt::info!(
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
