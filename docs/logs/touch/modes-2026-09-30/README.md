# Touch controller modes, 2026-09-30

Captured with `touch-mode-probe` on `bench/touch-latency`. Each short BOOT press steps the
controller to its next mode; every read is logged with the report's bytes, and the frame loop
logs a heartbeat each second with the count of INT's edges.

- `all-modes.txt`: normal, low-power scan, normal, gesture, normal, gesture, then a reset and
  normal, entered from the debug mode start-up leaves it in, with the bus's software timeout on.
  Low-power scan reports a held finger every 10 ms, as normal mode does. Four reads failed in it
  and one in normal mode: the controller ended them after 7 of the 15 bytes. Before the
  timeout, each of three such runs froze core 0 within seconds (see `docs/hardware-notes.md`).
  The normal-mode command did not leave gesture mode; the reset did.
- `gesture-from-debug.txt`: gesture mode entered straight from debug mode. Taps, double taps and
  one swipe each way. The controller goes on reporting contacts and adds a gesture code to each
  lift report.
- `gesture-from-normal.txt`: gesture mode entered from normal mode, as the firmware now does.
  Three taps, three double taps and one swipe each way: one report and one INT pulse per
  gesture, 13 in 36 s. Tap `0x10`, left `0x30`, right `0x50`, up `0x60`, down `0x40`; a double
  tap is two taps 127 to 188 ms apart.
