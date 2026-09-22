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
extern crate alloc;

use alloc::{alloc::Allocator, boxed::Box};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_futures::select::{Either, Either3, select, select3};
use embassy_sync::{
    blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex},
    mutex::Mutex,
    signal::Signal,
};
use embassy_time::{Duration, Instant, Timer};
use embedded_graphics::prelude::*;
use esp_hal::{
    dma_tx_buffer,
    gpio::{Input, InputConfig, Level, Output, OutputConfig},
    i2c::master::I2c,
    peripherals, spi,
    time::Rate,
    timer::timg::TimerGroup,
};
use esp_println::println;
use lc76g::{GnssError, GnssOperation, GnssState, Lc76g, NmeaParser};
use octowhere::{
    board,
    chrome::{self, Color, Dirty, FB, FontdueRenderer, FontdueRendererCtx},
    drivers::{co5300::Co5300Display, framebuffer::Framebuffer, qspi_bus::QspiBus},
    peripherals::{
        lora::{LoraError, Sx1272},
        magnetometer::Bmm350,
        power::Axp2101Power,
        rtc::{DateTime as RtcDateTime, Pcf85063aRtc},
        touch::{Cst9217, Cst9217Config, TouchData},
    },
    ui::{
        dirty::DirtyAreas,
        imu::{accel_micro_ms2, gyro_micro_rad_s},
        input::{Button, ButtonEvent, TouchState},
        prototypes::{self, Screen},
    },
    util::{Swap, SwapThread},
};
use static_cell::StaticCell;
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
type SensorLora = Sx1272<spi::master::Spi<'static, esp_hal::Async>, Output<'static>>;

static I2C_BUS: StaticCell<Mutex<NoopRawMutex, I2cBus>> = StaticCell::new();
static SENSOR_STATE: Signal<CriticalSectionRawMutex, SensorSnapshot> = Signal::new();
pub static PSRAM_HEAP: esp_alloc::EspHeap = esp_alloc::EspHeap::empty();

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
    accel_micro_ms2: [i32; 3],
    gyro_micro_rad_s: [i32; 3],
    imu_valid: bool,
    magnetic_microtesla: [i32; 3],
}

struct SensorTask {
    power: Axp2101Power<SharedI2cDevice>,
    lora: SensorLora,
    gnss: Lc76g<SharedI2cDevice, embassy_time::Delay>,
    nmea: [u8; 64],
    nmea_parser: NmeaParser,
    rtc: Pcf85063aRtc<SharedI2cDevice>,
    magnetometer: Bmm350<SharedI2cDevice>,
    imu: SensorImu,
    accel_lsb_per_g: i32,
    gyro_lsb_per_dps: i32,
    state: SensorSnapshot,
    rtc_sync_pending: bool,
}

