#![feature(impl_trait_in_assoc_type)]
#![feature(type_alias_impl_trait)]
#![feature(allocator_api)]
#![no_std]
#![no_main]
// Now defaults to deny in Rust-2024, however abusing statics is necessary in the embedded world.
#![expect(static_mut_refs)]
#![expect(unused)]
#![warn(unused_must_use)]
extern crate alloc;

use alloc::{alloc::Allocator, boxed::Box, rc::Rc};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embassy_time::{Duration, Instant, Ticker};
use embedded_graphics::{
    prelude::*,
    primitives::{Circle, Rectangle},
};
use esp_hal::{
    dma_tx_buffer,
    gpio::{Input, InputConfig, Level, Output, OutputConfig},
    i2c::master::I2c,
    interrupt::software::SoftwareInterruptControl,
    peripherals, spi,
    time::Rate,
    timer::timg::TimerGroup,
};
use esp_println::println;
use fontdue::FontRepr;
use octowhere::{
    board,
    chrome::{
        self, Color, FontdueRenderer, FontdueRendererCtx, FraktionMonoRegularFont,
        HEADING_FONT_FAST, MEDIUM_FONT_FAST, MarathonShapiroFont, UPSCALE,
    },
    drivers::{co5300::Co5300Display, framebuffer::Framebuffer, qspi_bus::QspiBus},
    peripherals::{
        power::Axp2101Power,
        rtc::Pcf85063aRtc,
        touch::{Cst9217, Cst9217Config, TouchData},
    },
    ui::dirty::DirtyAreas,
    util::{Swap, SwapThread, fill_buf_repeat},
};
use static_cell::StaticCell;

use esp_alloc as _;
use esp_backtrace as _;
use tca9554::Tca9554;
use u8g2_fonts::{FontRenderer, types::FontColor};
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
    dma_ch0: peripherals::DMA_CH0<'static>,
    spi2: peripherals::SPI2<'static>,
    swap: SwapThread<'static, SwapState<A>>,
}

const SCALED_WIDTH: usize = octowhere::board::LCD_WIDTH as usize / UPSCALE;
const SCALED_HEIGHT: usize = octowhere::board::LCD_HEIGHT as usize / UPSCALE;

type FB = Framebuffer<
    UPSCALE,
    {
        octowhere::drivers::framebuffer::buffer_size::<Color>(
            octowhere::board::LCD_WIDTH as usize / UPSCALE,
            octowhere::board::LCD_HEIGHT as usize / UPSCALE,
        )
    },
    { octowhere::board::LCD_WIDTH as usize / UPSCALE },
    { octowhere::board::LCD_HEIGHT as usize / UPSCALE },
    Color,
>;

static mut CORE1_STACK: esp_hal::system::Stack<8192> = esp_hal::system::Stack::new();
static CORE1_EXECUTOR: StaticCell<esp_rtos::embassy::Executor> = StaticCell::new();
static CORE1_INT: StaticCell<esp_rtos::embassy::Executor> = StaticCell::new();
static SWAP: StaticCell<Swap<SwapState<&esp_alloc::EspHeap>>> = StaticCell::new();
pub static PSRAM_HEAP: esp_alloc::EspHeap = esp_alloc::EspHeap::empty();

