# Sx127x-LoRa
`#![no_std]`, `async`-first driver for the LoRa modem on the Semtech SX127X transceiver built on top of the Rust
[embedded-hal](https://github.com/rust-embedded/embedded-hal).

The fork adds an explicit chip-variant abstraction to the upstream API. It currently supports
the SX1272/73 V2b silicon (`RegVersion = 0x22`) and retains the SX1276-family variant as the
default. Variant-specific register
addresses, bandwidth encodings, RSSI conversion, power-amplifier settings, and low-frequency
mode handling are selected by the type parameter rather than by board code.

The starting point is [ardentTech/sx127x](https://github.com/ardentTech/sx127x). The fork keeps
its async `embedded-hal` interface and extends it where the SX1272 datasheet requires different
register behavior.

### Cargo Features

- `defmt`: include deferred formatting logging functionality
- `half_duplex`: use the full 255-byte FIFO payload capacity instead of the default 128-byte buffer.
- `sync`: modem sync implementation

### Roadmap

- [x] async
- [x] async rp235x examples
- [x] sync
- [x] sync rp235x examples

### Examples

* [RP235x async](https://github.com/ardentTech/sx127x-lora/tree/main/examples/rp235x/async)
* [RP235x sync](https://github.com/ardentTech/sx127x-lora/tree/main/examples/rp235x/sync)

### Resources

* [Datasheet](https://semtech.my.salesforce.com/sfc/p/E0000000JelG/a/2R0000001Rbr/6EfVZUorrpoKFfvaF_Fkpgp5kzjiNyiAbqcpqh9qSjE)
* [Errata](https://semtech.my.salesforce.com/sfc/p/E0000000JelG/a/2R000000HSPv/sqi9xX0gs6hgzl2LoPwCK0TS9GDPlMwsXmcNzJCMHjw)

### License

* [MIT](https://github.com/ardentTech/sx127x-lora/blob/main/LICENSE-MIT)
* [Apache](https://github.com/ardentTech/sx127x-lora/blob/main/LICENSE-APACHE)