macro_rules! start_display_core {
    ($peripherals:ident, $framebuffer_thread:ident) => {
        let swap: &'static mut Swap<SwapState<_>> = SWAP.init_with(|| {
            Swap::new(
                SwapState {
                    fb: FB::alloc(&PSRAM_HEAP),
                    dirty: DirtyAreas::new(),
                    needs_full_redraw: DirtyAreas::new_full(),
                    #[cfg(feature = "damage-debug")]
                    debug_repaint: DirtyAreas::new(),
                    #[cfg(feature = "damage-debug")]
                    debug_damage: DirtyAreas::new(),
                    timings: Timings::default(),
                },
                SwapState {
                    fb: FB::alloc(&PSRAM_HEAP),
                    dirty: DirtyAreas::new(),
                    needs_full_redraw: DirtyAreas::new_full(),
                    #[cfg(feature = "damage-debug")]
                    debug_repaint: DirtyAreas::new(),
                    #[cfg(feature = "damage-debug")]
                    debug_damage: DirtyAreas::new(),
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
    #[cfg(feature = "damage-debug")]
    let mut previous_debug = Dirty::new();
    loop {
        let state = swap.get();
        let SwapState {
            fb,
            timings,
            dirty,
            needs_full_redraw: _,
            #[cfg(feature = "damage-debug")]
            debug_repaint,
            #[cfg(feature = "damage-debug")]
            debug_damage,
        } = state;

        let start = Instant::now();

        let is_first_flush = first_flush;
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

        #[cfg(feature = "damage-debug")]
        let mut flush_damage = dirty.clone();
        #[cfg(feature = "damage-debug")]
        flush_damage.extend(&previous_debug);
        #[cfg(not(feature = "damage-debug"))]
        let flush_damage = dirty.clone();

        if is_first_flush || flush_damage.is_full() {
            #[cfg(feature = "damage-debug")]
            let debug_full = debug_damage.is_full();
            #[cfg(not(feature = "damage-debug"))]
            let debug_full = false;
            fb.flush(&mut display, debug_full)
                .await
                .expect("display flush failed");
        } else {
            #[cfg(feature = "damage-debug")]
            for (region, overlay) in dirty.iter_merged_with_overlay(debug_damage, &previous_debug) {
                fb.flush_region(
                    &mut display,
                    region.top_left.x as u16,
                    region.top_left.y as u16,
                    region.size.width as u16,
                    region.size.height as u16,
                    (!overlay.is_zero_sized()).then_some(overlay),
                )
                .await
                .expect("display region flush failed");
            }

            #[cfg(not(feature = "damage-debug"))]
            for region in dirty.iter() {
                fb.flush_region(
                    &mut display,
                    region.top_left.x as u16,
                    region.top_left.y as u16,
                    region.size.width as u16,
                    region.size.height as u16,
                    None,
                )
                .await
                .expect("display region flush failed");
            }
        };

        #[cfg(feature = "damage-debug")]
        {
            // Full-screen transfers do not leave a region-specific overlay to clear.
            let current_debug = if dirty.is_full() {
                Dirty::new()
            } else {
                dirty.clone()
            };
            previous_debug = current_debug.clone();
            *debug_repaint = current_debug;
        }

        timings.spi_time = start.elapsed() - timings.vsync_wait;

        timings.swap_spi = prev_swap_spi;

        #[cfg(feature = "timing-log")]
        defmt::info!(
            "timing spi: vsync={}us flush={}us swap={}us",
            timings.vsync_wait.as_micros(),
            timings.spi_time.as_micros(),
            timings.swap_spi.as_micros(),
        );

        let before_swap = start.elapsed();
        swap.swap().await;
        prev_swap_spi = start.elapsed() - before_swap;
    }
}

#[embassy_executor::task]
async fn sensor_task(task: SensorTask) {
    let SensorTask {
        mut power,
        mut lora,
        mut gnss,
        mut nmea,
        mut nmea_parser,
        mut rtc,
        mut magnetometer,
        mut imu,
        accel_lsb_per_g,
        gyro_lsb_per_dps,
        mut state,
        mut rtc_sync_pending,
    } = task;

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

        if let Ok(irq) = lora.irq_flags().await {
            state.lora_irq = irq;
            println!("[LORA] sample IRQ=0x{irq:02X}");
        }
        match gnss.read_nmea_chunk(&mut nmea).await {
            Ok(data) => {
                state.gnss_bytes = data.len() as u16;
                let mut updates = 0;
                for &byte in data.iter() {
                    match nmea_parser.push(byte) {
                        Ok(Some(_)) => updates += 1,
                        Ok(None) => {}
                        Err(error) => println!("[GNSS] NMEA_PARSE_ERROR {error}"),
                    }
                }
                state.gnss = nmea_parser.state();
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
            Err(GnssError::I2c { operation, .. }) => match operation {
                GnssOperation::WriteConfig => println!("[GNSS] I2C_WRITE_CONFIG_FAILED"),
                GnssOperation::ReadLength => println!("[GNSS] I2C_READ_LENGTH_FAILED"),
                GnssOperation::ReadData => println!("[GNSS] I2C_READ_DATA_FAILED"),
                GnssOperation::WriteData => println!("[GNSS] I2C_WRITE_DATA_FAILED"),
            },
            Err(GnssError::BufferTooSmall { .. }) => {
                println!("[GNSS] NMEA_BUFFER_TOO_SMALL")
            }
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
        if magnetometer.data_ready().await.unwrap_or(false)
            && let Ok(data) = magnetometer.read_data().await
            && let Some(compensated) = magnetometer.compensate(&data)
        {
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
            state.magnetic_microtesla = [
                (compensated.x_microtesla * 1_000.0) as i32,
                (compensated.y_microtesla * 1_000.0) as i32,
                (compensated.z_microtesla * 1_000.0) as i32,
            ];
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
        if let Ok(sample) = imu.read_sync_sample(&mut embassy_time::Delay).await {
            if let Some(accel) = sample.accel {
                state.accel_micro_ms2 = [
                    accel_micro_ms2(accel.x, accel_lsb_per_g),
                    accel_micro_ms2(accel.y, accel_lsb_per_g),
                    accel_micro_ms2(accel.z, accel_lsb_per_g),
                ];
            }
            if let Some(gyro) = sample.gyro {
                state.gyro_micro_rad_s = [
                    gyro_micro_rad_s(gyro.x, gyro_lsb_per_dps),
                    gyro_micro_rad_s(gyro.y, gyro_lsb_per_dps),
                    gyro_micro_rad_s(gyro.z, gyro_lsb_per_dps),
                ];
            }
            state.imu_valid = sample.accel.is_some() || sample.gyro.is_some();
            println!(
                "[IMU] sample accel={:?} gyro={:?} si_accel={:?} si_gyro={:?}",
                sample.accel, sample.gyro, state.accel_micro_ms2, state.gyro_micro_rad_s
            );
        } else {
            state.imu_valid = false;
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

struct DrawCtx {
    touch_data: TouchData,
    selected_node: Option<u8>,
    screen: Screen,
    header_button: Button,
    touch_state: TouchState,
    peripherals: prototypes::PeripheralState,
    font_renderer: FontdueRenderer<'static, Color>,
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
    const LINE_BYTES: usize = core::mem::size_of::<fontdue::math::Line>();
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
        "[FONTDUE-BENCH] arm={} line_bytes={} px12={} px32={} px64={} checksums={},{},{}",
        ARM,
        LINE_BYTES,
        results[0].0,
        results[1].0,
        results[2].0,
        results[0].1,
        results[1].1,
        results[2].1
    );
    loop {
        println!(
            "[FONTDUE-BENCH] repeat arm={} line_bytes={} px12={} px32={} px64={} checksums={},{},{}",
            ARM,
            LINE_BYTES,
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

fn selected_node(touch_data: &TouchData) -> Option<u8> {
    let TouchData::Points(points) = touch_data else {
        return None;
    };
    points.iter().find_map(|point| {
        let x = point.x as i32;
        let y = point.y as i32;
        if (100..=164).contains(&x) && (146..=210).contains(&y) {
            Some(1)
        } else if (262..=326).contains(&x) && (206..=270).contains(&y) {
            Some(2)
        } else if (322..=386).contains(&x) && (284..=348).contains(&y) {
            Some(3)
        } else {
            None
        }
    })
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

fn draw<D>(ctx: &mut DrawCtx, target: &mut D) -> Dirty
where
    D: DrawTarget<Color = Color>,
    D::Error: core::fmt::Debug,
{
    octowhere::ui::prototypes::render(
        octowhere::ui::prototypes::ACTIVE_ARCHITECTURE,
        octowhere::ui::prototypes::State {
            screen: ctx.screen,
            selected_node: ctx.selected_node,
            peripherals: ctx.peripherals,
        },
        &ctx.font_renderer,
        target,
    )
    .expect("prototype renderer failed")
}

struct SwapState<A: Allocator = alloc::alloc::Global> {
    fb: Box<chrome::FB, A>,
    dirty: Dirty,
    needs_full_redraw: Dirty,
    #[cfg(feature = "damage-debug")]
    debug_repaint: Dirty,
    #[cfg(feature = "damage-debug")]
    debug_damage: Dirty,
    timings: Timings,
}

#[embassy_executor::task]
async fn async_main(spawner: Spawner) {
    // esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 72 * 1024);
    esp_alloc::heap_allocator!(size: 260 * 1024);

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
        let fonts = Box::leak(Box::new([
            &chrome::MarathonShapiroFont as &dyn fontdue::FontRepr,
            &chrome::FraktionMonoRegularFont as &dyn fontdue::FontRepr,
        ]));
        run_fontdue_target_benchmark(fonts[0]).await;
    }

    // The I²C pull-ups share VCC3V3 with the secondary board.
    Timer::after(Duration::from_millis(board::I2C_POWER_SETTLE_MS)).await;

    let i2c = I2c::new(
        peripherals.I2C0,
        esp_hal::i2c::master::Config::default().with_frequency(Rate::from_khz(400)),
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
    let exio_output_mask = (1 << board::EXIO_GPS_RESET) | (1 << board::EXIO_LORA_RESET);
    exio.write_direction(!exio_output_mask).await.unwrap();
    println!("[TCA9554] OK");

    let gps_reset = 1 << board::EXIO_GPS_RESET;
    let lora_reset = 1 << board::EXIO_LORA_RESET;
    exio.write_output(!(gps_reset | lora_reset)).await.unwrap();
    Timer::after(Duration::from_millis(10)).await;
    exio.write_output(!gps_reset).await.unwrap();
    Timer::after(Duration::from_micros(200)).await;
    exio.write_output(!(gps_reset | lora_reset)).await.unwrap();
    exio.write_direction(!lora_reset).await.unwrap();
    Timer::after(Duration::from_millis(10)).await;
    exio.write_output(!lora_reset).await.unwrap();
    let exio_direction = exio.read_direction().await.unwrap();
    println!("[TCA9554] direction=0x{exio_direction:02X}");
    Timer::after(Duration::from_secs(1)).await;

    let mut gnss = Lc76g::new(i2c.clone(), embassy_time::Delay);
    match gnss.enable_alp_mode().await {
        Ok(()) => println!("[GNSS] ALP_ENABLE_SENT"),
        Err(_) => println!("[GNSS] ALP_ENABLE_FAILED"),
    }
    let mut nmea = [0u8; 512];
    let mut nmea_parser = NmeaParser::new();
    let mut gnss_parse_ok = false;
    match gnss.read_nmea_chunk(&mut nmea).await {
        Ok(data) if !data.is_empty() => {
            println!("[GNSS] NMEA_CHUNK_OK bytes={}", data.len());
            for &byte in data {
                if let Err(error) = nmea_parser.push(byte) {
                    println!("[GNSS] NMEA_PARSE_ERROR {error}");
                }
            }
            gnss_parse_ok = nmea_parser.state().fix.is_some();
        }
        Ok(_) => println!("[GNSS] NMEA_EMPTY bytes=0"),
        Err(GnssError::I2c { operation, .. }) => match operation {
            GnssOperation::WriteConfig => println!("[GNSS] I2C_WRITE_CONFIG_FAILED"),
            GnssOperation::ReadLength => println!("[GNSS] I2C_READ_LENGTH_FAILED"),
            GnssOperation::ReadData => println!("[GNSS] I2C_READ_DATA_FAILED"),
            GnssOperation::WriteData => println!("[GNSS] I2C_WRITE_DATA_FAILED"),
        },
        Err(GnssError::BufferTooSmall { .. }) => println!("[GNSS] NMEA_BUFFER_TOO_SMALL"),
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
    let mut lora = Sx1272::new(lora_spi, lora_cs).expect("LoRa CS failed");
    let lora_probe = lora.probe_registers().await;
    let lora_result = match lora.identify().await {
        Ok(()) => 0,
        Err(LoraError::UnexpectedVersion(0)) => 10,
        Err(LoraError::UnexpectedVersion(0xFF)) => 11,
        Err(LoraError::UnexpectedVersion(_)) => 1,
        Err(LoraError::Spi(_)) => 2,
        Err(LoraError::ChipSelect(_)) => 3,
    };

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
            gnss,
            nmea: [0; 64],
            nmea_parser,
            rtc,
            magnetometer,
            imu,
            accel_lsb_per_g,
            gyro_lsb_per_dps,
            state: initial_sensor_state,
            rtc_sync_pending: true,
        })
        .unwrap(),
    );

    start_display_core!(peripherals, fb_st);

    let fonts = Box::leak(Box::new([
        &chrome::MarathonShapiroFont as &dyn fontdue::FontRepr,
        &chrome::FraktionMonoRegularFont as &dyn fontdue::FontRepr,
    ]));
    let font_renderer = FontdueRenderer::new(
        FontdueRendererCtx::new_rc(),
        20,
        chrome::WHITE,
        chrome::BLACK,
        fonts,
    );
    let mut draw_ctx = DrawCtx {
        touch_data: TouchData::default(),
        selected_node: None,
        screen: Screen::Map,
        header_button: Button::default(),
        touch_state: TouchState::default(),
        peripherals: prototypes::PeripheralState::default(),
        font_renderer,
    };
    draw_ctx.peripherals.pmic_valid = pmic_valid;
    draw_ctx.peripherals.battery_mv = initial_sensor_state.battery_mv;
    draw_ctx.peripherals.vbus_mv = initial_sensor_state.vbus_mv;
    draw_ctx.peripherals.vsys_mv = initial_sensor_state.vsys_mv;
    draw_ctx.peripherals.tca_valid = true;
    draw_ctx.peripherals.gnss_valid = gnss_parse_ok;
    draw_ctx.peripherals.lora_valid = lora_result == 0;
    draw_ctx.peripherals.compass_valid = bmm_ready && bmm_compensated;
    println!(
        "[MEM] internal_used={} psram_used={}",
        esp_alloc::HEAP.used(),
        PSRAM_HEAP.used(),
    );
    let mut prev_swap_draw = Duration::MIN;
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
                needs_full_redraw,
                #[cfg(feature = "damage-debug")]
                debug_repaint,
                #[cfg(feature = "damage-debug")]
                debug_damage,
            } = state;
            let fb = &mut **fb;
            let mut changed = Dirty::new();
            let previous_touch_points = draw_ctx.peripherals.touch_points;
            let previous_touch_position = draw_ctx.peripherals.touch_position;
            let previous_touch_positions = draw_ctx.peripherals.touch_positions;
            let touch_repoll_due =
                previous_touch_position.is_some() && last_touch_poll.elapsed() >= TOUCH_REPOLL;
            let wait_timeout = if touch_repoll_due {
                Duration::from_micros(0)
            } else if previous_touch_position.is_some() {
                TOUCH_REPOLL
            } else {
                Duration::from_millis(250)
            };
            let (touch_ready, sensor_state) = match select3(
                touch.wait_for_touch(),
                SENSOR_STATE.wait(),
                Timer::after(wait_timeout),
            )
            .await
            {
                Either3::First(result) => (result.is_ok(), None),
                Either3::Second(state) => (false, Some(state)),
                Either3::Third(()) => (touch_repoll_due, None),
            };
            if let Some(sensor_state) = sensor_state {
                draw_ctx.peripherals.battery_mv = sensor_state.battery_mv;
                draw_ctx.peripherals.vbus_mv = sensor_state.vbus_mv;
                draw_ctx.peripherals.vsys_mv = sensor_state.vsys_mv;
                draw_ctx.peripherals.gnss_bytes = sensor_state.gnss_bytes;
                draw_ctx.peripherals.gnss_valid = sensor_state.gnss.fix.is_some();
                draw_ctx.peripherals.lora_irq = sensor_state.lora_irq;
                draw_ctx.peripherals.clock = sensor_state.clock;
                draw_ctx.peripherals.accel_micro_ms2 = sensor_state.accel_micro_ms2;
                draw_ctx.peripherals.gyro_micro_rad_s = sensor_state.gyro_micro_rad_s;
                draw_ctx.peripherals.imu_valid = sensor_state.imu_valid;
                draw_ctx.peripherals.magnetic_microtesla = sensor_state.magnetic_microtesla;
                changed.make_full();
            };
            if touch_ready {
                draw_ctx.touch_data = touch.read_touch_data().await.unwrap();
                last_touch_poll = Instant::now();
            }
            let raw_touch_positions = match &draw_ctx.touch_data {
                TouchData::Points(points) => {
                    let mut positions = [None; 2];
                    for (slot, point) in points.iter().take(2).enumerate() {
                        positions[slot] = Some(Point::new(point.x as i32, point.y as i32));
                    }
                    positions
                }
                TouchData::CoverGesture => [None; 2],
            };
            let (touch_points, touch_positions) =
                draw_ctx.touch_state.update_positions(raw_touch_positions);
            let touch_position = touch_positions[0];
            draw_ctx.peripherals.touch_points = touch_points;
            draw_ctx.peripherals.touch_position = touch_position;
            draw_ctx.peripherals.touch_positions = touch_positions;
            let (touch_active, header_hit) = match &draw_ctx.touch_data {
                TouchData::Points(points) => (
                    !points.is_empty(),
                    points.iter().any(|point| {
                        (108..=366).contains(&(point.x as i32))
                            && (48..=98).contains(&(point.y as i32))
                    }),
                ),
                TouchData::CoverGesture => (false, false),
            };
            if draw_ctx
                .header_button
                .update_touch(touch_active, header_hit)
                == ButtonEvent::Pressed
            {
                draw_ctx.screen = draw_ctx.screen.next();
                draw_ctx.selected_node = None;
                changed.make_full();
            }
            let previous_selected_node = draw_ctx.selected_node;
            if let Some(node) = selected_node(&draw_ctx.touch_data) {
                draw_ctx.selected_node = Some(node);
            }
            if draw_ctx.selected_node != previous_selected_node {
                changed.make_full();
            }
            if draw_ctx.screen == Screen::Touch
                && (touch_points != previous_touch_points
                    || touch_position != previous_touch_position
                    || touch_positions != previous_touch_positions)
            {
                changed.make_full();
            }
            dirty.clear();

            let mut repaint = needs_full_redraw.clone();
            repaint.extend(&changed);
            #[cfg(feature = "damage-debug")]
            {
                repaint.extend(debug_repaint);
                debug_repaint.clear();
            }

            // esp_println::dbg!(&needs_full_redraw);
            if repaint.is_full() {
                draw(&mut draw_ctx, fb);
                dirty.make_full();
            } else {
                for area in repaint.iter() {
                    let mut clipped = fb.clipped(&area);
                    dirty.extend(&draw(&mut draw_ctx, &mut clipped));
                }
            }
            dirty.extend(&repaint);
            #[cfg(feature = "damage-debug")]
            {
                *debug_damage = changed.clone();
            }
            *needs_full_redraw = changed;

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
