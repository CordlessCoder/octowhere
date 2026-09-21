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
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embassy_time::{Duration, Instant};
use embedded_graphics::{
    prelude::*,
    primitives::{Circle, Rectangle},
};
use esp_hal::{
    dma_tx_buffer,
    gpio::{Input, InputConfig, Level, Output, OutputConfig},
    i2c::master::I2c,
    peripherals, spi,
    time::Rate,
    timer::timg::TimerGroup,
};
use esp_println::println;
use octowhere::{
    board,
    chrome::{self, Color, Dirty, FB, FontdueRenderer, FontdueRendererCtx},
    drivers::{co5300::Co5300Display, framebuffer::Framebuffer, qspi_bus::QspiBus},
    peripherals::{
        rtc::Pcf85063aRtc,
        touch::{Cst9217, Cst9217Config, TouchData},
    },
    ui::dirty::DirtyAreas,
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
pub static PSRAM_HEAP: esp_alloc::EspHeap = esp_alloc::EspHeap::empty();

// PERF: The unit of scheduling for embassy is a task, not an async Future - it may be beneficial to
// move some Futures into their own tasks.
#[embassy_executor::task]
async fn second_core(
    _spawner: Spawner,
    gpio0: esp_hal::peripherals::GPIO0<'static>,
    io: SecondCore<&'static esp_alloc::EspHeap>,
) {
    // let mut gpio0 = Input::new(gpio0, InputConfig::default());
    // let gpio0 = async {
    //     loop {
    //         gpio0.wait_for_any_edge().await;
    //         println!("GPIO0: {:?}", gpio0.level());
    //     }
    // };
    // gpio0.await;
    // embassy_futures::join::join(gpio0, async {}).await;
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

    let reset = Output::new(gpio39, Level::High, OutputConfig::default());
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
    let mut display = Co5300Display::new(spi, reset, te, dma_tx, dma_tx_swap).await;
    display.set_brightness(120);

    println!("[DISPLAY] OK");

    let mut prev_swap_spi = Duration::MIN;
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

        display.wait_for_vsync().await;

        timings.vsync_wait = start.elapsed();

        #[cfg(feature = "damage-debug")]
        let mut flush_damage = dirty.clone();
        #[cfg(feature = "damage-debug")]
        flush_damage.extend(&previous_debug);
        #[cfg(not(feature = "damage-debug"))]
        let flush_damage = dirty.clone();

        if flush_damage.is_full() {
            #[cfg(feature = "damage-debug")]
            let debug_full = debug_damage.is_full();
            #[cfg(not(feature = "damage-debug"))]
            let debug_full = false;
            fb.flush(&mut display, debug_full).await;
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
                .await;
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
                .await;
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
    font_renderer: FontdueRenderer<'static, Color>,
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
            selected_node: ctx.selected_node,
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

#[esp_rtos::main]
async fn main(_spawner: Spawner) {
    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 72 * 1024);
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

    let i2c = I2c::new(
        peripherals.I2C0,
        esp_hal::i2c::master::Config::default().with_frequency(Rate::from_khz(400)),
    )
    .expect("I2C failed")
    .with_scl(peripherals.GPIO14)
    .with_sda(peripherals.GPIO15)
    .into_async();

    let i2c = Mutex::<NoopRawMutex, _>::new(i2c);
    let i2c = I2cDevice::new(&i2c);

    let gyro_range = ph_qmi8658::GyroRange::Dps512;
    let accel_range = ph_qmi8658::AccelRange::G2;

    let mut exio = Tca9554::new(i2c.clone(), tca9554::Address::standard());

    exio.init().await.unwrap();

    // let mut power = Axp2101Power::new(i2c.clone());
    // power.init().await.unwrap();
    // power.trim_adc_channels().await.unwrap();

    let mut rtc = Pcf85063aRtc::new(i2c.clone());
    rtc.init().await.unwrap();
    println!("[RTC] OK");

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
        ph_qmi8658::I2cConfig::new(0x6B),
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

    println!("[IMU] INIT");

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

    let spi_config = spi::master::Config::default()
        .with_frequency(Rate::from_mhz(80))
        .with_mode(spi::Mode::_0);

    // let dma_tx = dma_tx_buffer!(4095 * 2).unwrap();
    // let dma_tx_swap = dma_tx_buffer!(4095 * 2).unwrap();
    //
    // let reset = Output::new(peripherals.GPIO39, Level::High, OutputConfig::default());
    // let te = Input::new(peripherals.GPIO13, InputConfig::default());
    // let cs = Output::new(peripherals.GPIO12, Level::High, OutputConfig::default());
    //
    // let spi = spi::master::Spi::new(peripherals.SPI2, spi_config)
    //     .expect("SPI failed")
    //     .with_sck(peripherals.GPIO38)
    //     .with_sio0(peripherals.GPIO4)
    //     .with_sio1(peripherals.GPIO5)
    //     .with_sio2(peripherals.GPIO6)
    //     .with_sio3(peripherals.GPIO7)
    //     .with_dma(peripherals.DMA_CH0)
    //     .into_async();
    // let spi = QspiBus::new(spi, dma_tx, cs);
    // let mut display = Co5300Display::new(spi, reset, te, dma_tx_swap).await;
    // display.set_brightness(120);
    // display.fill_screen(chrome::BLACK);

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
    let (mut fb_st, second_core_swap) = swap.split();
    // PERF: We want to utilize cores fairly evenly because running both uses little more power
    // than running just one.

    esp_rtos::start_second_core(
        peripherals.CPU_CTRL.reborrow(),
        peripherals.FROM_CPU_INTR1,
        // SAFETY: This static mut value must not be accessed ever again, anywhere
        unsafe { &mut CORE1_STACK },
        || {
            let executor = CORE1_EXECUTOR.init_with(esp_rtos::embassy::Executor::new);
            let io = SecondCore {
                gpio4: peripherals.GPIO4,
                gpio5: peripherals.GPIO5,
                gpio6: peripherals.GPIO6,
                gpio7: peripherals.GPIO7,
                gpio12: peripherals.GPIO12,
                gpio13: peripherals.GPIO13,
                gpio38: peripherals.GPIO38,
                gpio39: peripherals.GPIO39,
                dma_ch0: peripherals.DMA_CH2,
                spi2: peripherals.SPI2,
                swap: second_core_swap,
            };
            executor.run(|spawner| {
                spawner.spawn(second_core(spawner, peripherals.GPIO0, io).unwrap());
            })
        },
    );

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
        font_renderer,
    };
    println!(
        "[MEM] internal_used={} psram_used={}",
        esp_alloc::HEAP.used(),
        PSRAM_HEAP.used(),
    );
    let mut prev_swap_draw = Duration::MIN;
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
            draw_ctx.touch_data = touch.read_touch_data().await.unwrap();
            let previous_selected_node = draw_ctx.selected_node;
            if let Some(node) = selected_node(&draw_ctx.touch_data) {
                draw_ctx.selected_node = Some(node);
            }
            if draw_ctx.selected_node != previous_selected_node {
                needs_full_redraw.make_full();
            }
            dirty.clear();

            let mut repaint = needs_full_redraw.clone();
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
            let mut next_needs_full_redraw = Dirty::new();
            dirty.extend(&next_needs_full_redraw);

            #[cfg(feature = "damage-debug")]
            {
                *debug_damage = next_needs_full_redraw.clone();
            }
            *needs_full_redraw = next_needs_full_redraw;

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
