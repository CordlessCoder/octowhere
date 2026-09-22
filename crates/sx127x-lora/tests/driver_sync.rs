#![cfg(all(feature = "sync", not(feature = "defmt")))]

use core::convert::Infallible;

use embedded_hal::spi::{ErrorType, Operation, SpiDevice};
use sx127x_common::{FSTEP, Sx127xVariant, Sx1276, calculate::frf};
use sx127xlora::{
    Sx1272,
    driver::Sx1272Lora,
    registers::{
        DETECT_OPTIMIZE, FIFO, FIFO_RX_CURRENT_ADDR, FRF_LSB, FRF_MID, FRF_MSB, HOP_CHANNEL,
        HOP_CHANNEL_CRC_ON_PAYLOAD_MASK, INVERT_IQ, INVERT_IQ_2, IRQ_FLAGS, IRQ_FLAGS_RX_DONE_MASK,
        IRQ_FLAGS_RX_TIMEOUT_MASK, MODEM_CONFIG_1, MODEM_CONFIG_2, OP_MODE,
        OP_MODE_LOW_FREQUENCY_MODE_ON_MASK, OP_MODE_MODE_MASK, PA_CONFIG, PAYLOAD_LENGTH,
        PKT_SNR_VALUE, RX_NB_BYTES, TEMP, VERSION,
    },
    types::{
        Bandwidth, CodingRate, HeaderMode, OCP, PowerRamp, SpreadingFactor, Sx127xLoraConfig,
        TxConfig,
    },
};

struct MockSpi {
    registers: [u8; 128],
    fifo: [u8; 256],
    fifo_pointer: usize,
    burst_reads: usize,
    burst_writes: usize,
}

impl Default for MockSpi {
    fn default() -> Self {
        Self {
            registers: [0; 128],
            fifo: [0; 256],
            fifo_pointer: 0,
            burst_reads: 0,
            burst_writes: 0,
        }
    }
}

impl ErrorType for MockSpi {
    type Error = Infallible;
}

struct MockDelay;

impl embedded_hal::delay::DelayNs for MockDelay {
    fn delay_ns(&mut self, _ns: u32) {}
}

impl MockSpi {
    fn read_register(&self, address: u8) -> u8 {
        if address == FIFO {
            self.fifo[self.fifo_pointer]
        } else {
            self.registers[address as usize]
        }
    }

    fn write_register(&mut self, address: u8, value: u8) {
        if address == FIFO {
            self.fifo[self.fifo_pointer] = value;
            self.fifo_pointer = (self.fifo_pointer + 1) & 0xff;
        } else if address == IRQ_FLAGS {
            self.registers[address as usize] &= !value;
        } else {
            self.registers[address as usize] = value;
            if address == sx127xlora::registers::FIFO_ADDR_PTR {
                self.fifo_pointer = value as usize;
            }
        }
    }

    fn read_range(&mut self, address: u8, buffer: &mut [u8]) {
        if address == FIFO {
            self.burst_reads += 1;
            for byte in buffer {
                *byte = self.read_register(FIFO);
                self.fifo_pointer = (self.fifo_pointer + 1) & 0xff;
            }
        } else {
            for (offset, byte) in buffer.iter_mut().enumerate() {
                *byte = self.read_register(address.wrapping_add(offset as u8));
            }
        }
    }

    fn write_range(&mut self, address: u8, buffer: &[u8]) {
        if address == FIFO {
            self.burst_writes += 1;
        }
        for &byte in buffer {
            self.write_register(address, byte);
        }
    }
}

impl SpiDevice for MockSpi {
    fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        let mut cursor = 0;
        let mut command_only = false;
        for operation in operations {
            match operation {
                Operation::Read(buffer) => {
                    self.read_range(cursor, buffer);
                    cursor = cursor.wrapping_add(buffer.len() as u8);
                    command_only = false;
                }
                Operation::Write(buffer) => {
                    if buffer.len() == 1 {
                        cursor = buffer[0] & 0x7f;
                        command_only = true;
                        continue;
                    }
                    if command_only {
                        self.write_range(cursor, buffer);
                        cursor = cursor.wrapping_add(buffer.len() as u8);
                    } else {
                        let command = buffer[0];
                        cursor = command & 0x7f;
                        self.write_range(cursor, &buffer[1..]);
                        cursor = cursor.wrapping_add((buffer.len() - 1) as u8);
                    }
                    command_only = false;
                }
                Operation::Transfer(read, write) => {
                    read.fill(0);
                    if let Some(&command) = write.first() {
                        self.read_range(command & 0x7f, &mut read[1..]);
                    }
                    command_only = false;
                }
                Operation::TransferInPlace(buffer) => {
                    if let Some(&command) = buffer.first() {
                        self.read_range(command & 0x7f, &mut buffer[1..]);
                    }
                    command_only = false;
                }
                Operation::DelayNs(_) => {}
            }
        }
        Ok(())
    }
}

