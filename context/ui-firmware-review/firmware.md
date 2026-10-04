# Firmware review at b8ab7ca

Partition: `firmware/src/` (all), `firmware/Cargo.toml`, `host-tests/`, `crates/octowhere-peripherals`,
`crates/octowhere-motion`. Paths are relative to the worktree at `b8ab7ca`, line numbers at that commit.

Not re-reported: steps 12 and 13 of `MESH-CLEANUP-PLAN.md` (the group header's and slots' flash
encoding in `settings.rs`), step 4's `MESH_VERSION` comment, and step 6's `1 << id` sites in
`settings.rs`.

Two findings rest on what the compiler did. They come from a release build of the firmware in the
worktree (`CARGO_TARGET_DIR=/home/me/.claude/jobs/173b5f0c/tmp/review-target-firmware cargo build
--release --offline` from `firmware/`) and the frame listing in `MESH-CLEANUP-PLAN.md`, "Stack". Two
bugs were reproduced with throwaway host programs outside both checkouts
(`/home/me/.claude/jobs/173b5f0c/tmp/review-scratch-firmware/`).

## Bugs

### The touch queue drops a lift or a cover when both slots are full
- Where: firmware/src/main.rs:335-355 (`put_touch_read`), :314 (`heapless::Deque<TouchRead, 2>`); AGENTS.md:441-442
- Category: Bugs
- Weight: medium
- What: The doc says "A lift or a cover stays, and the next report queues behind it", and AGENTS.md says a lift or cover "is kept ahead of the next report". But once both slots hold reads, `|| reads.is_full()` lets any new report overwrite the back slot, lift or cover included. A lift and a cover waiting, then a contact, leaves the lift and the contact. A whole tap goes the same way while the frame loop is busy: `[lift, contact]` plus a lift gives `[lift, lift]`, and the next contact gives `[lift, contact]`.
- Evidence:
  ```rust
  Some(back) => {
      !is_report(&read) || is_contact(back) || !is_report(back) || reads.is_full()
  }
  ```
  The function copied into a host program, fed `Lifted`, `CoverGesture`, then `Points`, holds `[Ok(Lifted(TouchPoint { x: 1, y: 2 })), Ok(Points([TouchPoint { x: 1, y: 2 }]))]`.
- Change: Decide what gives when full and say so. Contacts repeat while a finger is down, so dropping the new contact rather than overwriting a lift or cover loses nothing. Move the policy into a host-tested function (see Tests, "Pure firmware logic").

### An RTC year register that is not BCD reads as year 255, shown as 2255
- Where: crates/octowhere-peripherals/src/rtc.rs:126, 62-69, 189-195; firmware/src/main.rs:1150-1160
- Category: Bugs
- Weight: low
- What: `bcd_to_dec_checked` returns the sentinel 0xFF for a nibble over 9, and `get_time` relies on `is_valid` to reject it. `is_valid` checks every field but the year, so a corrupt year passes as 255. `sensor_task` then builds `year: 2000 + i32::from(time.year)`. The field's own comment says `// 0-99 (2000-2099)`.
- Evidence:
  ```rust
  year: bcd_to_dec_checked(buf[6]),
  ...
  pub fn is_valid(&self) -> bool {
      self.seconds < 60 && self.minutes < 60 && self.hours < 24 && self.weekday < 7
          && (1..=12).contains(&self.month)
          && (1..=days_in_month(self.year, self.month)).contains(&self.day)
  ```
  A fake bus answering `[0x00, 0x00, 0x12, 0x01, 0x03, 0x01, 0xAB]` gives `Ok(DateTime { seconds: 0, minutes: 0, hours: 12, day: 1, weekday: 3, month: 1, year: 255 })`. The test `invalid_bcd_and_calendar_values_are_rejected` (rtc.rs:231-235) checks the sentinel, not `get_time`.
- Change: Make `bcd_to_dec_checked` return `Option<u8>` and fail `get_time` on any `None`, or check `year < 100` in `is_valid`. Test it through `get_time` with a fake bus.

## Structure

