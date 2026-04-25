#![feature(impl_trait_in_assoc_type)]
#![no_std]
#![no_main]
// Now defaults to deny in Rust-2024, however abusing statics is necessary in the embedded world.
#![expect(static_mut_refs)]
extern crate alloc;

use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embassy_time::{Duration, Instant, Ticker};
use embedded_bitmap_fonts::TextStyle;
use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
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
    drivers::{co5300::Co5300Display, framebuffer::Framebuffer, qspi_bus::QspiBus},
    peripherals::{
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

// Rgb888 is higher quality, Rgb565 cuts the size of the framebuffer by a third.
// Gray8 is 3x smaller than Rgb888... but I'm not sure we love monochrome.
type Color = Rgb565;

#[embassy_executor::task]
async fn second_core(_spawner: Spawner) {}

// TODO: CST9217 touch

#[esp_rtos::main]
async fn main(_spawner: Spawner) {
    // PERF: Figure out if we can reclaim any SRAM from the bootloader
    // Heap: 64KB SRAM + PSRAM for large allocs
    // esp_alloc::heap_allocator!(#[ram(reclaimed)] size: 64 * 1024);

    // PERF: How low can we drop the clock before running into timing issues?
    let mut peripherals =
        esp_hal::init(esp_hal::Config::default().with_cpu_clock(esp_hal::clock::CpuClock::_240MHz));
    let sw_int = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);

    esp_alloc::psram_allocator!(peripherals.PSRAM, esp_hal::psram);

    let timg0 = TimerGroup::new(peripherals.TIMG0);

    esp_rtos::start(timg0.timer0, sw_int.software_interrupt0);

    // Start up the executor on the second core.
    // PERF: We want to utilize cores fairly evenly because running both uses little more power
    // than running just one.
    // Idea: When using double buffering, have core0 handle the drawing, and core1 handle flipping
    esp_rtos::start_second_core(
        peripherals.CPU_CTRL.reborrow(),
        sw_int.software_interrupt1,
        // SAFETY: This static mut value must not be accessed ever again, anywhere
        unsafe { &mut CORE1_STACK },
        || {
            let executor = CORE1_EXECUTOR.init_with(esp_rtos::embassy::Executor::new);
            executor.run(|spawner| {
                spawner.spawn(second_core(spawner).unwrap());
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

    // FIXME: May not be actually routed anywhere on the waveshare board we're using, in which case
    // this has to go :(
    // let exio_int = Input::new(peripherals.GPIO0, InputConfig::default());
    let mut exio = Tca9554::new(i2c.clone(), tca9554::Address::standard());

    let i2c_peripherals = async {
        exio.init().await.unwrap();

        // let mut axp2101 = AsyncAxp2101::new(i2c.clone());
        // axp2101.init().await.unwrap();
        // axp2101.disable_general_adc_channel().await.unwrap();
        // println!("[Power] AXP2101 PMIC initalized");

        let mut rtc = Pcf85063aRtc::new(i2c.clone());
        rtc.init().await.unwrap();
        println!("[RTC] OK");

        let lpf = ph_qmi8658::LowPassFilterMode::OdrPercent14_0;
        let config = ph_qmi8658::Config {
            accel: Some(ph_qmi8658::AccelConfig {
                range: ph_qmi8658::AccelRange::G8,
                odr: ph_qmi8658::AccelOutputDataRate::Hz125,
                lpf: Some(lpf),
            }),
            gyro: Some(ph_qmi8658::GyroConfig {
                range: ph_qmi8658::GyroRange::Dps512,
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
        imu.apply_interrupt_config(ph_qmi8658::InterruptConfig {
            ctrl9_handshake_statusint: false,
            motion_pin: ph_qmi8658::InterruptPin::Int2,
            pedometer: false,
            significant_motion: false,
            no_motion: false,
            any_motion: false,
            tap: false,
        })
        .await
        .unwrap();
        imu.set_mode_with_delay(
            &mut embassy_time::Delay,
            ph_qmi8658::OperatingMode::AccelGyroOnly,
        )
        .await
        .unwrap();
        println!("[IMU] OK");

        let touch_rst = Output::new(peripherals.GPIO40, Level::High, OutputConfig::default());
        let touch_int = Input::new(peripherals.GPIO11, InputConfig::default());
        let mut touch = Cst9217::new(i2c.clone(), touch_rst, Some(touch_int), embassy_time::Delay);
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

        (rtc, imu, touch)
    };

    let display = async {
        let spi_config = spi::master::Config::default()
            .with_frequency(Rate::from_mhz(80))
            .with_mode(spi::Mode::_0);

        let dma_tx = dma_tx_buffer!(8000).unwrap();

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
        let mut display = Co5300Display::new(spi, reset, te).await;

        println!("[DISPLAY] OK");

        // delay_ms_async(100).await;

        let mut fb = Framebuffer::<
            { octowhere::drivers::framebuffer::buffer_size::<Color>() },
            Color,
        >::alloc();
        fb.clear_color(Color::CSS_DIM_GRAY);
        fb.flush_vsync(&mut display).await;
        println!("[FB] OK");
        (display, fb)
    };

    let ((_rtc, _imu, mut touch), (mut display, mut fb)) =
        embassy_futures::join::join(i2c_peripherals, display).await;
    // let (mut display, mut fb) = display.await;
    display.set_brightness(120);

    let mut ticker = Ticker::every(Duration::from_millis(1000 / 30));
    let mut frametime = Duration::MIN;
    let mut flush = Duration::MIN;
    let mut clear = Duration::MIN;
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
        let thin_stroke = PrimitiveStyle::with_stroke(Color::BLUE, 4);
        let thick_stroke = PrimitiveStyle::with_stroke(Color::RED, 8);
        let border_stroke = PrimitiveStyleBuilder::new()
            .stroke_color(if touch_data != TouchData::CoverGesture {
                Color::CSS_ORANGE
            } else {
                Color::CSS_CYAN
            })
            .stroke_width(8)
            .stroke_alignment(StrokeAlignment::Inside)
            .build();
        let fill = PrimitiveStyle::with_fill(Color::CSS_GRAY);
        let character_style = TextStyle::new(
            &embedded_bitmap_fonts::terminus::FONT_16x32,
            Color::CSS_ORANGE,
        );

        let yoffset = 200;

        let bbox = display.bounding_box();

        // Draw a 3px wide outline around the display.
        Circle::new(Point::new(0, 0), bbox.size.width)
            .into_styled(border_stroke)
            .draw(&mut fb)
            .unwrap();

        // Draw a triangle.
        Triangle::new(
            Point::new(56, 64 + yoffset),
            Point::new(56 + 64, 64 + yoffset),
            Point::new(56 + 32, yoffset),
        )
        .into_styled(thin_stroke)
        .draw(&mut fb)
        .unwrap();

        // Draw a filled square
        Rectangle::new(Point::new(200, yoffset), Size::new(64, 64))
            .into_styled(fill)
            .draw(&mut fb)
            .unwrap();

        // Draw a circle with a 3px wide stroke.
        Circle::new(Point::new(340, yoffset), 68)
            .into_styled(thick_stroke)
            .draw(&mut fb)
            .unwrap();

        match touch_data {
            TouchData::Points(points) => {
                for point in points {
                    // Draw a circle with a 3px wide stroke.
                    Circle::new(Point::new(point.x as i32, point.y as i32), 40)
                        .into_styled(
                            PrimitiveStyleBuilder::new()
                                .fill_color(Color::CSS_CYAN)
                                .build(),
                        )
                        .draw(&mut fb)
                        .unwrap();
                }
            }
            TouchData::CoverGesture => {}
        }

        // Draw centered text.
        let text = alloc::format!(
            "clear: {:.1}ms\nframetime: {:.1}ms\nflush: {:.1}ms",
            clear.as_micros() as f32 / 1_000.,
            frametime.as_micros() as f32 / 1_000.,
            flush.as_micros() as f32 / 1_000.
        );
        Text::with_alignment(
            &text,
            display.bounding_box().center() + Point::new(0, 60),
            character_style,
            Alignment::Center,
        )
        .draw(&mut fb)
        .unwrap();

        frametime = start.elapsed();

        fb.flush_vsync(&mut display).await;

        flush = start.elapsed() - frametime;

        fb.clear_color(Color::BLACK);
        clear = start.elapsed() - flush - frametime;
        ticker.next().await;
    }
}
