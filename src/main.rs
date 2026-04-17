#![no_std]
#![no_main]
// Now defaults to deny in Rust-2024, however abusing statics is necessary in the embedded world.
#![expect(static_mut_refs)]

use embassy_executor::Spawner;

use esp_hal::{
    interrupt::software::SoftwareInterruptControl,
    timer::timg::TimerGroup,
};
use static_cell::StaticCell;

use esp_alloc as _;
use esp_backtrace as _;
esp_bootloader_esp_idf::esp_app_desc!();

static mut CORE1_STACK: esp_hal::system::Stack<8192> = esp_hal::system::Stack::new();
static CORE1_EXECUTOR: StaticCell<esp_rtos::embassy::Executor> = StaticCell::new();

#[embassy_executor::task]
async fn second_core(_spawner: Spawner) {}

#[esp_rtos::main]
async fn main(_spawner: Spawner) {
    // Heap: 64KB SRAM + PSRAM for large allocs
    esp_alloc::heap_allocator!(size: 64 * 1024);

    // PERF: How low can we drop the clock before running into timing issues?
    let peripherals =
        esp_hal::init(esp_hal::Config::default().with_cpu_clock(esp_hal::clock::CpuClock::_240MHz));
    let sw_int = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);

    // PSRAM
    esp_alloc::psram_allocator!(peripherals.PSRAM, esp_hal::psram);

    // Embassy timer
    let timg0 = TimerGroup::new(peripherals.TIMG0);

    esp_rtos::start(timg0.timer0, sw_int.software_interrupt0);

    // Start up the executor on the second core.
    // PERF: Do we need the second core?
    esp_rtos::start_second_core(
        peripherals.CPU_CTRL,
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
}
