# Compass recordings, 2026-09-23

Serial logs from the compass bring-up, filtered to the sensor lines and xz-compressed. Read one
with `xz -dc <file>`.

- `[BMM350] sample … comp=(x, y, z)uT` is the compensated field in the magnetometer's own axes,
  and `[IMU] sample … si_accel=[…] si_gyro=[…]` the accelerometer in µm/s² and gyro in µrad/s in
  the IMU's own axes. Neither depends on the axis mapping loaded at the time. Both print every
  250 ms.
- `[COMPASS] screen …` lines are in screen axes, so they depend on the mapping the build had, and
  appear only while the compass screen showed. `[COMPASS] recalibrating` marks a tap on the dial.
- `[POSE]` lines are the axis check screen's captures.

Files:

- `spin-…` was recorded with the first mapping guesses (IMU `(x, -y, -z)`, magnetometer identity)
  while the board was turned level through a full circle. Its field jumps by 100 to 170 µT for
  about a second at a time, most likely a phone close by: the interference that spoiled a
  min/max calibration.
- `axischeck-…` holds the twelve `[POSE]` captures that `tools/fit-sensor-axes.py` fits the
  mappings from, and the raw samples around them.
- `verify-…` is with the fitted mappings and the min/max calibration, several recalibrations
  and a lot of tilting. Replaying its raw samples since the last recalibration showed the
  min/max z offset 9.7 µT out and a sphere fit 2.6 µT out, against the pose fit.
- `fusion-…` is with the gyro fusion and sphere-fit calibration, the build committed as
  `58dec8a`. Its `[COMPASS]` lines include the learned gyro offset.