fn sx1272_spi() -> MockSpi {
    let mut spi = MockSpi::default();
    spi.registers[VERSION as usize] = Sx1272::CHIP_VERSION;
    spi
}

#[test]
fn sx1272_initialization_applies_automatic_if_erratum_workaround() {
    let mut spi = sx1272_spi();
    spi.registers[DETECT_OPTIMIZE as usize] = 0xff;

    let driver = Sx1272Lora::new(spi).unwrap();

    assert_eq!(driver.spi.spi.registers[DETECT_OPTIMIZE as usize], 0x7f);
}

#[test]
fn sx1276_initialization_does_not_apply_sx1272_workaround() {
    let mut spi = MockSpi::default();
    spi.registers[VERSION as usize] = Sx1276::CHIP_VERSION;
    spi.registers[DETECT_OPTIMIZE as usize] = 0xff;

    let driver = sx127xlora::driver::Sx1276Lora::new(spi).unwrap();

    assert_eq!(driver.spi.spi.registers[DETECT_OPTIMIZE as usize], 0xff);
}

#[test]
fn sx1272_automatic_if_optimization_does_not_restore_reserved_detect_bit() {
    let mut config = Sx127xLoraConfig::for_variant::<Sx1272>();
    config.auto_optimize = true;
    config.bandwidth = Bandwidth::Bw500kHz;
    let driver = Sx1272Lora::new_with_config(sx1272_spi(), config).unwrap();

    assert_eq!(driver.spi.spi.registers[DETECT_OPTIMIZE as usize] & 0x80, 0);
}

#[test]
fn sx1272_iq_inversion_preserves_reserved_transmit_bit() {
    let mut spi = sx1272_spi();
    spi.registers[INVERT_IQ as usize] = 0x27;
    let mut driver = Sx1272Lora::new(spi).unwrap();

    driver.set_invert_iq(true).unwrap();

    assert_eq!(driver.spi.spi.registers[INVERT_IQ as usize], 0x67);
    assert_eq!(driver.spi.spi.registers[INVERT_IQ_2 as usize], 0);
}

#[test]
fn sx1276_iq_inversion_updates_both_inversion_registers() {
    let mut spi = MockSpi::default();
    spi.registers[VERSION as usize] = Sx1276::CHIP_VERSION;
    spi.registers[INVERT_IQ as usize] = 0x27;
    let mut driver = sx127xlora::driver::Sx1276Lora::new(spi).unwrap();

    driver.set_invert_iq(true).unwrap();

    assert_eq!(driver.spi.spi.registers[INVERT_IQ as usize], 0x66);
    assert_eq!(driver.spi.spi.registers[INVERT_IQ_2 as usize], 0x19);
}

#[test]
fn frequency_programming_uses_three_registers_and_keeps_sx1272_in_hf_mode() {
    let mut driver = Sx1272Lora::new(sx1272_spi()).unwrap();
    driver.set_frequency(868_000_000).unwrap();
    let spi = driver.spi.spi;
    let expected = frf(868_000_000, FSTEP);

    assert_eq!(
        [
            spi.registers[FRF_MSB as usize],
            spi.registers[FRF_MID as usize],
            spi.registers[FRF_LSB as usize],
        ],
        [
            (expected >> 16) as u8,
            (expected >> 8) as u8,
            expected as u8
        ]
    );
    assert_eq!(
        spi.registers[OP_MODE as usize] & OP_MODE_LOW_FREQUENCY_MODE_ON_MASK,
        0
    );
}

