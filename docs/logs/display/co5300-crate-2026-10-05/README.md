# The CO5300 crate on the board, 2026-10-05

Board 1A:38. The flush logs are `bench/flush-shift` at `f5102d5`, on `8f9c10b`, the driver
before its crate, and at `0d3c617`, on the crate. Each is 16 rounds of the bench, every case
64 flushes timed on the display core with no wait for TE. `controls.log` is
`bench/co5300-controls` at `3864947`. `reads-5mhz.log` is the same branch at `6742ac5`, which
reads known registers on both reply lines at 5 MHz.

The flush, median of the rounds' medians:

| Case | Before | After |
| --- | --- | --- |
| Full frame, unshifted | 11,506 µs | 11,476 µs |
| Full frame, at each of the 8 shifted positions | 11,532 to 11,563 µs | 11,485 to 11,515 µs |
| Region 300 × 300 | 4,951 µs | 4,927 µs |
| Region 466 × 120 | 3,080 µs | 3,066 µs |
| Region 2 rows | 97 µs | 93 µs |

Every case is a few microseconds faster on the crate. That is the start of each write: the
pixel write's header now spins on its DMA transfer instead of awaiting an interrupt.

The controls:

- The TE line pulses 59 to 60 times a second for the vertical blanking alone, not at all when
  off, and about 26,400 times a second with each horizontal blanking too, which is about 441
  lines a frame. Back in the first mode, the first second still counts 73 to 360 pulses: the
  datasheet has a change take effect from the next frame.
- Deep standby and back: the controller starts again, takes a full frame, and TE comes back at
  59 a second.
- Every read comes back as zeros: the scan line, and the identity (04h, whose first byte should
  be 33h), the power mode (0Ah) and the pixel format (0Ch), on SIO0 and on SIO1, at 80 MHz and
  at 5 MHz. The DMA receives each byte. Waveshare's schematic takes six lines to the panel's
  connector and leaves its pin 19 without a net; it draws only SIO0 as a line that runs both
  ways. So the panel's reply line is most likely not connected on this board, and nothing here
  can read the controller.
- Partial mode, idle mode, high-brightness mode, sunlight enhancement and the current limit
  show only on the panel. The bench holds each for 6 s and logs the step, and the owner, watching,
  saw each look right.
