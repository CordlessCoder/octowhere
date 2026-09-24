# Host checks

Run the production synchronization, peripheral and GNSS tests with:

```text
RUSTUP_TOOLCHAIN=stable cargo test --manifest-path host-tests/Cargo.toml --offline --target x86_64-unknown-linux-gnu
RUSTUP_TOOLCHAIN=stable cargo clippy --manifest-path host-tests/Cargo.toml --offline --target x86_64-unknown-linux-gnu --all-targets -- -D warnings
```

`examples/replay_calibration.rs` replays a recorded serial log through the compass calibration;
its header has the command.

The harness includes the production `util` and I2C peripheral modules by path. The UI, including
the compass maths, the dirty grid and the geometry, is the `octowhere-ui` crate, which runs its own
tests on the host; see `AGENTS.md`. Board-only drivers remain outside this host suite.