#[test]
fn crc_configuration_uses_each_variant_register_layout() {
    let mut sx1272 = Sx1272Lora::new(sx1272_spi()).unwrap();
    sx1272.set_crc(true).unwrap();
    assert_eq!(sx1272.spi.spi.registers[0x1d] & 0x02, 0x02);
    assert!(sx1272.crc().unwrap());

    let mut sx1276_spi = MockSpi::default();
    sx1276_spi.registers[VERSION as usize] = Sx1276::CHIP_VERSION;
    let mut sx1276 = sx127xlora::driver::Sx1276Lora::new(sx1276_spi).unwrap();
    sx1276.set_crc(true).unwrap();
    assert_eq!(sx1276.spi.spi.registers[0x1e] & 0x04, 0x04);
    assert!(sx1276.crc().unwrap());
}

#[test]
fn raw_temperature_runs_the_measurement_and_decodes_the_inverted_value() {
    let mut spi = sx1272_spi();
    spi.registers[TEMP as usize] = 30;
    let mut driver = Sx1272Lora::new(spi).unwrap();

    assert_eq!(driver.raw_temperature(MockDelay).unwrap(), -30);
    assert_eq!(
        driver.device_mode().unwrap(),
        sx127xlora::types::DeviceMode::STDBY
    );
}

#[test]
fn raw_temperature_decodes_positive_values_without_losing_one_degree() {
    let mut spi = sx1272_spi();
    spi.registers[TEMP as usize] = (-30i8) as u8;
    let mut driver = Sx1272Lora::new(spi).unwrap();

    assert_eq!(driver.raw_temperature(MockDelay).unwrap(), 30);
}

#[test]
fn sx1272_rfo_power_does_not_set_reserved_pa_config_bits() {
    let mut sx1272 = Sx1272Lora::new(sx1272_spi()).unwrap();
    let config = TxConfig::new(OCP::default(), 14, PowerRamp::default(), true).unwrap();
    sx1272.configure_tx(config).unwrap();
    assert_eq!(sx1272.spi.spi.registers[PA_CONFIG as usize], 14);

    let mut sx1276_spi = MockSpi::default();
    sx1276_spi.registers[VERSION as usize] = Sx1276::CHIP_VERSION;
    let mut sx1276 = sx127xlora::driver::Sx1276Lora::new(sx1276_spi).unwrap();
    sx1276.configure_tx(config).unwrap();
    assert_eq!(sx1276.spi.spi.registers[PA_CONFIG as usize], 0x7e);
}

#[test]
fn packet_snr_preserves_signed_quarter_db_precision() {
    let mut spi = sx1272_spi();
    spi.registers[PKT_SNR_VALUE as usize] = (-2i8) as u8;
    let mut driver = Sx1272Lora::new(spi).unwrap();

    assert_eq!(driver.last_packet_snr_raw().unwrap(), -2);
    assert_eq!(driver.last_packet_snr().unwrap(), 0);
}

#[test]
fn configuration_writes_sx1272_and_sx1276_modem1_fields_at_their_datasheet_positions() {
    let mut sx1272_config = Sx127xLoraConfig::for_variant::<Sx1272>();
    sx1272_config.bandwidth = Bandwidth::Bw125kHz;
    sx1272_config.coding_rate = CodingRate::Cr4_6;
    sx1272_config.header_mode = HeaderMode::Implicit;
    sx1272_config.use_crc = true;
    let sx1272 = Sx1272Lora::new_with_config(sx1272_spi(), sx1272_config).unwrap();
    assert_eq!(sx1272.spi.spi.registers[0x1d], 0x16);

    let mut sx1276_spi = MockSpi::default();
    sx1276_spi.registers[VERSION as usize] = Sx1276::CHIP_VERSION;
    let mut sx1276_config = Sx127xLoraConfig::default();
    sx1276_config.bandwidth = Bandwidth::Bw125kHz;
    sx1276_config.coding_rate = CodingRate::Cr4_6;
    sx1276_config.header_mode = HeaderMode::Explicit;
    sx1276_config.use_crc = true;
    let sx1276 =
        sx127xlora::driver::Sx1276Lora::new_with_config(sx1276_spi, sx1276_config).unwrap();
    assert_eq!(sx1276.spi.spi.registers[0x1d], 0x74);
    assert_eq!(sx1276.spi.spi.registers[0x1e] & 0x04, 0x04);
}

#[test]
fn configuration_can_leave_sf6_implicit_mode_for_a_regular_explicit_mode() {
    let mut spi = sx1272_spi();
    spi.registers[MODEM_CONFIG_1 as usize] = 0x04;
    spi.registers[MODEM_CONFIG_2 as usize] = 0x60;
    let driver =
        Sx1272Lora::new_with_config(spi, Sx127xLoraConfig::for_variant::<Sx1272>()).unwrap();

    assert_eq!(driver.spi.spi.registers[MODEM_CONFIG_1 as usize] & 0x04, 0);
    assert_eq!(
        driver.spi.spi.registers[MODEM_CONFIG_2 as usize] & 0xf0,
        0x70
    );
}

