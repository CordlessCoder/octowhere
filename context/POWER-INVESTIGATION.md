# Power, sleep and medium access

Findings of 2026-10-04, from datasheets and from reading esp-hal's and esp-rtos's sources. Nothing
here is measured. It waits for the owner's PPK2, and feeds two decisions:

- whether the mesh keeps its slots or moves to contention (CSMA/CA);
- whether the firmware light-sleeps while the screen is dark.

## Why it came up

The owner revisited contention on 2026-10-04 for two reasons: message latency, and how much of the
protocol the slot timing brings. They suspect the radio's draw is small next to the panel, the
sensors and the drawing. If so, CSMA/CA with continuous receive gives the lowest latency at low
channel use.

The power budget is two days on about 1,000 mAh (owner, 2026-10-03, answering "Step 5 needs a
power budget. The bench boards have no battery fitted. What should it assume?"). Read here as the
whole device: 20.8 mA on average, 500 mAh a day. That reading is an assumption the owner has not
confirmed.

## What the parts draw

Datasheet figures; none measured on the board.

| Part | Current | Note |
| --- | --- | --- |
| ESP32-S3, 240 MHz, both cores in WAITI | 33–48 mA | what the firmware idles in today |
| ESP32-S3, 80 MHz / 40 MHz, both in WAITI | 22–36 / 13–19 mA | if the clock could drop with the screen dark |
| ESP32-S3, 240 MHz, both cores busy | 66–92 mA | drawing; PSRAM adds to it |
| ESP32-S3, light sleep | 240 µA | plus 140–200 µA for 8 MB octal PSRAM |
| SX1272, receive, 125 kHz | 9.7 mA | 10.8 with LNA boost; 233 mAh a day if continuous |
| SX1272, transmit, +17 dBm | 90 mA | |
| SX1272, CAD at 125 kHz | 10.8 mA, then 5.6 mA | (2^SF + 32) / BW receiving, then about a symbol processing |
| LC76G (AB), tracking | 36 mA | ours is the AB |
| LC76G (AB), ALP | unknown | Quectel measured ALP only on the PA |
| QMI8658, 6-axis | about 0.75 mA | at 1.8 V |
| BMM350, 100 Hz | 0.2–0.35 mA | |
| AMOLED panel | unknown | the CO5300 sheet covers the driver chip only |
| CST9217 touch | unknown | no datasheet in `docs/` |

- Continuous receive takes 47% of the budget.
- The ESP32 in WAITI alone, at 33 mA, empties 1,000 mAh in about 30 h. The two-day budget is not
  reachable without light sleep, whatever the radio does.
- An OLED's draw follows the lit pixels and their brightness, and the screens are mostly black.
  The panel may draw less than the drawing does.
- The AXP2101 measures only voltages (VBAT, VBUS, VSYS, TS, die temperature), so the board cannot
  report its own current.

### GNSS low power

ALP works on our module. Quectel's low-power note
(`docs/datasheets/LC76G_Low_Power_Mode_Application_Note_V1.0.pdf`, Table 3) lists ALP for the
LC76G (AB) from firmware `LC76GABNR12A01S`. Ours reports `LC76GABNR12A05S`, and its start-up query
reports ALP on (`docs/hardware-notes.md`). The spec sheet gives no ALP current for the AB, which
is not evidence that it lacks ALP.

Quectel's ALP figures are for the PA: 5.5 mA on the sheet, and 6.8 mA on average over the first
30 minutes after power-on. ALP draws its most until the almanac is complete, about 12.5 minutes.
The PA tracks at 10 mA against the AB's 36, so its ALP figure says nothing of ours.

## ESP32 sleep in esp-rtos

Sources: esp-hal 1.2.2 and esp-rtos 0.4.0, the versions `firmware/Cargo.lock` pins, under
`~/.cargo/registry/src/index.crates.io-*/`.

### Today: WAITI

`firmware/src/main.rs` calls `esp_rtos::start`, whose default idle hook loops on
`wait_for_interrupt` (esp-rtos `src/task/mod.rs:36`). Both cores run esp-rtos executors, so each
waits there when nothing is ready. WAITI stops the core's clock; the PLL and the 240 MHz clock tree
keep running. It is embassy-stm32's default WFE (Sleep mode), not its low-power executor (Stop).

### Automatic light sleep

esp-rtos has it, marked experimental, on the S3 (its metadata sets `sleep.light_sleep`) and on
both cores since esp-hal #5825. Enabling it replaces the `start` call:

```rust
let sleep = esp_rtos::sleep::configure(peripherals.LPWR);
esp_rtos::start_with_idle_hook(timg0.timer0, peripherals.FROM_CPU_INTR0, sleep.light_sleep_hook);
```

Nothing else in the firmware takes `LPWR`. Whenever no task is ready, the hook (esp-rtos
`src/sleep.rs`) light-sleeps if no `WakeLock` is held, both cores are idle, and the next timer is
at least `ESP_RTOS_CONFIG_LIGHT_SLEEP_MIN_US` (default 1,000 µs) away. It parks the other core,
sleeps until that timer or a pin a driver listens on, restores the time and re-arms the timer.

esp-hal drivers that take a `WakeLock`: async I2C transfers, DMA receive futures, ADC conversions,
UART, TWAI, RSA, SPI slave, and the USB-Serial-JTAG driver for its whole life.

### What would bite us

From the sources and the firmware; none tried on a board.

- **SPI DMA writes hold no wake lock.** `SpiDma::wait_for_idle_async` (esp-hal
  `src/spi/master/dma.rs:363`) takes the lock only through `DmaRxFuture`, when a receive is in
  progress. A write alone waits on the SPI's done interrupt with no lock. The display's flushes
  are such writes, awaited with `wait_for_done()` on core 1. With both cores idle and the next
  timer over 1 ms away, the hook could sleep mid-flush. The owner will PR a fix to esp-hal; until
  then, hold a `WakeLock` while the panel is on. Under esp-hal's AI policy the owner writes that
  code; agents research and review.
- **Timekeeping.** Time slept is counted by the RTC timer on the slow clock (esp-hal
  `src/rtc_cntl/sleep/mod.rs:209`). esp-hal defaults to the internal ~136 kHz RC oscillator
  (`src/soc/xtal32k.rs:39`), calibrated once. The board cannot fit a 32 kHz crystal: its pins,
  GPIO15 and GPIO16, are the I2C bus's SDA and the radio's SCLK. Once most time is slept, the
  local timer runs at the RC oscillator's accuracy, not the 40 MHz crystal's. The slot timing and
  `gnss_time` both assume the crystal. The datasheet gives no figure for the RC's drift. Contention
  needs time only to seconds.
- **Deadlock.** The hook's FIXME: parking the other core wherever it stands can deadlock if that
  core holds a cross-core lock the sleep path takes. Upstream accepts the risk for now.
- **PSRAM.** The default light-sleep config keeps VDD_SDIO, the flash and PSRAM supply, powered
  (`vddsdio_pd_en` false). The S3's sleep path has no PSRAM handling that I found. Whether the
  framebuffers and the message store survive is unverified.
- **USB link.** esp-hal's USB-Serial-JTAG driver holds a wake lock for its life, so upstream does
  not expect that link to work through light sleep. The defmt log (esp-println, `jtag-serial`) and
  probe-rs injection bypass that driver, take no lock, and may drop. Bench and inject builds
  probably keep the WAITI hook.
- **Wake cadence.** At rest the frame loop wakes on every sensor snapshot (250 ms), every motion
  snapshot (250 ms, 20 ms while the compass or member face shows) and a 250 ms timeout, and each
  wake hands core 1 a frame. GNSS reads cluster around each second. How much of the time is left
  in gaps over 1 ms is unmeasured.
- **Wake pins.** A listening pin wakes the chip, and an edge wait becomes a level wake
  (esp-hal `src/gpio/mod.rs`, `listen`'s docs). A pin waiting for an edge on a line already at its
  level ends every sleep at once. DIO0 (rising), touch INT (falling) and BOOT (falling) idle at
  the other level. Core 1 waits on TE only when it has something to flush.

Continuous receive and light sleep fit together: the radio receives on its own crystal, and DIO0
wakes the ESP32 for each packet.

## Medium access revisited

`LORA-PROTOCOL.md`, "Why not contention", has the original case. What is new below is the
latency argument, the SX1272's limits, and the power figures set against the whole device.

### Latency

A node sends once per 45 s round, so a message waits about half a round per hop on average, up to
a whole round. The round is 45 s because each of its 32 slots carries a ±250 ms guard. Shorter
slots need step 5's tighter timing, which adds timing code rather than removing it. Under
contention a hop costs a random wait plus the airtime: about 0.1–0.4 s with continuous receive,
under a second with low-power listening at T = 250 ms.

### Radio energy a day

One packet per node per 45 s round, 92 ms packets, receive 9.7 mA, transmit 90 mA, a CAD 19.4 µC
at SF7 and 125 kHz. Low-power listening (LPL): receivers run a CAD every T; senders stretch the
preamble to at least T.

| Listening | 8 neighbours | 31 neighbours | Channel busy, 32 nodes |
| --- | --- | --- | --- |
| Continuous receive | 233 mAh | 233 mAh | 6.5% |
| LPL, T = 100 ms | 15 mAh | 32 mAh | 14% |
| LPL, T = 250 ms | 23 mAh | 49 mAh | 24% |
| Slots as built, with sweeps | about 55 mAh | | 6.5% |
| Slots with step 5's CAD, ideal, with sweeps | 22 mAh | 33 mAh | 6.5% |

- The LPL rows count the CADs, the long preambles' extra transmit time, and receiving each
  neighbour's packet plus half a preamble on average.
- They leave out waking the ESP32. In LoRa mode the SX1272 cannot schedule a receive or a CAD
  itself (its sequencer is FSK-only), so the ESP32 starts every CAD, ten a second at T = 100 ms.
- Slots as built keep each window open about 0.9 s.

### What the SX1272 and the board allow

- **Continuous receive:** an ordinary mode with no time limit. The errata's receive items are
  handled. The RF switch stays on receive between packets, so it costs no I2C traffic.
- **Sensing a busy channel:** CAD finds preambles only (datasheet: "designed to detect a LoRa
  preamble"). Our preamble is about 12.5 ms of a 92 ms packet, so a CAD misses a packet already
  under way. RSSI does not fill the gap: SF7 decodes down to −124 dBm, about 7 dB below the
  125 kHz noise floor (about −117 dBm with a 6 dB noise figure). A node already listening has
  `RegModemStat`'s signal-detected and header-valid flags, and the header's length. So reliable
  sensing comes with continuous receive, or with LPL's long preambles, which CAD sees.
- **Turning round to transmit:** `RX_SWITCH` and `TX_SWITCH` are TCA9554 outputs, so each switch
  to transmit is an I2C write on the shared bus, which a GNSS read holds for about 12 ms. A node
  that finds the channel clear can miss a preamble that starts while it waits for the bus. At 32
  nodes that is about a 1% collision chance per packet among nodes that hear each other, against
  0.14% with a 2 ms gap. Take the bus lock before sensing, sense over SPI, switch, then release.
- **Interrupts:** only DIO0 reaches the ESP32. CadDone maps to DIO0 (mapping 10), and CadDetected
  is read from `RegIrqFlags` over SPI.

### If contention is chosen

- **Relaying:** every neighbour that hears a message wants to pass it on as the packet ends. Each
  needs a random wait longer than it takes to see another's preamble. The modem needs at least 4
  preamble symbols to lock, about 4 ms at SF7, plus the turnaround: wait steps of about 6–8 ms with
  the bus held across the check, about 20 ms without.
- **Hidden nodes:** two nodes that cannot hear each other collide at one between them, and
  broadcasts have no RTS/CTS. Capture needs the wanted packet 6 dB stronger (co-channel
  rejection, −6 dB). At 6.5% load, a node hidden from every sender loses about 12% of packets, as
  pure ALOHA does. The message summaries repair losses, and continuous receive makes retries cheap.
- **What relays at once:** messages. Positions and digests can stay periodic, and sending
  positions less often cuts load and power together.
- **What goes:** the slot schedule and its shuffle, windows and guards, the timebase's ranking,
  roots and hops as a slot clock, sweeps (a node in range is heard at its first packet), notices,
  and holding packets that would move the clock.
- **What stays:** a shared UTC, to seconds, for record stamps, message freshness and the removal's
  switch, which is named in rounds today and would be named in UTC.
- **Regulation:** contention does not need the polite-spectrum-access option. Under band O's 10%
  duty-cycle option a channel check is neither required nor forbidden, as pairing already relies on.

## Measuring with the PPK2

- **Hookup:** no battery is fitted, so all the board's current comes in on VBUS. The PPK2 as an
  ammeter in series with VBUS, through a USB breakout, keeps the USB JTAG for logs and injection
  and reads the 5 V side, the PMIC's losses included. As a source on the battery connector, with
  USB unplugged and charging off, it reads what a cell would supply, but then the firmware must
  step through the states itself.
- **Bench branch:** `bench/power`, on a committed revision, built in a worktree. Features: the
  light-sleep hook with a `WakeLock` while the panel is on; the radio held asleep; the GNSS held in
  reset (`bench/gnss-stuck` has the hold). `lora-link-rx` is continuous receive already.
- **States:** a settled face, a continuous swipe loop (touch-inject), the always-on face, the
  screen dark. Each with the radio asleep and receiving, and with and without the light-sleep
  hook. The GNSS running and held in reset, with a fix and without; ALP needs about 12.5 minutes
  of almanac to settle.
- **Also:** the AB's current in ALP (the difference against reset); the panel's draw by content,
  black against lit; one CAD with its ESP32 wake, for LPL; the RC slow clock's drift against the
  crystal over temperature, with GNSS seconds as the reference, for slots under light sleep.

What the numbers decide:

- If light sleep brings the dark screen down to a few mA, continuous receive is the largest draw.
  Contention then means LPL, or the budget moves.
- If the device stays in WAITI, or wakes too often to sleep much, continuous receive adds about 20%
  to the draw that never stops. CSMA/CA with continuous receive then wins on latency, and the
  two-day budget moves.
- Light sleep's RC timekeeping weighs against slots either way.
