# Host checks

Run the production geometry, dirty-grid, synchronization, and touch tests with:

```text
RUSTUP_TOOLCHAIN=stable cargo test --manifest-path host-tests/Cargo.toml --offline --target x86_64-unknown-linux-gnu
RUSTUP_TOOLCHAIN=stable cargo clippy --manifest-path host-tests/Cargo.toml --offline --target x86_64-unknown-linux-gnu --all-targets -- -D warnings
```

The harness includes the production `util`, `touch`, `dirty`, and geometry modules by path. Board-only drivers remain outside this host suite.
