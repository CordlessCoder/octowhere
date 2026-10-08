# Host checks

Run the production synchronization and GNSS tests with:

```text
cargo nextest run -p octowhere-host-tests
cargo test -p octowhere-host-tests --doc
cargo clippy -p octowhere-host-tests --all-targets -- -D warnings
```

The second line runs the doctest that nextest does not: a compile-fail check that a `Swap`
half whose value is not `Send` stays on its thread.

`examples/replay_calibration.rs` replays a recorded serial log through the compass calibration;
its header has the command.

The harness includes the production `util`, `gnss_time` and `settings_queue` modules by path. The I2C peripheral
drivers are the `octowhere-peripherals` crate, and the UI, including the compass maths, the dirty
grid and the geometry, the `octowhere-ui` crate; each runs its own tests on the host (see
`AGENTS.md`). Board-only drivers remain outside this host suite.
