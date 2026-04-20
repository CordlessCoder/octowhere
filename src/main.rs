#![feature(impl_trait_in_assoc_type)]
#![no_std]
#![no_main]
// Now defaults to deny in Rust-2024, however abusing statics is necessary in the embedded world.
#![expect(static_mut_refs)]

use axp2101_embedded::AsyncAxp2101;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embedded_graphics::{pixelcolor::Rgb888, prelude::*};
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
    peripherals::rtc::Pcf85063aRtc,
};
use static_cell::StaticCell;

use esp_alloc as _;
use esp_backtrace as _;
use tca9554::Tca9554;
esp_bootloader_esp_idf::esp_app_desc!();

static mut CORE1_STACK: esp_hal::system::Stack<8192> = esp_hal::system::Stack::new();
static CORE1_EXECUTOR: StaticCell<esp_rtos::embassy::Executor> = StaticCell::new();

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
    .with_sda(peripherals.GPIO15)
    .with_scl(peripherals.GPIO14)
    .into_async();

    let i2c = Mutex::<NoopRawMutex, _>::new(i2c);
    let i2c = I2cDevice::new(&i2c);

    // FIXME: May not be actually routed anywhere on the waveshare board we're using, in which case
    // this has to go :(
    let exio_int = Input::new(peripherals.GPIO0, InputConfig::default());
    let mut exio = Tca9554::new(i2c.clone(), tca9554::Address::standard())
        .with_int::<_, 8, embassy_sync::blocking_mutex::raw::NoopRawMutex>(exio_int);

    let i2c_peripherals = async {
        exio.init().await.unwrap();

        let mut axp2101 = AsyncAxp2101::new(i2c.clone());
        axp2101.init().await.unwrap();
        axp2101.disable_general_adc_channel().await.unwrap();
        println!("[Power] AXP2101 PMIC initalized");

        let mut rtc = Pcf85063aRtc::new(i2c.clone());
        rtc.init().await.unwrap();
        println!("[RTC] OK");

        let imu_int2 = Input::new(peripherals.GPIO21, InputConfig::default());
        let mut imu = ph_qmi8658::Qmi8658::new_i2c(i2c.clone(), Some(exio.pin(6)), Some(imu_int2));
        imu.init(&mut embassy_time::Delay).await.unwrap();
        println!("[IMU] OK");

        (axp2101, rtc, imu)
    };

    let display = async {
        let spi_config = spi::master::Config::default()
            .with_frequency(Rate::from_mhz(80))
            .with_mode(spi::Mode::_0);

        let dma_tx = dma_tx_buffer!(8000).unwrap();

        let spi = spi::master::Spi::new(peripherals.SPI2, spi_config)
            .expect("SPI failed")
            .with_sck(peripherals.GPIO38)
            .with_sio0(peripherals.GPIO4)
            .with_sio1(peripherals.GPIO5)
            .with_sio2(peripherals.GPIO6)
            .with_sio3(peripherals.GPIO7)
            .with_dma(peripherals.DMA_CH0)
            .into_async();
        let cs = Output::new(peripherals.GPIO12, Level::High, OutputConfig::default());
        let reset = Output::new(peripherals.GPIO39, Level::High, OutputConfig::default());

        let spi = QspiBus::new(spi, dma_tx, cs);
        let mut display = Co5300Display::new(spi, reset, peripherals.GPIO13).await;

        println!("[DISPLAY] OK");

        // Statically alloacted framebuffer, around 600Kb

        let mut fb = Framebuffer::alloc();
        fb.clear_color(Rgb888::BLACK);
        fb.flush(&mut display);
        println!("[FB] OK");
        (display, fb)
    };

    let ((power, rtc, imu), (display, fb)) =
        embassy_futures::join::join(i2c_peripherals, display).await;
}
