//! Steps the CO5300 through each control the crate added, holding each long enough to watch
//! the panel, and logs what can be measured: the scan line it reads back and the TE pulses a
//! second in each mode. Round after round; never returns.

use co5300::{Sunlight, TeMode};
use defmt::info;
use embassy_time::{Duration, Instant, Timer, with_timeout};
use embedded_graphics::prelude::*;
use octowhere::{
    chrome::{Color, FB},
    drivers::{Display, framebuffer::Flush as _},
};

const HOLD: Duration = Duration::from_secs(6);
const LEVEL: u8 = 0x80;

/// Eight colour bars across, darkening down the panel, so idle mode's eight colours and
/// the partial area's edges show.
fn pattern(fb: &mut FB) {
    const BARS: [(u8, u8, u8); 8] = [
        (31, 63, 31),
        (31, 63, 0),
        (0, 63, 31),
        (0, 63, 0),
        (31, 0, 31),
        (31, 0, 0),
        (0, 0, 31),
        (6, 12, 6),
    ];
    for y in 0..466u32 {
        for x in 0..466u32 {
            let (r, g, b) = BARS[(x * 8 / 466) as usize];
            let scale = 466 - y / 2;
            let color = Color::new(
                (u32::from(r) * scale / 466) as u8,
                (u32::from(g) * scale / 466) as u8,
                (u32::from(b) * scale / 466) as u8,
            );
            let _ = Pixel(Point::new(x as i32, y as i32), color).draw(fb);
        }
    }
}

/// TE rising edges counted over a second.
async fn te_edges(display: &mut Display<'_, Color>) -> u32 {
    let end = Instant::now() + Duration::from_secs(1);
    let mut edges = 0;
    loop {
        let now = Instant::now();
        if now >= end {
            return edges;
        }
        if with_timeout(end - now, display.wait_for_te()).await.is_ok() {
            edges += 1;
        }
    }
}

async fn step(name: &str) {
    info!("[CTRLBENCH] {=str}", name);
    Timer::after(HOLD).await;
}

pub async fn run(display: &mut Display<'_, Color>, fb: &mut FB) -> ! {
    pattern(fb);
    for round in 0.. {
        info!("[CTRLBENCH] round {}", round);
        fb.flush(display, false).await.unwrap();
        display.set_brightness(LEVEL).await.unwrap();
        step("pattern at the plain level").await;

        for sio1 in [false, true] {
            octowhere::drivers::qspi_bus::READ_ON_SIO1
                .store(sio1, core::sync::atomic::Ordering::Relaxed);
            for (command, len) in [(0x04u8, 3usize), (0x0A, 1), (0x0C, 1), (0x45, 2)] {
                let mut reply = [0u8; 3];
                display.read_raw(command, &mut reply[..len]).await.unwrap();
                info!(
                    "[CTRLBENCH] sio1={} read {=u8:#04x}: {=[u8]:#04x}",
                    sio1,
                    command,
                    reply[..len]
                );
            }
        }
        octowhere::drivers::qspi_bus::READ_ON_SIO1
            .store(false, core::sync::atomic::Ordering::Relaxed);
        let mut lines = [0u16; 16];
        for line in &mut lines {
            *line = display.scan_line().await.unwrap();
            Timer::after(Duration::from_micros(1_300)).await;
        }
        info!("[CTRLBENCH] scan lines {}", lines);

        info!(
            "[CTRLBENCH] te vblank at 150: {} a second",
            te_edges(display).await
        );
        display.te_off().await.unwrap();
        info!("[CTRLBENCH] te off: {} a second", te_edges(display).await);
        display.te_on(TeMode::VAndHBlank).await.unwrap();
        info!(
            "[CTRLBENCH] te v and h blank: {} a second",
            te_edges(display).await
        );
        display.te_off().await.unwrap();
        display.te_on(TeMode::VBlank).await.unwrap();
        display.set_te_line(150).await.unwrap();
        info!(
            "[CTRLBENCH] te back at 150: {} a second",
            te_edges(display).await
        );

        display
            .set_partial_area((0, 465), (150, 315))
            .await
            .unwrap();
        display.partial_mode().await.unwrap();
        step("partial: rows 150 to 315 only").await;
        display.normal_mode().await.unwrap();
        step("normal again").await;

        display.set_idle(true).await.unwrap();
        step("idle: eight colours").await;
        display.set_idle(false).await.unwrap();

        display.set_hbm_brightness(0xFF).await.unwrap();
        display.set_hbm(true).await.unwrap();
        step("high-brightness mode at 255").await;
        display.set_hbm(false).await.unwrap();
        step("plain level again").await;

        for (name, level) in [
            ("sunlight low", Sunlight::Low),
            ("sunlight medium", Sunlight::Medium),
            ("sunlight high", Sunlight::High),
        ] {
            display.set_sunlight(Some(level)).await.unwrap();
            step(name).await;
        }
        display.set_sunlight(None).await.unwrap();

        display.set_current_limit(true).await.unwrap();
        step("current limit on").await;
        display.set_current_limit(false).await.unwrap();

        display.deep_standby().await.unwrap();
        step("deep standby").await;
        display.leave_deep_standby().await.unwrap();
        fb.flush(display, false).await.unwrap();
        display.set_brightness(LEVEL).await.unwrap();
        info!(
            "[CTRLBENCH] after deep standby: te {} a second",
            te_edges(display).await
        );
        step("back from deep standby").await;

        octowhere::settings::hold_display_core_if_asked();
    }
    unreachable!()
}
