# Our peripheral drivers against crates.io (2026-10-06)

The firmware's own drivers compared with the crates.io drivers for the same chips, at master
`8e44764`. Out of scope: SX127x and TCA9554 (forks we keep), and QMI8658 (`ph-qmi8658`, from
crates.io already). The candidates were downloaded and read, never built or run. Line citations
point into each crate's published source.

The main session checked these claims against the source:
- cst92xx `protocol.rs:86-94` and the mirror in `types.rs`;
- pcf85063a's seconds mask, `decode_bcd` and the COF XOR;
- axp2101-embedded's 0x47 ID check;
- bmm350's 16-bit write at 0x04;
- display-driver-co5300's TE commands.

"(unverified)" marks what nobody checked against code, datasheet or board record.

**Verdict: keep ours for all six.** No crate exists for the LC76G, or for Quectel's I2C GNSS
transport.

| Chip | Best candidate | Verdict | Deciding reason |
|---|---|---|---|
| CST9217 | cst92xx 0.2.4 | Keep ours | `touches()` returns `[Option<Point>; 2]`, so stale reads, lifts with their position, cover and gestures all read as "no points". The firmware's touch handling rests on those four. Recovering them is an API break upstream. |
| AXP2101 | axp2101-dd 0.3.1 | Keep ours | Reaches all 22 registers the firmware uses, but adds device-driver's DSL and bisync2 for no feature we use, and reaches the power key only through its low-level API. The other two fail on this board. |
| PCF85063A | pcf85063a 0.1.1 | Keep ours | `get_datetime` discards the oscillator-stop flag and accepts invalid BCD. The mesh's clock gate and the "no time" state rest on both. |
| BMM350 | bmm350 0.0.2 | Keep ours | Blocking only, never applies its compensation data, and its configuration call disables all three axes. |
| CO5300 | display-driver-co5300 0.1.1 | Keep ours | One pixel slice per window, a new RAM write per call, and no TE line or TE wait. Ours streams shifted rows under one RAM write and starts on TE at scan line 150. |
| LC76G | none | Keep ours | No crate exists. |

## CST9217 touch

The firmware uses:
- `new(i2c, rst, int, delay)` and `init()`, with errors split into no reply and bad reply;
- `set_config` (mirror only), `resolution()` and `firmware()`;
- `enter_gesture_mode()` and `leave_gesture_mode()`;
- `wait_for_touch()`, on INT's falling edge;
- `read_touch_data()`, returning `Points`, `Lifted(point)`, `Gesture(Tap | Swipe ×4)`, `CoverGesture` or `Stale`.

What the board taught (`cst9217-configuration.md`):
- Gesture mode is 0xD104, entered from normal mode. Each gesture's code is in byte 4 of the lift report, and only a reset leaves the mode.
- Normal and low-power scan modes sometimes cut a 15-byte read to 7 bytes, so the driver stays in debug mode, which never does.
- Byte 0 = 0xAB is a stale read. Byte 0 = 0x00 is a lift that carries the last position.
- INT is a pulse of about 2.3 ms, so the driver waits for its edge, not its level.

**cst92xx 0.2.4**:
- Released 2026-08-20; 197 downloads; MIT; github.com/ScripTerasu/cst92xx-touch-driver. The maintainer is active.
- Async or blocking; `no_std`. Ported from SensorLib. Its README says it was tested on the Waveshare 1.75C (unverified).
- Problems:
  - Stale reads, lifts and covers all return `[None, None]`, and the gesture byte is never read (`protocol.rs:86-94`).
  - It has no gesture mode, and its raw writes are private.
  - Its mirror is `max - x` clamped to `max`, one past the last pixel (`types.rs:75-81`). Its test pins that.
  - It offers the Normal and LowPower modes without warning that they cut reads short.
  - It drops a valid second finger when slot 0 is invalid (`protocol.rs:127`).
- Tests: embedded-hal-mock tests plus transform unit tests.

## AXP2101 PMIC

The firmware uses:
- `init`, which:
  - checks the chip ID is 0x4A (BadReply otherwise);
  - enables only the key's short- and long-press interrupts in 0x40-0x42, and clears 0x48-0x4A;
  - enables the VBAT, VBUS, VSYS and die-temperature ADCs, with TS off.
- `configure_power_key`: REG27 IRQLEVEL 1 s and ONLEVEL 512 ms, by read-modify-write; REG22.
- `power_sources` (REG20 and REG21).
- Every 250 ms:
  - `take_key_press`, which writes back only the key bits;
  - battery presence;
  - VBAT, VBUS and VSYS, each one 14-bit burst;
  - percent, charging, and VBUS in.
- `power_off`: interrupts disabled and cleared, then REG10 bit 0.

What the board taught:
- The chip answers 0x4A. This is inferred from a check that has passed since `630e870`.
- Interrupts are disabled and cleared before soft power-off, because IRQ low can power the board back on.
- REG10 bit 3 is never written, since PWROK drives CHIP_PU.
- TS stays off, because there is no thermistor.