#[embassy_executor::task]
async fn second_core(
    _spawner: Spawner,
    gpio0: esp_hal::peripherals::GPIO0<'static>,
    io: SecondCore<&'static esp_alloc::EspHeap>,
) {
    let mut gpio0 = Input::new(gpio0, InputConfig::default());
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

    let dma_tx = dma_tx_buffer!(4095 * 2).unwrap();
    let dma_tx_swap = dma_tx_buffer!(4095 * 2).unwrap();

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
    let spi = QspiBus::new(spi, dma_tx, cs);
    let mut display = Co5300Display::new(spi, reset, te, dma_tx_swap).await;
    display.set_brightness(120);

    println!("[DISPLAY] OK");

    let mut prev_swap_spi = Duration::MIN;
    loop {
        let state = swap.get();
        let SwapState {
            fb,
            timings:
                Timings {
                    vsync_wait,
                    spi_time,
                    swap_draw,
                    swap_spi,
                    frametime,
                },
            dirty,
            needs_full_redraw,
        } = state;

        let start = Instant::now();

        display.wait_for_vsync().await;

        *vsync_wait = start.elapsed();

        if dirty.is_full() {
            fb.flush(&mut display, |pixels| {
                // fill_buf_repeat(pixels, &[0], pixels.len());
            })
            .await;
        } else {
            for region in dirty.iter() {
                fb.flush_region(
                    &mut display,
                    region.top_left.x as u16,
                    region.top_left.y as u16,
                    region.size.width as u16,
                    region.size.height as u16,
                    |pixels| {
                        // fill_buf_repeat(pixels, &[0], pixels.len());
                    },
                )
                .await;
            }
            // fb.clear_color(chrome::BLACK);
        };

        *spi_time = start.elapsed() - *vsync_wait;

        *swap_spi = prev_swap_spi;
        let before_swap = start.elapsed();
        swap.swap().await;
        prev_swap_spi = start.elapsed() - before_swap;
    }
}

