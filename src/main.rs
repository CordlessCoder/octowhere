#![feature(impl_trait_in_assoc_type)]
#![no_std]
#![no_main]
// Now defaults to deny in Rust-2024, however abusing statics is necessary in the embedded world.
#![expect(static_mut_refs)]
#![expect(unused)]
extern crate alloc;

use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embassy_time::{Duration, Instant, Ticker};
use embedded_bitmap_fonts::TextStyle;
use embedded_graphics::{
    prelude::*,
    primitives::{Circle, Rectangle},
};
use esp_hal::{
    dma_tx_buffer,
    gpio::{Input, InputConfig, Level, Output, OutputConfig},
    i2c::master::I2c,
    interrupt::software::SoftwareInterruptControl,
    spi,
    time::Rate,
    timer::timg::TimerGroup,
};
use esp_println::println;
use octowhere::{
    color::{self, Color},
    drivers::{co5300::Co5300Display, framebuffer::Framebuffer, qspi_bus::QspiBus},
    peripherals::{
        power::Axp2101Power,
        rtc::Pcf85063aRtc,
        touch::{Cst9217, Cst9217Config, TouchData},
    },
};
use static_cell::StaticCell;

use esp_alloc as _;
use esp_backtrace as _;
use tca9554::Tca9554;
esp_bootloader_esp_idf::esp_app_desc!();

static mut CORE1_STACK: esp_hal::system::Stack<8192> = esp_hal::system::Stack::new();
static CORE1_EXECUTOR: StaticCell<esp_rtos::embassy::Executor> = StaticCell::new();

#[embassy_executor::task]
async fn second_core(_spawner: Spawner, gpio0: esp_hal::peripherals::GPIO0<'static>) {
    let mut gpio0 = Input::new(gpio0, InputConfig::default());
    let gpio0 = async {
        loop {
            gpio0.wait_for_any_edge().await;
            println!("GPIO0: {:?}", gpio0.level());
        }
    };
    gpio0.await;
    // embassy_futures::join::join(gpio0, async {}).await;
}

fn bench<R>(the_thing: impl FnOnce() -> R, name: &str) -> R {
    let start = Instant::now();
    let ret = the_thing();
    let took = start.elapsed();
    println!("{name}: {:.1}ms", took.as_micros() as f32 / 1_000.,);
    ret
}

const UPSCALE: usize = 2;
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