**axp2101-dd 0.3.1**:
- Released 2026-09-23; MIT/Apache; github.com/okhsunrog/axp2101-dd.
- Async and blocking through bisync2; device-driver 2.1.1 compiles a DSL, with MSRV 1.94.
- Its `defmt` feature enables embedded-hal's `defmt-03` alongside our defmt 1.1 (unverified).
- Every register is reachable. REG27 and REG22, and clearing only the key bits, need its low-level API.
- It reads each ADC value in two transactions. Its `is_charging` is the battery current's direction, where ours reads the charge-status bits; agreement between the two is unverified.
- No tests.

**axp2101-embedded 0.3.0**:
- `init()` requires chip ID 0x47 (`registers.rs:13`), so on this board it returns `DeviceNotFound`.
- It has no REG27 setter.
- Its interrupt enable disagrees with its status for groups 0 and 2: `enable_irq(IRQ_WARNING_LEVEL2)` sets REG42 bit 7, the watchdog enable (`driver_async.rs:1607-1627`).
- No tests.

**axp2101 0.1.0**:
- It needs std.
- Its interrupt-enable bits are reversed: short press at bit 4, where the datasheet puts it at bit 3.
- It has no power-off and no interrupt status.

Ruled out: m5stack-core pins esp-hal `=1.1.2`, and core-s3 sets CoreS3 rails and is blocking.

## PCF85063A RTC

The firmware uses:
- `init` (clears STOP and 12/24 mode);
- `get_time`, a 7-byte burst that returns `InvalidDateTime` for non-BCD or impossible values;
- `oscillator_stopped()`, which gates `mesh::RTC_TIME`, fix injection and the GNSS reference time;
- `set_time`, which clears the stop flag.

**pcf85063a 0.1.1**:
- Released 2024-06-27; MIT/Apache; tweedegolf. Async, `no_std`, built on `time` 0.3.
- Problems:
  - The seconds read is masked with 0x7F, so the oscillator-stop flag is discarded (`datetime.rs:34`).
  - Invalid BCD isn't rejected.
  - `decode_bcd` keeps only 3 bits of the tens digit, so years 2080-2099 decode wrong (`lib.rs:263`).
  - `write_clock_output_frequency` XORs the COF field instead of clearing it (`lib.rs:253`).
  - `set_datetime` doesn't validate the year: `(year - 2000) as u8` wraps.
- Tests: BCD only.

## BMM350 magnetometer

The firmware uses:
- `init`: soft reset, chip ID 0x33, the 32 OTP words, OTP off, then bit reset and flux-guide reset.
- `start_normal_mode` (100 Hz, 2× averaging, three axes).
- `data_ready`, polling INT_STATUS 0x30, since this board has no INT pin.
- `read_data`, a 17-byte burst with 2 dummy bytes.
- `compensate`, the full OTP model, giving µT.

**bmm350 0.0.2** (released 2025-04-07; Apache):
- Blocking only.
- `set_mag_config` writes a little-endian u16 to 0x04, so its high byte lands in 0x05, the axis-enable register. Normal mode writes 0x10 there, which clears all three axes (`lib.rs:111-118`, `device.rs:186-188`).
- `get_interrupt_status` reads 0x03, not 0x30.
- Compensation offsets are parsed and never used, and `to_ut` returns twice the raw value.
- Every single-register read moves 128 bytes.
- No tests.

## CO5300 AMOLED

The firmware uses:
- `new(bus, reset, te, delay, board::DISPLAY)`: 466×466, column offset 6, TE line 150, brightness 0 before display-on.
- `display_on` and `display_off`, `wait_for_te` on the rising edge, and `set_brightness`.
- `set_window`, which widens to the 2×2 grain.
- `pixels()`, `fill()` and `finish()`: rows streamed into two DMA buffers under one RAM write (0x12), with the pixel shift applied per row.

**display-driver-co5300 0.1.1** (decaday/display-driver):
- Async; no tests.
- Its start-up sends a vendor page and password sequence that ours doesn't.
- It turns TE on (0x35) but never sets a TE line (0x44), so TE pulses at blanking: the case that tore.
- It has no TE pin and no wait.
- `write_pixels` takes one slice per call, and each call reopens the window and the RAM write (0x32). That's unverified on this board, and the shifted flush would need a 434,312-byte contiguous copy.
- Display on and off are on a private field.
- Odd windows return `UnalignedArea`.

**sh8601-rs 0.1.8** is for another controller. It is blocking, owns a `Box` framebuffer, and allocates a `Vec` on every partial flush.

## LC76G GNSS

There is no crate for the LC76G, for any Airoha/PAIR receiver, or for Quectel's I2C GNSS transport (0x50 configuration, 0x54 read, 0x58 write, with whole-step retries). The nearest are:
- embassy_gps 0.1.0: L76K over UART, PCAS commands, AGPL.
- pmtk: MediaTek's command set.
- u-blox crates.

## Noticed in ours

- `power.rs`: `is_charging` counted charge status 1..=3, though the datasheet lists 000 as trickle charge. Fixed the same day: 000 counts as charging.
- `lc76g`: `write_config` retries 20 times inside each of `read_after_config`'s and `write_after_config`'s 20 attempts. That can add up to about 4 s of delays per call against a module that never answers (arithmetic, not measured).

## Possible upstream reports

None of these changes a verdict.
- cst92xx: the mirror off-by-one, and the gesture-mode facts.
- axp2101-embedded: the 0x47 ID, and the interrupt group swap.
- pcf85063a: the year decode, and the COF XOR.
