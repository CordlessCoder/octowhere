# Sx127x-Common

Common register, SPI, and chip-variant definitions for the async SX127x drivers. The local fork
supports both the SX1272/73 V2b and SX1276-family register variants.

`async`-first common code for the [FSK](https://crates.io/crates/sx127x-fsk) and [LoRa](https://crates.io/crates/sx127x-lora) modems on the Semtech sx127x chips. While this crate isn't
terribly useful on it's own, it exists to reduce duplication in the downstream modem driver implementations.

### Cargo Features

- `sync`: sync implementation

### Resources
* [Datasheet](https://semtech.my.salesforce.com/sfc/p/E0000000JelG/a/2R0000001Rbr/6EfVZUorrpoKFfvaF_Fkpgp5kzjiNyiAbqcpqh9qSjE)
* [Errata](https://semtech.my.salesforce.com/sfc/p/E0000000JelG/a/2R000000HSPv/sqi9xX0gs6hgzl2LoPwCK0TS9GDPlMwsXmcNzJCMHjw)

### License
* [MIT](https://github.com/ardentTech/sx127x/blob/main/LICENSE-MIT)
* [Apache](https://github.com/ardentTech/sx127x/blob/main/LICENSE-APACHE)