fn bench10<R>(mut the_thing: impl FnMut() -> R, name: &str) -> (R, Duration) {
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

#[expect(clippy::too_many_arguments)]
async fn bench_font<
    const UPSCALE: usize,
    const N: usize,
    const WIDTH: usize,
    const HEIGHT: usize,
    C,
>(
    mut display: Option<&mut Co5300Display<'_, C>>,
    fb: &mut Framebuffer<UPSCALE, N, WIDTH, HEIGHT, C>,
    fg: C,
    bg: C,
    desc: &str,
    u8g2_renderer: u8g2_fonts::FontRenderer,
    fontdue: FontdueRenderer<'_, C>,
    delay: u32,
) where
    <C as embedded_graphics::pixelcolor::raw::ToBytes>::Bytes: core::convert::AsRef<[u8]>,
    C: Into<embedded_graphics::pixelcolor::Rgb888>
        + From<embedded_graphics::pixelcolor::Rgb888>
        + core::fmt::Debug
        + octowhere::drivers::co5300::Co5300ColorMode,
{
    let top_left = Point::new(10, 150);
    let test_text = "abcdefghijklmnopqrstuvwxyz\n\
                           ABCDEFGHIJKLMNOPQRSTUVWXYZ\n\
                           1234567890`~!@#$%^&*()_+/[]";
    let ext_renderer =
        u8g2_fonts::FontRenderer::new::<u8g2_fonts::fonts::u8g2_font_spleen12x24_mr>();
    let ext_pos = Point::new(100, 340);
    println!("{desc}");
    fb.clear_color(bg);
    let (_, took) = bench10(
        || {
            u8g2_renderer
                .render(
                    test_text,
                    top_left,
                    u8g2_fonts::types::VerticalPosition::Top,
                    u8g2_fonts::types::FontColor::Transparent(fg),
                    fb,
                )
                .unwrap()
        },
        "- u8g2",
    );
    if let Some(display) = display.as_mut() {
        ext_renderer
            .render(
                format_args!("{desc}\nu8g2: {:.1}ms", took.as_micros() as f32 / 1_000.,),
                ext_pos,
                u8g2_fonts::types::VerticalPosition::Top,
                u8g2_fonts::types::FontColor::Transparent(fg),
                fb,
            )
            .unwrap();
        fb.flush(display, |_| ()).await;
        board::delay_ms_async(delay).await;
    }
    fb.clear_color(bg);
    let (_, took) = bench10(
        || {
            embedded_graphics::text::Text::new(test_text, top_left, fontdue.clone())
                .draw(fb)
                .unwrap()
        },
        "- fontdue",
    );
    if let Some(display) = display.as_mut() {
        ext_renderer
            .render(
                format_args!("{desc}\nfontdue: {:.1}ms", took.as_micros() as f32 / 1_000.,),
                ext_pos,
                u8g2_fonts::types::VerticalPosition::Top,
                u8g2_fonts::types::FontColor::Transparent(fg),
                fb,
            )
            .unwrap();
        fb.flush(display, |_| ()).await;
        board::delay_ms_async(delay).await;
    }
}

struct DrawCtx {
    touch_data: TouchData,
    bounding_box: Rectangle,
}

#[derive(Debug, Default)]
struct Timings {
    vsync_wait: Duration,
    spi_time: Duration,
    swap_draw: Duration,
    swap_spi: Duration,
    frametime: Duration,
}

fn update_text<D>(ctx: &DrawCtx, timings: &Timings, target: &mut D) -> Dirty
where
    D: DrawTarget,
    D: DrawTarget<Color = Color>,
    D::Error: core::fmt::Debug,
{
    let mut dirty = Dirty::new();
    let Timings {
        vsync_wait,
        spi_time,
        swap_draw,
        swap_spi,
        frametime,
    } = timings;
    let text = format_args!(
        // let text = alloc::format!(
        "draw: {:.1}ms\nvsync: {:.1}ms\nspi+clear: {:.1}ms\nswap(draw): {:.1}ms\nswap(spi): {:.1}ms",
        frametime.as_micros() as f32 / 1_000.,
        vsync_wait.as_micros() as f32 / 1_000.,
        spi_time.as_micros() as f32 / 1_000.,
        swap_draw.as_micros() as f32 / 1_000.,
        swap_spi.as_micros() as f32 / 1_000.,
    );
    let dim = HEADING_FONT_FAST
        .render(
            "Statistics",
            (ctx.bounding_box.center() + Point::new(-100, 60)).scale(),
            u8g2_fonts::types::VerticalPosition::Bottom,
            FontColor::Transparent(chrome::WHITE),
            target,
        )
        .unwrap();
    _ = dim.bounding_box.map(|rect| dirty.add(rect));
    let dim = MEDIUM_FONT_FAST
        .render(
            text,
            (ctx.bounding_box.center() + Point::new(-100, 60)).scale(),
            u8g2_fonts::types::VerticalPosition::Top,
            FontColor::Transparent(chrome::WHITE),
            target,
        )
        .unwrap();
    _ = dim.bounding_box.map(|rect| dirty.add(rect));
    // fontdue_renderer
    //     .render(
    //         |layout, fonts| {
    //             layout.append(
    //                 fonts,
    //                 &fontdue::layout::TextStyle::new("Statistics\n", 32.0, 0),
    //             );
    //             layout.append(fonts, &fontdue::layout::TextStyle::new(&text, 24.0, 1));
    //         },
    //         (bbox.center() + Point::new(-100, 10)).scale(),
    //         fb,
    //     )
    //     .unwrap();
    dirty
}

fn update_touch<D>(ctx: &DrawCtx, target: &mut D) -> Dirty
where
    D: DrawTarget<Color = Color>,
    D: DrawTarget,
    D::Error: core::fmt::Debug,
{
    use embedded_graphics::{
        prelude::*,
        primitives::{
            Circle, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, StrokeAlignment, Triangle,
        },
        text::{Alignment, Text},
    };
    let mut dirty = Dirty::new();

    // Create styles used by the drawing operations.
    let thin_stroke = PrimitiveStyle::with_stroke(chrome::PURPLE, 4 / UPSCALE as u32);
    let thick_stroke = PrimitiveStyle::with_stroke(chrome::RED, 8 / UPSCALE as u32);
    let border_stroke = PrimitiveStyleBuilder::new()
        .stroke_color(if ctx.touch_data != TouchData::CoverGesture {
            chrome::LIME
        } else {
            chrome::RED
        })
        .stroke_width(8 / UPSCALE as u32)
        .stroke_alignment(StrokeAlignment::Inside)
        .build();
    let fill = PrimitiveStyle::with_fill(Color::CSS_GRAY);

    let yoffset = 140 / UPSCALE as i32;

    match &ctx.touch_data {
        TouchData::Points(points) => {
            let size = 64 / UPSCALE;
            for point in points {
                let prim = Circle::with_center(
                    Point::new(point.x as i32, point.y as i32).scale(),
                    size as u32,
                )
                .into_styled(
                    PrimitiveStyleBuilder::new()
                        .fill_color(Color::CSS_CYAN)
                        .build(),
                );
                prim.draw(target).unwrap();
                dirty.add(prim.bounding_box());
            }
        }
        TouchData::CoverGesture => {}
    }
    dirty
}

fn draw<D>(ctx: &mut DrawCtx, timings: &Timings, target: &mut D) -> Dirty
where
    D: DrawTarget<Color = Color>,
    D: DrawTarget,
    D::Error: core::fmt::Debug,
{
    let mut dirty = Dirty::new();
    use embedded_graphics::{
        prelude::*,
        primitives::{
            Circle, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, StrokeAlignment, Triangle,
        },
        text::{Alignment, Text},
    };

    target
        .fill_solid(&target.bounding_box(), chrome::BLACK)
        .unwrap();
    dirty.add(target.bounding_box());

    let Timings {
        vsync_wait,
        spi_time,
        swap_draw,
        swap_spi,
        frametime,
    } = timings;

    // Create styles used by the drawing operations.
    let thin_stroke = PrimitiveStyle::with_stroke(chrome::PURPLE, 4 / UPSCALE as u32);
    let thick_stroke = PrimitiveStyle::with_stroke(chrome::RED, 8 / UPSCALE as u32);
    let border_stroke = PrimitiveStyleBuilder::new()
        .stroke_color(if ctx.touch_data != TouchData::CoverGesture {
            chrome::LIME
        } else {
            chrome::RED
        })
        .stroke_width(8 / UPSCALE as u32)
        .stroke_alignment(StrokeAlignment::Inside)
        .build();
    let fill = PrimitiveStyle::with_fill(Color::CSS_GRAY);

    let yoffset = 140 / UPSCALE as i32;

    // // Draw a 3px wide outline around the display.
    // Circle::new(Point::new(0, 0), ctx.bounding_box.size.width)
    //     .scale()
    //     .into_styled(border_stroke)
    //     .draw(target)
    //     .unwrap();

    // PERF: Currently, embedded_graphics primitives completely ignore the target's bounding box,
    // attempting to draw even if it would go out of bounds, wasting compute.

    // // Draw a triangle.
    // Triangle::new(
    //     Point::new(56, 64 + yoffset).scale(),
    //     Point::new(56 + 64, 64 + yoffset).scale(),
    //     Point::new(56 + 32, yoffset).scale(),
    // )
    // .into_styled(thin_stroke)
    // .draw(target)
    // .unwrap();

    // // Draw a filled square
    // Rectangle::new(Point::new(200, yoffset), Size::new(64, 64))
    //     .scale()
    //     .into_styled(fill)
    //     .draw(target)
    //     .unwrap();
    //
    // // Draw a circle with a 3px wide stroke.
    // Circle::new(Point::new(340, yoffset), 68)
    //     .scale()
    //     .into_styled(thick_stroke)
    //     .draw(target)
    //     .unwrap();

    dirty
}

trait Scalable {
    fn scale(self) -> Self;
}
impl Scalable for Point {
    fn scale(self) -> Self {
        self.component_div(Point::new_equal(UPSCALE as i32))
    }
}
impl Scalable for Size {
    fn scale(self) -> Self {
        self.component_div(Size::new_equal(UPSCALE as u32))
    }
}
impl Scalable for Rectangle {
    fn scale(self) -> Self {
        let top_left = self.top_left.scale();
        let size = self.size.scale();
        Rectangle::new(top_left, size)
    }
}
impl Scalable for Circle {
    fn scale(self) -> Self {
        let top_left = self.top_left.scale();
        let diameter = self.diameter / UPSCALE as u32;
        Circle::new(top_left, diameter)
    }
}

type Dirty = DirtyAreas<SCALED_WIDTH, SCALED_HEIGHT, 2, 2, { 2 * 2 }>;
struct SwapState<A: Allocator = alloc::alloc::Global> {
    fb: Box<FB, A>,
    dirty: Dirty,
    needs_full_redraw: Dirty,
    timings: Timings,
}

#[esp_rtos::main]
async fn main(_spawner: Spawner) {
    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 72 * 1024);
    esp_alloc::heap_allocator!(size: 280 * 1024);

    // PERF: How low do we want to drop the clock speed?
    let mut peripherals =
        esp_hal::init(esp_hal::Config::default().with_cpu_clock(esp_hal::clock::CpuClock::_240MHz));
    let sw_int: SoftwareInterruptControl<'static> =
        SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);

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

    esp_rtos::start(timg0.timer0, sw_int.software_interrupt0);

    esp_println::logger::init_logger_from_env();

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

    let mut power = Axp2101Power::new(i2c.clone());
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

    let mut fb = Framebuffer::<
        1,
        { octowhere::drivers::framebuffer::buffer_size::<Color>(466, 466) },
        466,
        466,
        Color,
    >::alloc(&PSRAM_HEAP);
    let swap: &'static mut Swap<SwapState<_>> = SWAP.init_with(|| {
        Swap::new(
            SwapState {
                fb: Box::clone(&fb),
                dirty: DirtyAreas::new(),
                needs_full_redraw: DirtyAreas::new_full(),
                timings: Timings::default(),
            },
            SwapState {
                fb,
                dirty: DirtyAreas::new(),
                needs_full_redraw: DirtyAreas::new_full(),
                timings: Timings::default(),
            },
        )
    });
    let (mut fb_st, second_core_swap) = swap.split();
    // PERF: We want to utilize cores fairly evenly because running both uses little more power
    // than running just one.
    esp_rtos::start_second_core(
        peripherals.CPU_CTRL.reborrow(),
        sw_int.software_interrupt1,
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
                dma_ch0: peripherals.DMA_CH0,
                spi2: peripherals.SPI2,
                swap: second_core_swap,
            };
            executor.run(|spawner| {
                spawner.spawn(second_core(spawner, peripherals.GPIO0, io).unwrap());
            })
        },
    );

    // let fontdue_ctx = FontdueRendererCtx::new_rc();
    // let fontdue_renderer = FontdueRenderer::new(
    //     fontdue_ctx,
    //     32,
    //     chrome::WHITE,
    //     chrome::BLACK,
    //     &[&MarathonShapiroFont, &FraktionMonoRegularFont],
    // );
    let mut draw_ctx = DrawCtx {
        touch_data: TouchData::default(),
        bounding_box: chrome::DISPLAY_BBOX,
    };
    let mut prev_swap_draw = Duration::MIN;
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
            } = state;
            let fb = &mut **fb;
            draw_ctx.touch_data = touch.read_touch_data().await.unwrap();
            dirty.clear();

            // esp_println::dbg!(&needs_full_redraw);
            if needs_full_redraw.is_full() {
                draw(&mut draw_ctx, timings, fb);
                dirty.make_full();
            } else {
                for area in needs_full_redraw.iter() {
                    let mut clipped = fb.clipped(&area);
                    dirty.extend(&draw(&mut draw_ctx, timings, &mut clipped));
                }
            }
            dirty.extend(needs_full_redraw);
            *needs_full_redraw = update_text(&draw_ctx, timings, fb);
            needs_full_redraw.extend(&update_touch(&draw_ctx, fb));

            dirty.extend(needs_full_redraw);

            timings.frametime = start.elapsed();
            timings.swap_draw = prev_swap_draw;
        }

        let start = Instant::now();
        fb_st.swap().await;
        prev_swap_draw = start.elapsed();
    }
}