#[esp_rtos::main]
async fn main(_spawner: Spawner) {
    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 72 * 1024);
    esp_alloc::heap_allocator!(size: 128 * 1024);

    // PERF: How low do we want to drop the clock speed?
    let mut peripherals =
        esp_hal::init(esp_hal::Config::default().with_cpu_clock(esp_hal::clock::CpuClock::_240MHz));
    let sw_int = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);

    let psram_config = esp_hal::psram::PsramConfig {
        mode: esp_hal::psram::PsramMode::OctalSpi,
        size: esp_hal::psram::PsramSize::AutoDetect,
        core_clock: None,
        flash_frequency: esp_hal::psram::FlashFreq::FlashFreq80m,
        ram_frequency: esp_hal::psram::SpiRamFreq::Freq80m,
    };
    esp_alloc::psram_allocator!(peripherals.PSRAM, esp_hal::psram, psram_config);

    let timg0 = TimerGroup::new(peripherals.TIMG0);

    esp_rtos::start(timg0.timer0, sw_int.software_interrupt0);

    // PERF: We want to utilize cores fairly evenly because running both uses little more power
    // than running just one.
    esp_rtos::start_second_core(
        peripherals.CPU_CTRL.reborrow(),
        sw_int.software_interrupt1,
        // SAFETY: This static mut value must not be accessed ever again, anywhere
        unsafe { &mut CORE1_STACK },
        || {
            let executor = CORE1_EXECUTOR.init_with(esp_rtos::embassy::Executor::new);
            executor.run(|spawner| {
                spawner.spawn(second_core(spawner, peripherals.GPIO0).unwrap());
            })
        },
    );

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

    // FIXME: May not be actually routed anywhere on the waveshare board we're using, in which case
    // this has to go :(
    let mut exio = Tca9554::new(i2c.clone(), tca9554::Address::standard());

    let i2c_peripherals = async {
        exio.init().await.unwrap();

        let mut power = Axp2101Power::new(i2c.clone());
        power.init().await.unwrap();
        power.trim_adc_channels().await.unwrap();

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

        (power, rtc, imu, touch)
    };

    let display = async {
        let spi_config = spi::master::Config::default()
            .with_frequency(Rate::from_mhz(80))
            .with_mode(spi::Mode::_0);

        let dma_tx = dma_tx_buffer!(4095 * 2).unwrap();
        let dma_tx_swap = dma_tx_buffer!(4095 * 2).unwrap();

        let reset = Output::new(peripherals.GPIO39, Level::High, OutputConfig::default());
        let te = Input::new(peripherals.GPIO13, InputConfig::default());
        let cs = Output::new(peripherals.GPIO12, Level::High, OutputConfig::default());

        let spi = spi::master::Spi::new(peripherals.SPI2, spi_config)
            .expect("SPI failed")
            .with_sck(peripherals.GPIO38)
            .with_sio0(peripherals.GPIO4)
            .with_sio1(peripherals.GPIO5)
            .with_sio2(peripherals.GPIO6)
            .with_sio3(peripherals.GPIO7)
            .with_dma(peripherals.DMA_CH0)
            .into_async();
        let spi = QspiBus::new(spi, dma_tx, cs);
        let mut display = Co5300Display::new(spi, reset, te, dma_tx_swap).await;

        println!("[DISPLAY] OK");

        // delay_ms_async(100).await;

        let mut fb = Framebuffer::<
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
        >::alloc();
        fb.clear_color(Color::CSS_DIM_GRAY);
        fb.flush(&mut display).await;
        println!("[FB] OK");
        (display, fb)
    };

    let ((power, _rtc, _imu, mut touch), (mut display, mut fb)) =
        embassy_futures::join::join(i2c_peripherals, display).await;
    // let (mut display, mut fb) = display.await;
    display.set_brightness(120);

    fb.clear_color(Color::BLACK);
    fb.flush(&mut display).await;

    // let mut ticker = Ticker::every(Duration::from_millis(1000 / 60));
    // let mut prev_touch_data = TouchData::Points(heapless::Vec::new());
    // let mut touch_data = TouchData::Points(heapless::Vec::new());
    // loop {
    //     let start = Instant::now();
    //
    //     use embedded_graphics::{
    //         prelude::*,
    //         primitives::{Circle, PrimitiveStyle, PrimitiveStyleBuilder, StrokeAlignment},
    //     };
    //     // Create styles used by the drawing operations.
    //     let thin_stroke = PrimitiveStyle::with_stroke(Color::BLUE, 4);
    //     let thick_stroke = PrimitiveStyle::with_stroke(Color::RED, 8);
    //     let border_stroke = PrimitiveStyleBuilder::new()
    //         .stroke_color(if touch_data != TouchData::CoverGesture {
    //             Color::CSS_ORANGE
    //         } else {
    //             Color::CSS_CYAN
    //         })
    //         .stroke_width(8)
    //         .stroke_alignment(StrokeAlignment::Inside)
    //         .build();
    //     let fill = PrimitiveStyle::with_fill(Color::CSS_GRAY);
    //     let character_style = TextStyle::new(
    //         &embedded_bitmap_fonts::terminus::FONT_16x32,
    //         Color::CSS_ORANGE,
    //     );
    //
    //     let yoffset = 200;
    //
    //     let bbox = display.bounding_box();
    //
    //     let size = 120;
    //
    //     display.wait_for_vsync().await;
    //     let vsync_wait = start.elapsed();
    //
    //     match &prev_touch_data {
    //         TouchData::Points(points) => {
    //             for point in points {
    //                 fb.fill_rect(
    //                     point.x.saturating_sub(size / 2) as usize,
    //                     point.y.saturating_sub(size / 2) as usize,
    //                     size as usize,
    //                     size as usize,
    //                     &Color::BLACK.to_be_bytes(),
    //                 );
    //             }
    //         }
    //         TouchData::CoverGesture => {}
    //     }
    //     match &touch_data {
    //         TouchData::Points(points) => {
    //             for point in points {
    //                 // Draw a circle with a 3px wide stroke.
    //                 Circle::new(
    //                     Point::new(
    //                         point.x as i32 - size as i32 / 2,
    //                         point.y as i32 - size as i32 / 2,
    //                     ),
    //                     size as u32,
    //                 )
    //                 .into_styled(
    //                     PrimitiveStyleBuilder::new()
    //                         .fill_color(Color::CSS_CYAN)
    //                         .build(),
    //                 )
    //                 .draw(&mut fb)
    //                 .unwrap();
    //             }
    //         }
    //         TouchData::CoverGesture => {}
    //     }
    //
    //     let frametime = start.elapsed() - vsync_wait;
    //
    //     match &prev_touch_data {
    //         TouchData::Points(points) => {
    //             for point in points {
    //                 fb.flush_region(
    //                     &mut display,
    //                     point.x.saturating_sub(size / 2),
    //                     point.y.saturating_sub(size / 2),
    //                     size,
    //                     size,
    //                 )
    //                 .await;
    //             }
    //         }
    //         TouchData::CoverGesture => {}
    //     }
    //     match &touch_data {
    //         TouchData::Points(points) => {
    //             for point in points {
    //                 fb.flush_region(
    //                     &mut display,
    //                     point.x.saturating_sub(size / 2),
    //                     point.y.saturating_sub(size / 2),
    //                     size,
    //                     size,
    //                 )
    //                 .await;
    //             }
    //         }
    //         TouchData::CoverGesture => {}
    //     }
    //
    //     let flush = start.elapsed() - vsync_wait - frametime;
    //
    //     esp_println::println!(
    //         "draw: {:.1}ms\nvsync: {:.1}ms\nspi: {:.1}ms",
    //         frametime.as_micros() as f32 / 1_000.,
    //         vsync_wait.as_micros() as f32 / 1_000.,
    //         flush.as_micros() as f32 / 1_000.,
    //     );
    //
    //     prev_touch_data = touch_data;
    //     if !matches!(prev_touch_data, TouchData::Points(ref points) if !points.is_empty()) {
    //         touch.wait_for_touch().await;
    //     }
    //     touch_data = touch.read_touch_data().await.unwrap();
    // }

    let mut ticker = Ticker::every(Duration::from_millis(1000 / 30));
    let mut frametime = Duration::MIN;
    let mut flush = Duration::MIN;
    let mut clear = Duration::MIN;
    let mut vsync_wait = Duration::MIN;
    loop {
        let touch_data = touch.read_touch_data().await.unwrap();
        let start = Instant::now();

        use embedded_graphics::{
            prelude::*,
            primitives::{
                Circle, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, StrokeAlignment, Triangle,
            },
            text::{Alignment, Text},
        };
        // Create styles used by the drawing operations.
        let thin_stroke = PrimitiveStyle::with_stroke(Color::BLUE, 4 / UPSCALE as u32);
        let thick_stroke = PrimitiveStyle::with_stroke(color::RED, 8 / UPSCALE as u32);
        let border_stroke = PrimitiveStyleBuilder::new()
            .stroke_color(if touch_data != TouchData::CoverGesture {
                color::LIME
            } else {
                color::RED
            })
            .stroke_width(8 / UPSCALE as u32)
            .stroke_alignment(StrokeAlignment::Inside)
            .build();
        let fill = PrimitiveStyle::with_fill(Color::CSS_GRAY);
        let character_style =
            TextStyle::new(&embedded_bitmap_fonts::terminus::FONT_8x16, color::LIME);

        let yoffset = 200 / UPSCALE as i32;

        let bbox = display.bounding_box();

        // Draw a 3px wide outline around the display.
        Circle::new(Point::new(0, 0), bbox.size.width)
            .scale()
            .into_styled(border_stroke)
            .draw(&mut *fb)
            .unwrap();

        // Draw a triangle.
        Triangle::new(
            Point::new(56, 64 + yoffset).scale(),
            Point::new(56 + 64, 64 + yoffset).scale(),
            Point::new(56 + 32, yoffset).scale(),
        )
        .into_styled(thin_stroke)
        .draw(&mut *fb)
        .unwrap();

        // Draw a filled square
        Rectangle::new(Point::new(200, yoffset), Size::new(64, 64))
            .scale()
            .into_styled(fill)
            .draw(&mut *fb)
            .unwrap();

        // Draw a circle with a 3px wide stroke.
        Circle::new(Point::new(340, yoffset), 68)
            .scale()
            .into_styled(thick_stroke)
            .draw(&mut *fb)
            .unwrap();

        match touch_data {
            TouchData::Points(points) => {
                for point in points {
                    // Draw a circle with a 3px wide stroke.
                    Circle::new(Point::new(point.x as i32, point.y as i32), 40)
                        .scale()
                        .into_styled(
                            PrimitiveStyleBuilder::new()
                                .fill_color(Color::CSS_CYAN)
                                .build(),
                        )
                        .draw(&mut *fb)
                        .unwrap();
                }
            }
            TouchData::CoverGesture => {}
        }

        // Draw centered text.
        let text = alloc::format!(
            "fb clear: {:.1}ms\ndraw: {:.1}ms\nvsync: {:.1}ms\nspi: {:.1}ms",
            clear.as_micros() as f32 / 1_000.,
            frametime.as_micros() as f32 / 1_000.,
            vsync_wait.as_micros() as f32 / 1_000.,
            flush.as_micros() as f32 / 1_000.,
        );
        Text::with_alignment(
            &text,
            (display.bounding_box().center() + Point::new(100, 60)).scale(),
            character_style,
            Alignment::Right,
        )
        .draw(&mut *fb)
        .unwrap();

        frametime = start.elapsed();

        display.wait_for_vsync().await;

        vsync_wait = start.elapsed() - frametime;

        fb.flush(&mut display).await;

        flush = start.elapsed() - frametime - vsync_wait;

        fb.clear_color(Color::BLACK);
        clear = start.elapsed() - flush - frametime - vsync_wait;
        ticker.next().await;
    }
}
