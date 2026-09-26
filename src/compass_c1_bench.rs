//! Times C1 stage draws while the display core flushes alternating framebuffers.
//! Build with `cargo build --release --offline --features compass-c1-bench`.

use embassy_time::{Duration, Instant, Timer};
use esp_alloc::EspHeap;
use octowhere::{
    chrome::{self, Dirty},
    ui::{
        compass::CompassView,
        screens::Screen,
        script::Driver,
        stage::{Motion, Stage},
    },
    util::SwapThread,
};
use embedded_graphics::prelude::Point;

use super::SwapState;

const SAMPLES: usize = 120;

async fn measure(
    name: &str,
    stage: &Stage,
    clip: Option<&Dirty>,
    swap: &mut SwapThread<'static, SwapState<&'static EspHeap>>,
) {
    let mut samples = [0u32; SAMPLES];
    for (index, sample) in samples.iter_mut().enumerate() {
        let frame = swap.get();
        let fb = &mut *frame.fb;
        let start = Instant::now();
        if let Some(clip) = clip {
            stage.draw(&mut chrome::Clip::new(fb, clip));
            frame.dirty.clone_from(clip);
        } else {
            stage.draw(fb);
            frame.dirty.make_full();
        }
        *sample = start.elapsed().as_micros() as u32;
        frame.drawn = true;
        frame.brightness = (index == 0).then_some(180);
        frame.display_on = (index == 0).then_some(true);
        swap.swap().await;
    }
    samples.sort_unstable();
    defmt::info!(
        "[COMPASS C1 BENCH] name={=str} n={} pixels={} phase={} min_us={} median_us={} p95_us={} max_us={}",
        name,
        SAMPLES,
        clip.map_or(466 * 466, Dirty::pixels),
        stage.accents().texture,
        samples[0],
        samples[SAMPLES / 2],
        samples[SAMPLES * 95 / 100],
        samples[SAMPLES - 1],
    );
}

fn calibrated() -> CompassView {
    CompassView {
        live: true,
        calibration_percent: 100,
        heading_decidegrees: Some(470),
        pitch_deg: 5,
        roll_deg: -12,
        disturbed: false,
    }
}

fn on(view: CompassView, elapsed: u64) -> Driver<'static> {
    let mut driver = Driver::on(Screen::Compass);
    driver.motion(Motion { compass: view });
    driver.wait(elapsed);
    driver
}

async fn prime(stage: &Stage, swap: &mut SwapThread<'static, SwapState<&'static EspHeap>>) {
    for _ in 0..2 {
        let frame = swap.get();
        stage.draw(&mut *frame.fb);
        frame.dirty.make_full();
        frame.drawn = true;
        swap.swap().await;
    }
}

pub async fn run(swap: &mut SwapThread<'static, SwapState<&'static EspHeap>>) -> ! {
    Timer::after(Duration::from_secs(3)).await;

    let heading = calibrated();
    let cases = [
        ("heading", heading),
        ("interference", CompassView { disturbed: true, ..heading }),
        ("calibrating", CompassView { heading_decidegrees: None, calibration_percent: 54, ..heading }),
        ("top_edge", CompassView { heading_decidegrees: None, ..heading }),
        ("no_data", CompassView::default()),
    ];
    for (name, view) in cases {
        let driver = on(view, 500_000);
        measure(name, &driver.stage, None, swap).await;
    }

    for (name, elapsed, phase) in [
        ("entry_blocks", 40_000, 1),
        ("entry_dark", 100_000, 2),
        ("entry_tiles", 150_000, 3),
        ("entry_pixels", 275_000, 4),
        ("entry_settled", 400_000, 5),
    ] {
        let driver = on(heading, elapsed);
        assert_eq!(driver.stage.accents().texture, phase);
        measure(name, &driver.stage, None, swap).await;
    }

    let mut driver = on(heading, 500_000);
    for x in [400, 340, 280, 245] {
        driver.touch(Some(Point::new(x, 233)));
    }
    measure("swiping", &driver.stage, None, swap).await;

    let mut driver = on(heading, 500_000);
    prime(&driver.stage, swap).await;
    driver.motion(Motion { compass: CompassView { heading_decidegrees: Some(480), ..heading } });
    let mut dirty = Dirty::new();
    dirty.clone_from(driver.stage.changed());
    assert!(!dirty.is_empty() && !dirty.is_full());
    measure("heading_one_degree", &driver.stage, Some(&dirty), swap).await;

    defmt::info!("[COMPASS C1 BENCH] done");
    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
}