#[test]
fn tx_uses_burst_fifo_and_clears_stale_interrupts() {
    let mut driver = Sx1272Lora::new(sx1272_spi()).unwrap();
    driver.spi.spi.registers[IRQ_FLAGS as usize] = 0xff;

    driver.tx(&[1, 2, 3, 4]).unwrap();
    let spi = driver.spi.spi;

    assert_eq!(&spi.fifo[128..132], &[1, 2, 3, 4]);
    assert_eq!(spi.registers[PAYLOAD_LENGTH as usize], 4);
    assert_eq!(spi.registers[IRQ_FLAGS as usize], 0);
    assert_eq!(spi.registers[OP_MODE as usize] & OP_MODE_MODE_MASK, 0x3);
    assert_eq!(spi.burst_writes, 1);
}

#[test]
fn clear_interrupt_writes_only_the_selected_write_one_to_clear_bit() {
    let mut driver = Sx1272Lora::new(sx1272_spi()).unwrap();
    driver.spi.spi.registers[IRQ_FLAGS as usize] = 0xff;

    driver
        .clear_interrupt::<sx127xlora::types::RxDone>()
        .unwrap();

    assert_eq!(
        driver.spi.spi.registers[IRQ_FLAGS as usize],
        !IRQ_FLAGS_RX_DONE_MASK
    );
}

#[test]
fn rx_packet_reads_only_the_reported_payload_length() {
    let mut spi = sx1272_spi();
    spi.registers[HOP_CHANNEL as usize] = HOP_CHANNEL_CRC_ON_PAYLOAD_MASK;
    spi.registers[IRQ_FLAGS as usize] = IRQ_FLAGS_RX_DONE_MASK;
    spi.registers[FIFO_RX_CURRENT_ADDR as usize] = 32;
    spi.registers[RX_NB_BYTES as usize] = 3;
    spi.fifo[32..35].copy_from_slice(&[0xa5, 0x5a, 0x11]);

    let mut driver = Sx1272Lora::new(spi).unwrap();
    let packet = driver.rx_packet().unwrap();

    assert_eq!(packet.length, 3);
    assert_eq!(packet.payload(), &[0xa5, 0x5a, 0x11]);
    assert_eq!(packet.payload.len(), 128);
    assert_eq!(driver.spi.spi.burst_reads, 1);
}

#[test]
fn rx_packet_rejects_crc_errors_without_consulting_header_metadata() {
    let mut spi = sx1272_spi();
    spi.registers[IRQ_FLAGS as usize] = IRQ_FLAGS_RX_DONE_MASK | 0x20;

    let mut driver = Sx1272Lora::new(spi).unwrap();
    let result = driver.rx_packet();

    assert!(matches!(
        result,
        Err(sx127xlora::driver::Sx127xError::PacketTermination)
    ));
}

#[test]
fn rx_packet_rejects_reads_before_reception_is_complete() {
    let mut driver = Sx1272Lora::new(sx1272_spi()).unwrap();

    let result = driver.rx_packet();

    assert!(matches!(
        result,
        Err(sx127xlora::driver::Sx127xError::PacketNotReady)
    ));
}

#[test]
fn rx_packet_reports_timeout_before_packet_readiness() {
    let mut spi = sx1272_spi();
    spi.registers[IRQ_FLAGS as usize] = IRQ_FLAGS_RX_TIMEOUT_MASK;
    let mut driver = Sx1272Lora::new(spi).unwrap();

    let result = driver.rx_packet();

    assert!(matches!(
        result,
        Err(sx127xlora::driver::Sx127xError::PacketTermination)
    ));
}

#[test]
fn public_configuration_cannot_bypass_sf6_header_validation() {
    let mut config = Sx127xLoraConfig::default();
    config.header_mode = HeaderMode::Explicit;
    config.spreading_factor = SpreadingFactor::Sf6;

    let result = Sx1272Lora::new_with_config(sx1272_spi(), config);

    assert!(matches!(
        result,
        Err(sx127xlora::driver::Sx127xError::SF6RequiresImplicitHeaderMode)
    ));
}
