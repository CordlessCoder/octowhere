# A returning device on two boards, 2026-10-09

Both boards ran `96f5050`, which added the welcome's returning byte (version 4 of the pairing
frames), with `pair-inject`, `fix-inject` and `rtc-inject`. 1A38 is member 0 and 1C1C member 1.
`1a38.log` and `1c1c.log` are each board's capture from its reset.

1. `tools/pair-inject.py deaf 60` on 1A38, then `leave` on 1C1C. 1C1C told the group it left
   twice, at 150.0 and 160.2 s, and 1A38 dropped both (`deaf to len=133`), so it still held 1C1C
   as member 1.
2. `add` on 1A38 and `join` on 1C1C, then `choose 0` and `accept` on both. Both showed the code
   784798.
3. 1A38 ended with `Done(Added { id: 1, returning: true })` and 1C1C with
   `Done(Joined { id: 1, confirmed: true, returning: true })`, at 352.8 and 352.5 s. They heard
   each other again at 358.4 and 357.8 s.

No warnings or errors. The pairing was given through `pair-inject`, not the screens, so the
panels did not show its outcome.
