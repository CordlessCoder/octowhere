//! Replays the magnetometer samples of a serial log through the compass calibration. From the
//! repository root:
//!
//! ```text
//! xz -dc docs/logs/compass/<file> | cargo +stable run --manifest-path host-tests/Cargo.toml \
//!   --target x86_64-unknown-linux-gnu --example replay_calibration
//! ```
//!
//! It prints each calibration event; `VERBOSE=1` prints every sample. A `[COMPASS]
//! recalibrating` line restarts it, as the tap did.
//!
//! The samples are logged at debug level, so record from firmware built with `DEFMT_LOG=debug`,
//! decoded by `espflash monitor --log-format defmt`.

use std::io::BufRead;

use octowhere_host_tests::compass::{AxisMap, Calibration, CalibrationEvent, HardIron};

/// `MAG_AXES` in `src/main.rs`.
const MAG_AXES: AxisMap = AxisMap([(0, -1.0), (1, -1.0), (2, 1.0)]);

fn comp(line: &str) -> Option<[f32; 3]> {
    let start = line.find("[BMM350] sample raw=(")?;
    let line = &line[start..];
    let values = line.split("comp=(").nth(1)?.split(")uT").next()?;
    let values: Vec<f32> = values
        .split(", ")
        .map(|value| value.parse().ok())
        .collect::<Option<_>>()?;
    values.try_into().ok()
}

fn describe(fit: Option<&HardIron>) -> String {
    fit.map_or_else(String::new, |fit| {
        let offset = fit.offset();
        format!(
            " candidate: progress={:.2} offset=({:.1}, {:.1}, {:.1}) radius={:.1} residual={:.2?}",
            fit.progress(),
            offset[0],
            offset[1],
            offset[2],
            fit.radius(),
            fit.residual()
        )
    })
}

fn main() {
    let mut calibration = Calibration::new();
    // Every sample since the last restart, never forgetting, for the residual a calibration has.
    let mut whole = HardIron::new();
    let mut disturbed = 0;
    let mut samples = 0;
    for (number, line) in std::io::stdin().lock().lines().enumerate() {
        let line = line.unwrap();
        if line.contains("[COMPASS] recalibrating") {
            println!("{number:5} recalibrate after {samples} samples, {disturbed} disturbed");
            calibration = Calibration::new();
            whole = HardIron::new();
            (samples, disturbed) = (0, 0);
            continue;
        }
        let Some(raw) = comp(&line) else { continue };
        let field = MAG_AXES.apply(raw);
        let event = calibration.update(field);
        whole.update(field);
        let offset = calibration.offset();
        let corrected: [f32; 3] = core::array::from_fn(|axis| field[axis] - offset[axis]);
        let strength = corrected.iter().map(|v| v * v).sum::<f32>().sqrt();
        samples += 1;
        let is_disturbed = calibration.progress() >= 1.0 && calibration.disturbed(corrected);
        disturbed += usize::from(is_disturbed);
        if event != CalibrationEvent::None || std::env::var_os("VERBOSE").is_some() {
            println!(
                "{number:5} {event:?} offset=({:6.1}, {:6.1}, {:6.1}) radius={:5.1} |b|={strength:6.1}{} all-samples residual={:.2?}{}",
                offset[0],
                offset[1],
                offset[2],
                calibration.radius(),
                if is_disturbed { " DISTURBED" } else { "" },
                whole.residual(),
                describe(calibration.candidate()),
            );
        }
    }
    let offset = calibration.offset();
    println!(
        "end: {samples} samples, {disturbed} disturbed, offset=({:.1}, {:.1}, {:.1}) radius={:.1}{}",
        offset[0],
        offset[1],
        offset[2],
        calibration.radius(),
        describe(calibration.candidate())
    );
}
