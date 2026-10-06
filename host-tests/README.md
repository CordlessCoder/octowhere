# Host checks

Run the production synchronization and GNSS tests with:

```text
cargo test -p octowhere-host-tests
cargo clippy -p octowhere-host-tests --all-targets -- -D warnings
```

`examples/replay_calibration.rs` replays a recorded serial log through the compass calibration;
its header has the command.

The harness includes the production `util`, `gnss_time` and `settings_queue` modules by path. The I2C peripheral
drivers are the `octowhere-peripherals` crate, and the UI, including the compass maths, the dirty
grid and the geometry, the `octowhere-ui` crate; each runs its own tests on the host (see
`AGENTS.md`). Board-only drivers remain outside this host suite.