### The board's group store is split between main.rs and mesh/device.rs
- Where: firmware/src/main.rs:241-250 (`GROUP_WRITES`, `GROUP_QUEUED`, `GROUP_DONE`, `GROUP_RESULTS`, `GROUP_SAVED`), :975-1007 (`queue_group_write`, `send_group_write`, `group_result`, `group_saved`), :946-958 (`settings_task`'s half); firmware/src/mesh/device.rs:157-175
- Category: Structure
- Weight: medium
- What: `BoardGroupStore` only forwards each method to `crate::`. The numbering lives in three places that must agree: the queuing side numbers a write `GROUP_QUEUED + 1`, `settings_task` numbers it again as `GROUP_DONE + 1` and sets bit `number % 32`, and `group_result` reads that bit back with its own wrap rule.
- Evidence:
  ```rust
  Some(GROUP_QUEUED.fetch_add(1, Ordering::Relaxed).wrapping_add(1))            // :980
  let number = GROUP_DONE.load(Ordering::Relaxed).wrapping_add(1);               // :948
  let bit = 1 << (number % 32);                                                  // :949
  .then(|| GROUP_RESULTS.load(Ordering::Relaxed) & 1 << (number % 32) != 0)      // :996
  ```
- Change: One module (`mesh/store.rs`, or `settings.rs`) holding the queue, the counters and a small type with `queue`, `send`, `result`, `saved` and `finish(saved)`, which `BoardGroupStore` and `settings_task` both call. The number is then computed once.

### frame_loop does a dozen jobs in one 280-line body
- Where: firmware/src/main.rs:2616-2899
- Category: Structure
- Weight: medium
- What: In one body: waking on seven sources into a positional six-tuple of `Option`s; copying the mesh view and messages; converting touch reports, power keys and the sensor snapshot into stage input; stepping; mapping the stage's store choice to a settings write and a zone choice; a two-swap countdown for that write; a second two-swap countdown, written separately, for power-off; damage bookkeeping and the draw.
- Evidence:
  ```rust
  let (touch_read, sensor_state, motion_state, boot, key, boot_key) = match select4(   // :2680
      ...
      Either4::Second(Either4::Fourth(Either::First(key))) => {
          (None, None, None, None, None, Some(key))
  ```
  and the countdowns `pending_write: Option<(settings::Write, u8)>` (:2633, :2883-2889) and `power_off_after: Option<u8>` (:2636, :2890-2897).
- Change: An enum for the wake; plain functions for the conversions (`touch_input`, `sensors`, `write_for(choice) -> (Write, Option<ZoneChoice>)`); one "after n swaps" countdown type used for both the write and the power-off.

### The I/O expander is driven three ways, and its output value is copied by hand
- Where: firmware/src/main.rs:2380-2408 (`reset_lora`, through the tca9554 driver), :132-164 (`LoraPath`, raw writes of register 0x01 with its own cached value), :2462-2476 (`pulse_gnss_reset`, raw read and write of register 0x03), :2313-2318 (`LoraPath`'s initial value)
- Category: Structure
- Weight: medium
- What: `pulse_gnss_reset`'s doc states the rule: the GNSS reset's output bit stays low in every write. Three functions keep it with their own literal masks. `LoraPath`'s cached output must equal `reset_lora`'s last write, and `bring_up` types that value again. The tca9554 driver, a patched fork per AGENTS.md, does the first six register operations at boot and is then dropped, so the run-time writes bypass its cache.
- Evidence:
  ```rust
  exio.write_output(!(gps_reset | lora_reset | lora_tx_switch))        // :2401, reset_lora's last write
  path: LoraPath::new(i2c.clone(), !((1 << board::EXIO_GPS_RESET)      // :2313-2317
          | (1 << board::EXIO_LORA_RESET) | (1 << board::EXIO_LORA_TX_SWITCH))),
  ```
- Change: One expander type in the firmware owning the output and direction values and the register addresses, with the RF switch, radio reset and GNSS reset as methods; `LoraPath` holds it. Or keep the tca9554 driver past boot and go through it.

### SwapThread's unsafe Send and Sync impls are ones the compiler derives anyway
- Where: firmware/src/util.rs:222-224
- Category: Structure
- Weight: low
- What: `SwapThread` holds a `&'s Swap<T>` and three bools. With `Swap<T>: Sync` for `T: Send` (util.rs:221), the auto traits already give `SwapThread<T>: Send + Sync` for exactly `T: Send`. The two extra `unsafe impl`s add unsafe surface and carry a comment as if they were needed.
- Evidence: A copy with the two impls removed still passes both of `swap_accepts_send_values_that_are_not_sync`'s assertions, and still refuses `SwapThread<MutexGuard<'_, ()>>` as `Send` with E0277, as host-tests' `SWAP_SEND_BOUND` doc test expects.
- Change: Delete util.rs:222-224.

## Duplication

### The display bus's transfer is written eight times, blocking and async
- Where: firmware/src/drivers/qspi_bus.rs:62-127 and 136-197 (`execute_async`, `execute`), :216-241 and 244-264 (`begin_quad_write_async`, `begin_quad_write`); firmware/src/drivers/co5300.rs:446-484 and 485-511 (`flush_buf_async`, `flush_buf`), :232-240 and 242-250 (`hw_reset_async`, `hw_reset`)
- Category: Duplication
- Weight: medium
- What: Setting up `half_duplex_write_buffer` and restoring `spi`, `tx` and CS on its error path is written six times in qspi_bus.rs. PixelStream writes it twice more by reaching into `QspiBus`'s `pub(crate) spi` and `cs`. Each async/blocking pair differs only by `wait_for_done().await`. Within each `execute` the two branches differ only by `EmptyBuf` against `tx`. The opcodes 0x02 (single write) and 0x12 (quad write) are bare literals. The two resets disagree: the async one sets reset high and straight back low with no delay between, the blocking one does not.
- Evidence: `grep -c half_duplex_write_buffer` gives qspi_bus.rs 6 and co5300.rs 2. `grep -n "Command::_8Bit(0x" qspi_bus.rs` gives 0x02 at 81, 101, 155, 173 and 0x12 at 225, 250.
  ```rust
  self.reset.set_high();   // co5300.rs:235
  self.reset.set_low();    // :236
  ```
- Change: One function in `QspiBus` that starts a transfer and owns the take-and-restore, with named opcodes. Most of the blocking half has no caller (Public surface, "The display driver's blocking path"). Do this with BACKLOG's "Move the CO5300 driver into its own crate".

### Date and time conversions between the RTC, GNSS and tz types are written five times
- Where: firmware/src/main.rs:1075-1088 and 1115-1126 (to the RTC, each with `(year % 100) as u8` and a `2000..=2099` check), :1150-1160 (RTC to tz, `2000 + i32::from(time.year)`), :2430-2439 (RTC to `GnssDateTime`, `2000 + u16::from(time.year)`), :1507-1518 (GNSS to Unix microseconds); `"20{:02}"` log formats at :2187 and :2442
- Category: Duplication
- Weight: medium
- What: The century offset and the range check are repeated at each site. `sensor_task`'s two RTC-setting blocks also repeat the seven positional `u8` arguments of `RtcDateTime::with_weekday`, the write and its logging.
- Evidence:
  ```rust
  let rtc_time = RtcDateTime::with_weekday(
      (time.year % 100) as u8, time.month, time.day, time.weekday(),   // :1080-1088
  ...
  let rtc_time = RtcDateTime::with_weekday(
      (utc.year % 100) as u8, utc.month, utc.day, utc.weekday,        // :1118-1126
  ```
- Change: A constructor taking the full year and returning `Option`, and a full-year accessor, on `rtc::DateTime` in octowhere-peripherals (no tz dependency needed); one helper in `sensor_task` that sets the RTC and logs where the time came from.

### The heading and the vector helpers are written twice in octowhere-motion
- Where: crates/octowhere-motion/src/compass.rs:469-502 (`attitude`, `cross`, `normalize`), :452 (`length`); crates/octowhere-motion/src/fusion.rs:60-75 (`cross`, `length`, `normalize`), :293-312 (`Fusion::attitude`); test helpers compass.rs:618-625 and fusion.rs:329-336
- Category: Duplication
- Weight: low
- What: The heading from the top edge is the same code in both `attitude` functions: the gate `> 0.2`, unnamed in both, then `atan2f` and the wrap into 0..360. `cross`, `length` and `normalize` are defined in both modules, `normalize` with different thresholds (`1e-3` and `1e-6`). `compass::attitude` is public, but only its own tests call it; the firmware uses `Fusion::attitude` (main.rs:882). Both test modules define `close` and `heading`, with different signatures.
- Evidence: `grep -n "fn cross\|fn length\|fn normalize\|> 0.2" crates/octowhere-motion/src/*.rs` gives fusion.rs:60, 68, 72, 299 and compass.rs:452, 476, 491, 499. `grep -rnw attitude` outside the motion crate gives only main.rs:882.
- Change: A private module with the vector helpers and one `heading_of(top)` with the 0.2 named. Make `compass::attitude` test-only, or use it as the reference in fusion's tests.

### decimal_year re-implements octowhere-tz's civil-date conversion
- Where: crates/octowhere-motion/src/declination.rs:186-207; crates/tz/src/civil.rs:80-107
- Category: Duplication
- Weight: low
- What: Howard Hinnant's algorithm, in both directions, a second time (`719_468`, `146_097` and the era arithmetic). octowhere-tz exposes `days_from_civil` and `civil_from_days` publicly.
- Evidence: `grep -rn "719_468\|146_097" crates --include=*.rs` gives declination.rs:191-204 and civil.rs:87-95.
- Change: Have `ui::members::declination` work out the decimal year with octowhere-tz and pass it in, or let octowhere-motion depend on octowhere-tz without its `boundaries` feature.

### save_mesh encodes the removal state twice, and the two loaders read through near-identical closures
- Where: firmware/src/settings.rs:454-459 and 472-477; :308-317 and 362-377
- Category: Duplication
- Weight: low
- What: `GroupWrite::Rekey` and `GroupWrite::Group` each encode a `Rekey` into a `STORED_MAX` buffer, copy it behind the version byte and write `KEY_REKEY`. `load` and `load_mesh` each build an async closure that reads a key into a `heapless::Vec`, differing only by the version-byte check.
- Evidence:
  ```rust
  let mut bytes = [0; rekey::STORED_MAX];
  let len = rekey.encode(&mut bytes);
  value[1..1 + len].copy_from_slice(&bytes[..len]);
  ```
  at both :455-457 and :473-475.
- Change: One helper that writes the removal state, and one reader taking whether a version byte leads. Separate from steps 12 and 13, which cover the group header and the slots.

### What a group write does to storage is written twice, in settings.rs and the simulator
- Where: firmware/src/settings.rs:355-427 and 430-502; crates/octowhere-sim/src/store.rs:49-72 and 89-99
- Category: Duplication
- Weight: low
- What: The simulator's store says it keeps writes "as the firmware's settings store keeps them" (store.rs:1-2), but by its own code. One rule already differs: the board restores the removal state only when a group is stored, the simulator always does.
- Evidence:
  ```rust
  let rekey = match &group {             // settings.rs:408
      Some(_) => value(KEY_REKEY).await.and_then(|bytes| { ... }),
      None => None,
  ```
  against `rekey: self.rekey.clone(),` in `Stored::start` (store.rs:96).
- Change: When steps 12 and 13 move the encodings into the mesh crate, move the apply and restore rules into octowhere-node as well, as a stored state the firmware fills from flash, so the simulator runs the board's rules.

### mesh/device.rs publishes two values by the same pattern, counted in different orders
- Where: firmware/src/mesh/device.rs:31-35 and 79-106 (`VIEW`, `VIEWS`), :38-60 and 132-145 (`MESSAGES`, `MESSAGE_VIEWS`)
- Category: Duplication
- Weight: low
- What: Both keep the latest value behind a lock and a counter beside it. `publish` bumps `VIEWS` inside the lock (:100); `publish_messages` bumps `MESSAGE_VIEWS` after it (:141). Both orders work, since a reader at worst copies once more, but nothing says which is required.
- Change: One published-value type with `publish` and `since`, used for both.

### Smaller repeats in main.rs
- Where: firmware/src/main.rs:1494-1504 and 2549-2553; :962-971; :1181-1186, 1295-1301, 1341-1348, 1425-1430; :529-550; :279-284
- Category: Duplication
- Weight: low
- What: `configure_gnss` repeats `log_gnss_error`'s three arms with the same strings. `settings_task`'s log has four arms where two would do. `GNSS_HEALTH` is updated by the same lock, get and set four times. `start_display_core!` writes the same `SwapState` literal twice. main.rs's `GpsTime` copies `octowhere_node::GpsTime`, with `Instant` in place of `i64`.
- Change: Call `log_gnss_error` from `configure_gnss`; a helper that updates the health through a closure; `SwapState::new(fb)`; store `octowhere_node::GpsTime` in `GPS_TIME`.

### The radio's IRQ masks are copied from its driver
- Where: firmware/src/mesh/radio.rs:25-26; crates/sx127x-lora/src/registers.rs:24 and 27
- Category: Duplication
- Weight: low
- What: `IRQ_TX_DONE: u8 = 0x08` and `IRQ_RX_DONE: u8 = 0x40` restate `IRQ_FLAGS_TX_DONE_MASK` and `IRQ_FLAGS_RX_DONE_MASK`, which the same file already imports from (`sx127xlora::registers`).
- Change: Use the driver's constants.

### The replay example copies main.rs's MAG_AXES
- Where: host-tests/examples/replay_calibration.rs:19-20; firmware/src/main.rs:757-758
- Category: Duplication
- Weight: low
- What: `/// MAG_AXES in firmware/src/main.rs.` and a copy of the value; nothing checks that they agree.
- Change: Move `IMU_AXES` and `MAG_AXES` somewhere both can import, such as `octowhere-ui`'s `board` module, which the firmware already re-exports.

## Names and types

### Latitude and longitude travel as (i32, i32) tuples
- Where: firmware/src/main.rs:300 (`ZONE_FIX`), :401 (`SensorSnapshot.position`), :420 (`ZoneTracker.looked_up_at`), :216 (`fix_inject::position() -> Option<((i32, i32), bool)>`), :1197-1199
- Category: Names and types
- Weight: low
- What: Which half is latitude is known only by convention. The fix-inject path unpacks `position.0` and `position.1` into `mesh::Fix`'s named `latitude` and `longitude`, and its function returns a positional bool beside the pair.
- Change: A small position type, shared with `ui::screens::Gnss.position` if ui-faces agrees; a named struct for the injected fix.

### SecondCore names its pins by number, and dma_ch0 holds DMA channel 2
- Where: firmware/src/main.rs:100-112, 571, 607
- Category: Names and types
- Weight: low
- What: `dma_ch0: peripherals::DMA_CH2<'static>` misleads. `Parts` names its pins by role (`lora_sck`, `touch_int`); `SecondCore` uses `gpio4` to `gpio39` and maps them to roles only at the use in `second_core`.
- Change: `dma` and role names (`sck`, `sio0`..`sio3`, `cs`, `te`, `reset`), as `Parts` does.

### Two names for one value in the peripheral drivers
- Where: crates/octowhere-peripherals/src/power.rs:9 and 36; crates/octowhere-peripherals/src/touch.rs:31 and 39, 30 and 174
- Category: Names and types
- Weight: low
- What: `REG_STATUS2` and `REG_CHG_STATUS` are both 0x01. `ACK_VALUE` repeats `CST92XX_ACK` and nothing uses it. `Cst9217::new` writes `addr: 0x5A` beside the unused `CST92XX_BOOT_ADDRESS = 0x5A`.
- Change: One name each; use the address constant in `new`.

## Unstated invariants and magic numbers

### is_charging_status counts trickle charge as not charging, unexplained
- Where: crates/octowhere-peripherals/src/power.rs:254-256, test :274-283
- Category: Unstated invariants and magic numbers
- Weight: low
- What: The AXP2101 datasheet gives REG 01H[2:0] = 000 as "tri_charge" (trickle charge). The code and its test treat 000 as not charging, with no comment. 000 is also the register's reset value, which may be the reason, but the code does not say so.
- Evidence: `matches!(status & 0x07, 0b001..=0b011)` and `assert!(!is_charging_status(0b000));`; datasheet `docs/datasheets/AXP2101_SWcharge_V1.0.pdf`, 6.13.2.2: "000: tri_charge 001: pre_charge 010: constant charge(CC) 011: constant voltage(CV) 100: charge done 101: not charging".
- Change: State why in a comment, or use bits 6:5 (battery current direction, 01 = charge), which the same register offers.

### Literals with a second side, or repeated with different meanings
- Where: firmware/src/mesh.rs:78-135, 89-94, 107-112; tools/pair-inject.py:40-41; firmware/src/main.rs:1249 and 1364; :1029, 1355, 1664, 2670
- Category: Unstated invariants and magic numbers
- Weight: low
- What: The pair-inject command codes 1 to 15 are bare literals that `CODES` in tools/pair-inject.py must match, and the arms run out of order (… 11, 13, 14, 15, 12, 10). The 16-byte name and 160-byte text match the statics' 4 and 40 words only by arithmetic. `nmea[..available.min(512)]` repeats the buffer's length. `Duration::from_millis(250)` appears four times with three meanings (the sensor period, the GNSS retry, the link test's period, the frame loop's idle wait), beside `MOTION_PERIOD` of the same value.
- Change: Name the codes, in order, with a comment pointing at the script; derive the byte limits from the statics; `nmea.len()`; one named constant per meaning.

### Unsafe blocks without SAFETY comments, one of them avoidable
- Where: firmware/src/main.rs:585-587, 1968-1976, 7, 114, 558-559
- Category: Unstated invariants and magic numbers
- Weight: low
- What: `main` transmutes `&mut executor` to `'static` with no SAFETY comment, though `CORE1_EXECUTOR` (:115) shows the `StaticCell` way. The PSRAM block has no SAFETY comment either. `CORE1_STACK` is the only `static mut`, and the only reason for `#![expect(static_mut_refs)]`.
- Change: `StaticCell`s for the core 0 executor and the core 1 stack, which drops the `expect`; a SAFETY comment on the PSRAM region.

## Comments

### Stale doc comments
- Where: firmware/src/main.rs:1649, 2614-2615, 274; firmware/src/board.rs:7; firmware/src/drivers/co5300.rs:3, 347; firmware/src/drivers/qspi_bus.rs:2; firmware/src/mesh.rs:64, 67; AGENTS.md:427, 439
- Category: Comments
- Weight: medium
- What:
  - `radio_task`: "Without a link test it leaves the radio as start-up configured it." Without one it runs the mesh (:1723-1735), which retunes the radio.
  - `frame_loop`: "Touch is read once boot has put the controller in `touch_slot`." Nothing is called `touch_slot`; reads come from `TOUCH_READS`.
  - `GPS_TIME`: "see [`gps_utc`]", a function nothing calls (:288).
  - board.rs: "Drivers in `src/peripherals/` own theirs privately." They are in `crates/octowhere-peripherals`.
  - co5300.rs: "Resolution: 410x502, col_offset=22, RGB565." This panel is 466 × 466 at column offset 6 (board.rs:16).
  - `set_brightness`: "0xD0 = default". The firmware's default is 120 (`DEFAULT_BRIGHTNESS`, crates/octowhere-ui/src/ui/screens.rs:47), and init writes 0x00.
  - qspi_bus.rs: "Uses SpiDmaBus". It uses `SpiDma`.
  - `DEAF_UNTIL_MS` and `is_deaf`: "a pairing drops every frame it hears". `BoardRadio::read_packet` (radio.rs:150-154) drops every packet, the mesh's too.
  - AGENTS.md:439: `touch_task` reads "every 10 ms while a finger is down". `touch_task` has no timer; the frame loop asks every 16,667 µs (`TOUCH_REPOLL`, main.rs:2629). AGENTS.md:427 says UTC is "read with `gps_utc`".
- Change: Correct or delete each.

### Commented-out code in the display driver
- Where: firmware/src/drivers/co5300.rs:537-611, 25, 29, 50-51, 100-103
- Category: Comments
- Weight: low
- What: A 75-line `DrawTarget` impl, four commented command constants and a commented `PIXFMT` block.
- Change: Delete them; git keeps them.

### "Undefined" for a value that is simply absent
- Where: crates/octowhere-motion/src/declination.rs:15
- Category: Comments
- Weight: low
- What: "How close to a pole, in degrees of latitude, the declination is left undefined." The function returns `None` there. The owner's rule keeps "undefined" for undefined behaviour.
- Change: "… no declination is given."

## Tests

### Pure firmware logic has no test
- Where: firmware/src/drivers/framebuffer.rs:252-274 (`write_row`), :187-193 (the 2 × 2 rounding), :127-134 (`reach`); firmware/src/drivers/co5300.rs:267-291 (`even_window`); firmware/src/main.rs:335-355 (`put_touch_read`), :416-492 (`ZoneTracker`), :993-997 (`group_result`'s wrap)
- Category: Tests
- Weight: medium
- What: Each is plain arithmetic or a queue policy, testable on the host the way host-tests includes util.rs and gnss_time.rs by `#[path]`. But each sits in a file that also imports esp-hal. `write_row` builds the edges of every shifted row. The 2 × 2 rounding is written twice and applied twice in a region flush: `stream_shifted` rounds, then `set_addr_window` rounds again through `even_window`. The touch queue's bug above would have shown in a test.
- Change: Move them into board-free modules (flush geometry, the touch queue, the zone tracker's decisions) that host-tests includes, and test them there.

### Two peripheral tests check constants rather than the code
- Where: crates/octowhere-peripherals/src/rtc.rs:214-220; crates/octowhere-peripherals/src/power.rs:258-271, 128-131, 250
- Category: Tests
- Weight: low
- What: `init_clears_stop_and_selects_24_hour_mode` recomputes the mask and never calls `init`. `adc_masks_match_axp2101_register_bits` checks `ADC_ENABLE_INIT` and `ADC_ENABLE_TRIMMED`, which only the test uses: `init` and `trim_adc_channels` write the same expressions inline.
- Evidence:
  ```rust
  let ctrl1 = 0xFF;
  let configured = ctrl1 & !(CTRL1_STOP | CTRL1_12_24);
  assert_eq!(configured, 0xDD);
  ```
- Change: Drive `init` through a fake bus, as touch.rs's tests do, and assert the register write. Use the two constants in `init` and `trim_adc_channels`, or drop the latter (Public surface).

### host-tests tests the lc76g crate, three tests twice
- Where: host-tests/src/gnss.rs:76-130; crates/lc76g/src/lib.rs:1725, 1817, 1833; host-tests/README.md:13-16
- Category: Tests
- Weight: low
- What: gnss.rs tests a library crate, not a board-side module, which is what the harness is for. `parser_decodes_rmc_position` is a subset of lc76g's `parser_decodes_rmc_position_and_utc`. `parser_rejects_a_bad_checksum_and_recovers` is identical to lc76g's. `parser_reports_pair_acknowledgements` is lc76g's `parser_ignores_proprietary_acknowledgements`, whose name contradicts its own assertion. The mock bus is set up by hand six times (`grep -c "Rc::new(RefCell::new(MockState" host-tests/src/gnss.rs` gives 6). touch.rs's tests write their own fake bus and `block_on`.
- Change: Move gnss.rs to `crates/lc76g/tests/`, drop the three copies, and add a helper that makes the mock.

## Work and stack

### async_main's poll, under the frame loop for the device's life, is 15,920 bytes and holds boot-only code
- Where: firmware/src/main.rs:1947-2059 (`async_main`), :2054-2058 (the `join` with `frame_loop`); inlined from firmware/src/settings.rs:251-287 (`Store::new`, its 3,072-byte partition-table buffer at :254) and :355-427 (`load_mesh`, its 32 `Option<Slot>` at :379), firmware/src/main.rs:2062-2090 (`mesh_start`), firmware/src/mesh.rs:23-25 (`publish_start`)
- Category: Work and stack
- Weight: high
- What: The frame loop runs inside async_main's task poll, so that poll's frame sits under every step and draw. The frame is 15,920 bytes. The frame loop's own poll adds 4,976 and `Stage::advance` 3,472, and a `BUS_EXECUTOR` task preempts on top of that (the radio task's poll is 15,824). The poll includes boot-only code inlined into it: the partition-table read and its MD5 check, the ekv mount and loads, the identity's SHA-512, `queue_group_write` and `blank_view`. Their buffers are boot locals that a non-inlined callee would release on return: the 3,072-byte partition table, and 32 `Option<Slot>` of at least the 127 bytes of a member record's fields each, over 4 KB. What share of the 15,920 bytes they take was not measured.
- Evidence: From the release build at b8ab7ca, with the plan's frame listing:
  ```text
  15920  420b2540 <<embassy_executor::raw::TaskStorage<octowhere::__async_main_task::...>>::poll>   entry a1, 0x3e30
  15824  42113578 <<...TaskStorage<octowhere::__radio_task_task::...>>::poll>
   4976  420c1be8 <octowhere::frame_loop::{closure#0}>                                              entry a1, 0x1370
   3472  4216db60 <<octowhere_ui::ui::stage::Stage>::advance>
  ```
  The poll's literal pool loads `MD5Init`, `MD5Update`, `MD5Final`, `sha2::sha512::compress512`, `octowhere::queue_group_write`, `octowhere_node::node::blank_view` and `octowhere::frame_loop::{closure#0}`. `Store::new`, `Store::load`, `Store::load_mesh` and `mesh_start` have no symbol of their own; only their inner closures do.
- Change: Put the boot steps (settings load, mesh start, publishing the start) behind `#[inline(never)]` functions, or run them before the executor starts, so the long-lived poll keeps only its own locals. Then list the frames again. Related to BACKLOG's "Lay out the 512 KiB of SRAM deliberately"; the plan's baseline records the 15,920 bytes but not their cause.

### sensor_task reads three PMIC registers every 250 ms only for a debug line
- Where: firmware/src/main.rs:1047-1065; crates/octowhere-peripherals/src/power.rs:223-233
- Category: Work and stack
- Weight: low
- What: `battery_present`, `vbus_mv` and `vsys_mv` are used only in the `debug!` at :1062, which the default `DEFMT_LOG=info` compiles out. The VBUS and VSYS reads still run every 250 ms, and STATUS1 is read twice, once for `is_battery_present` and once for `is_vbus_in`. That is 3 of the 7 PMIC transactions each tick on the shared bus, for nothing in a normal build.
- Evidence:
  ```rust
  let present = power.is_battery_present().await.ok();
  let battery_present = present.unwrap_or(false);
  let battery_mv = power.get_battery_voltage().await.ok();
  let vbus_mv = power.get_vbus_voltage().await.ok();
  let vsys_mv = power.get_system_voltage().await.ok();
  ...
  let usb = power.is_vbus_in().await.ok();
  ```
- Change: Read STATUS1 once and take both bits from it. Read VBUS and VSYS only when a log is due, as `motion_task`'s `log` does, or drop them.

### declination recomputes per coefficient what depends only on degree or order
- Where: crates/octowhere-motion/src/declination.rs:166-177; called on each sensor snapshot from crates/octowhere-ui/src/ui/stage.rs:940 (ui-runtime's file)
- Category: Work and stack
- Weight: low
- What: The loop over the 90 coefficients calls `libm::powf(ratio, n + 2)` and `libm::sincosf(m * lambda)` for each one, though there are 12 degrees and 13 orders. libm's `f32` trigonometry computes in `f64`, which this core emulates (BACKLOG, "Xtensa-specific acceleration"). The stage calls it on every sensor snapshot, every 250 ms, though the position and year change slowly.
- Evidence:
  ```rust
  for &(n, m, g, h, g_dot, h_dot) in &COEFFICIENTS {
      ...
      let scale = libm::powf(ratio, n as f32 + 2.0);
      let (sin_m, cos_m) = libm::sincosf(m as f32 * lambda);
  ```
- Change: Work out `ratio^(n+2)` per degree and the sine and cosine per order before the loop. Whether stage.rs should call it only when the position or day changes is ui-runtime's.

### Timings travel between the cores, but each core reads only its own, and only under timing-log
- Where: firmware/src/main.rs:1820-1827, 1865; :671, 688, 734-736; :2869-2870
- Category: Work and stack
- Weight: low
- What: Core 1 writes `vsync_wait`, `spi_time` and `swap_spi`; the frame loop writes `frametime` and `swap_draw`. Each side reads only its own fields, and only in a `timing-log` `info!`. In a normal build the writes are never read, and the struct need not be in `SwapState`.
- Change: Keep each core's timings as locals under `cfg(feature = "timing-log")` and drop the field from `SwapState`.

## Public surface

### The display driver's blocking path and helpers have no caller
- Where: firmware/src/drivers/framebuffer.rs:25-30 and 86-113 (`Flush::flush_blocking`); firmware/src/drivers/co5300.rs:199 (`with_color_format`), 242 (`hw_reset`), 294 (`fill_screen`), 301 (`write_repeat`), 324 (`write_pixels_area`), 386 (`begin_stream`), 425 (`flush_if_needed_and_get_buf`), 21-22 (`X_OFFS`, `Y_OFFS`), 62 and 68 (`pub bus`, `pub te_pin`); firmware/src/drivers/qspi_bus.rs:244 (`begin_quad_write`)
- Category: Public surface
- Weight: medium
- What: Grepping `firmware`, `crates`, `tools` and `host-tests`: `flush_blocking`, `with_color_format`, `fill_screen`, `hw_reset`, `X_OFFS` and `Y_OFFS` have no caller; `write_pixels_area` appears only in the commented-out code. `write_repeat` is called only by those two, `begin_stream` only by `write_repeat` and `flush_blocking`, and `begin_quad_write` only by `begin_stream`. `X_OFFS` and `Y_OFFS` repeat `board::LCD_COL_OFFSET` and `LCD_ROW_OFFSET`. `bus` and `te_pin` are used only inside co5300.rs. This blocking path is most of the reason qspi_bus.rs and `PixelStream` are written twice (Duplication).
- Change: Remove it, or keep what BACKLOG's CO5300 crate move wants, with a test. Make `bus` and `te_pin` private.

### Dead code in the firmware that main.rs's #![expect(unused)] hides
- Where: firmware/src/main.rs:8, 1738-1753 (`bench_repeat`), 288-294 (`gps_utc`), imports at 26 (`Either3`, `select3`), 48 (`GnssOperation`), 53 (`Color`), 56 (`Framebuffer`), 87 (`FRF_MSB`, `IRQ_FLAGS`, `OP_MODE`, `VERSION`); firmware/src/util.rs:11-24 (`widening_copy`), 71-73 (`Swap::release`); firmware/src/board.rs:12 (`CACHE_LINE`); firmware/Cargo.toml:41 (`futures`)
- Category: Public surface
- Weight: low
- What: Each name appears only where it is defined or imported. The firmware has no `futures::` path; it uses `embassy_futures`. AGENTS.md records lib.rs's `#![expect(unused)]` as deliberate. main.rs carries its own, and this list is what it hides. Whether to keep it is the owner's call.
- Evidence: `grep -cw <name> firmware/src/main.rs` gives 1 for each import above. `grep -rnw --include=*.rs` over `firmware/src`, `crates`, `tools` and `host-tests` gives only the definition for `bench_repeat`, `widening_copy` and `CACHE_LINE`. `gps_utc` appears only in its definition and the doc link at main.rs:274.
- Change: Delete them, and the `futures` dependency.

### Unused public API in octowhere-peripherals, one function of which panics
- Where: crates/octowhere-peripherals/src/touch.rs:191-196 (`sleep`), 197-207 (`set_mode`), 41-57 (`Cst9217RunMode`), 184-187 (`with_address`), 364 (`reset`); power.rs:241-243 (`read_status2`), 245-251 (`trim_adc_channels`); rtc.rs:141-143 (`time_valid`), 36 (`DateTime::new`, tests only)
- Category: Public surface
- Weight: low
- What: None has a caller outside its crate. `set_mode` reaches `_ => unimplemented!()` for 8 of the 13 `Cst9217RunMode` variants, and only the unused `sleep` calls it. `trim_adc_channels`' doc claims it saves "a few hundred µA", which nothing measured.
- Change: Remove them, or make them private where the crate uses them (`reset`).

### octowhere-motion's compass module exports tuning constants and methods nothing outside uses
- Where: crates/octowhere-motion/src/compass.rs:24-71, 187, 194, 442, 470
- Category: Public surface
- Weight: low
- What: The 13 `pub const` tuning values, `HardIron::track` and `is_tracking`, `Calibration::strays` and `attitude` have no user outside the crate (`grep -rn` over `crates`, `firmware`, `tools` and `host-tests`, the motion sources excluded).
- Change: Make them `pub(crate)` or private.

### host-tests depends on four crates it does not use
- Where: host-tests/Cargo.toml:9-11, 13
- Category: Public surface
- Weight: low
- What: `embedded-graphics`, `embedded-graphics-core`, `heapless` and `libm` appear nowhere in host-tests' sources. `critical-section` stays: embassy-sync's `AtomicWaker`, which util.rs uses, needs its `std` implementation.
- Evidence: `grep -rn "embedded_graphics\|heapless\|libm\|critical_section" host-tests/` matches only Cargo.toml.
- Change: Remove the four.

## Files read

Read in full:

- firmware/Cargo.toml
- firmware/src/lib.rs, board.rs, util.rs, gnss_time.rs, settings.rs, touch_inject.rs, main.rs
- firmware/src/mesh.rs, mesh/device.rs, mesh/radio.rs, mesh/random.rs, mesh/time.rs
- firmware/src/drivers/mod.rs, qspi_bus.rs, co5300.rs, framebuffer.rs
- host-tests/Cargo.toml, README.md, src/lib.rs, src/gnss.rs, examples/replay_calibration.rs
- crates/octowhere-peripherals: Cargo.toml, src/lib.rs, i2c_helper.rs, rtc.rs, power.rs, touch.rs, magnetometer.rs
- crates/octowhere-motion: Cargo.toml, src/lib.rs, imu.rs, compass.rs, fusion.rs, declination.rs

Skimmed: the 90-row `COEFFICIENTS` table in declination.rs. It is data, and its test checks it against NOAA's values.

Read outside the partition, to check a finding: AGENTS.md; context/BACKLOG.md, WORKING-NOTES.md and MESH-CLEANUP-PLAN.md; crates/lc76g/src/lib.rs's tests; crates/octowhere-sim/src/store.rs; crates/tz/src/civil.rs:80-107; crates/octowhere-ui/src/ui/stage.rs:925-950 and members.rs:120-140; crates/octowhere-node/src/write.rs and the `GroupWrite` sites in node.rs; crates/sx127x-lora/src/registers.rs and types.rs; tools/pair-inject.py:40-41; the AXP2101 and PCF85063A datasheets.
