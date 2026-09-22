#[cfg(feature = "defmt")]
use defmt::{debug, error};

use crate::constants::PAYLOAD_SIZE;
use crate::registers::*;
use crate::types::*;
use crate::validate;
use crate::{calculate, check};
use sx127x_common::bits::{get_bits, set_bits};
pub use sx127x_common::error::Sx127xError;
use sx127x_common::error::Sx127xError::{InvalidInput, InvalidVersion};
use sx127x_common::spi::{DelayNs, SpiDevice, Sx127xSpi};
use sx127x_common::{FSTEP, Hz, Modem, Sx127xVariant, Sx1276};

// -------------------------------------------------------------------------------------------------
/// Sx127x driver with LoRa modem.
pub struct Sx127xLora<SPI, V = Sx1276> {
    pub spi: Sx127xSpi<SPI>,
    _variant: core::marker::PhantomData<V>,
}
pub type Sx1272Lora<SPI> = Sx127xLora<SPI, sx127x_common::Sx1272>;
pub type Sx1276Lora<SPI> = Sx127xLora<SPI, Sx1276>;

impl<SPI: SpiDevice, V: Sx127xVariant> Sx127xLora<SPI, V> {
    /// Initializes a new instance of the LoRa driver and enables the LoRa modem.
    #[maybe_async::maybe_async]
    pub async fn new(spi: SPI) -> Result<Sx127xLora<SPI, V>, Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.new");
        let mut driver = Self {
            spi: Sx127xSpi::new(spi),
            _variant: core::marker::PhantomData,
        };

        driver.verify_version().await?;
        driver.set_modem(Modem::LoRa).await?;
        if V::CLEAR_DETECT_OPTIMIZE_AUTOMATIC_IF {
            driver.set_automatic_if(false).await?;
        }
        Ok(driver)
    }

    /// Initializes and configures a new instance of the LoRa driver.
    #[maybe_async::maybe_async]
    pub async fn new_with_config(
        spi: SPI,
        config: Sx127xLoraConfig,
    ) -> Result<Sx127xLora<SPI, V>, Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.new_with_config: {:?}", config);
        let mut driver = Self {
            spi: Sx127xSpi::new(spi),
            _variant: core::marker::PhantomData,
        };

        driver.verify_version().await?;
        driver.set_modem(Modem::LoRa).await?;
        if V::CLEAR_DETECT_OPTIMIZE_AUTOMATIC_IF {
            driver.set_automatic_if(false).await?;
        }
        driver.configure(config).await?;
        Ok(driver)
    }

    /// Clears all interrupts.
    #[maybe_async::maybe_async]
    pub async fn clear_all_interrupts(&mut self) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.clear_all_interrupts");

        self.write(IRQ_FLAGS, 0xff).await
    }

    /// Configure band-specific registers 0x61-0x73 based upon the currently programmed frequency.
    ///
    /// See: datasheet section 4.3
    #[maybe_async::maybe_async]
    pub async fn configure_band_specific_registers(
        &mut self,
    ) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.configure_band_specific_registers");

        let frequency = self.frequency().await?;
        let mut byte = self.read(OP_MODE).await?;
        set_bits(
            &mut byte,
            V::low_frequency_mode(frequency) as u8,
            OP_MODE_LOW_FREQUENCY_MODE_ON_MASK,
            OP_MODE_LOW_FREQUENCY_MODE_ON_OFFSET,
        );
        self.write(OP_MODE, byte).await
    }

    /// Configures TX settings.
    #[maybe_async::maybe_async]
    pub async fn configure_tx(&mut self, config: TxConfig) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.configure_tx: {}", config);

        let mut pa_config = if config.use_rfo {
            0
        } else {
            PA_CONFIG_PA_SELECT_MASK
        };
        if V::PA_CONFIG_MAX_POWER_SUPPORTED {
            pa_config |= PA_CONFIG_MAX_POWER_MASK;
        }
        pa_config |= config.power & PA_CONFIG_OUTPUT_POWER_MASK;
        self.write(PA_CONFIG, pa_config).await?;
        if config.use_rfo {
            self.write(V::PA_DAC, V::pa_dac(false)).await?;
        } else {
            self.write(V::PA_DAC, V::pa_dac(config.high_power)).await?;
        }
        self.set_power_ramp(config.ramp).await?;
        self.set_ocp(config.ocp).await
    }

    /// Clears an interrupt.
    #[maybe_async::maybe_async]
    pub async fn clear_interrupt<I: IRQ>(&mut self) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.clear_interrupt");

        // IRQ flags use write-one-to-clear semantics. Reading first adds a
        // race and is unnecessary because writing zero leaves other flags
        // untouched.
        self.write(IRQ_FLAGS, <I as IRQ>::MASK).await
    }

    /// Gets the cyclic redundancy check (CRC) on/off flag.
    ///
    /// See: section 4.1.1.6
    #[maybe_async::maybe_async]
    pub async fn crc(&mut self) -> Result<bool, Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.crc");
        Ok(get_bits(
            self.read(V::CRC_REGISTER).await?,
            V::CRC_MASK,
            V::CRC_OFFSET,
        ) == 1)
    }

    /// Calculates the data rate in bits/s.
    #[maybe_async::maybe_async]
    pub async fn data_rate(&mut self) -> Result<u16, Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.data_rate");
        let coding_rate: f32 = self.coding_rate().await?.into();
        let symbol_rate = self.symbol_rate().await? as f32;
        let spreading_factor = (self.spreading_factor().await? as u8) as f32;
        Ok(calculate::data_rate(
            symbol_rate,
            spreading_factor,
            coding_rate,
        ))
    }

    /// Calculates the frequency error indicatino (FEI) in Hz and ppm.
    #[maybe_async::maybe_async]
    pub async fn fei(&mut self) -> Result<FEI, Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.fei");
        let msb = self.read(FEI_MSB).await?;
        let mid = self.read(FEI_MID).await?;
        let lsb = self.read(FEI_LSB).await?;
        let raw = (((msb as i32) & 0x0f) << 16) | ((mid as i32) << 8) | lsb as i32;
        let fei = if raw & (1 << 19) != 0 {
            raw | !0x0f_ffff
        } else {
            raw
        };

        let bandwidth = self.bandwidth().await?;
        let frequency = self.frequency().await?;

        Ok(FEI::new_for_variant::<V>(bandwidth, fei, frequency))
    }

    /// Gets the carrier frequency in Hz.
    ///
    /// See: datasheet section 4.1.4
    #[maybe_async::maybe_async]
    pub async fn frequency(&mut self) -> Result<Hz, Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.frequency");
        let msb = self.read(FRF_MSB).await? as u32;
        let mid = self.read(FRF_MID).await? as u32;
        let lsb = self.read(FRF_LSB).await? as u32;
        let frf = ((msb << 16) | (mid << 8) | lsb) as f32;
        Ok((frf * FSTEP) as Hz)
    }

    /// Gets the current hop channel.
    ///
    /// See: datasheet section 4.1.1.8
    #[maybe_async::maybe_async]
    pub async fn hop_channel(&mut self) -> Result<u8, Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.hop_channel");
        Ok(self.read(HOP_CHANNEL).await? & HOP_CHANNEL_FHSS_PRESENT_CHANNEL_MASK)
    }

    /// Gets the flag for interrupt `I`.
    #[maybe_async::maybe_async]
    pub async fn interrupt_flag<I: IRQ>(&mut self) -> Result<bool, Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.interrupt_flag");
        Ok(self.read(IRQ_FLAGS).await? & <I as IRQ>::MASK != 0)
    }

    /// Gets the received signal strength indicator (RSSI) in dBm of the last packet received.
    ///
    /// See: datasheet section 3.5.5
    #[maybe_async::maybe_async]
    pub async fn last_packet_rssi(&mut self) -> Result<i16, Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.last_packet_rssi");
        let snr_raw = self.last_packet_snr_raw().await?;
        self.last_packet_rssi_with_snr(snr_raw).await
    }

    /// Gets the signal-to-noise ratio (SNR) of the last packet received.
    #[maybe_async::maybe_async]
    pub async fn last_packet_snr(&mut self) -> Result<i16, Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.last_packet_snr");
        Ok(self.last_packet_snr_raw().await? as i16 / 4)
    }

    /// Gets the signed packet SNR register value in quarter-dB units.
    #[maybe_async::maybe_async]
    pub async fn last_packet_snr_raw(&mut self) -> Result<i8, Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.last_packet_snr_raw");
        Ok(self.read(PKT_SNR_VALUE).await? as i8)
    }

    /// Maps the DIO0 pin signal source.
    ///
    /// See: datasheet table 18
    #[maybe_async::maybe_async]
    pub async fn map_dio0<S: Dio0Signal>(&mut self) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.map_dio0");
        self.map_dio(
            DIO_MAPPING_1,
            <S as Dio0Signal>::VALUE,
            DIO_MAPPING_1_DIO0_MASK,
            DIO_MAPPING_1_DIO0_OFFSET,
        )
        .await
    }

    /// Maps the DIO1 pin signal source.
    ///
    /// See: datasheet table 18
    #[maybe_async::maybe_async]
    pub async fn map_dio1<S: Dio1Signal>(&mut self) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.map_dio1");
        self.map_dio(
            DIO_MAPPING_1,
            <S as Dio1Signal>::VALUE,
            DIO_MAPPING_1_DIO1_MASK,
            DIO_MAPPING_1_DIO1_OFFSET,
        )
        .await
    }

    /// Maps the DIO2 pin signal source.
    ///
    /// See: datasheet table 18
    #[maybe_async::maybe_async]
    pub async fn map_dio2<S: Dio2Signal>(&mut self) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.map_dio2");
        self.map_dio(
            DIO_MAPPING_1,
            <S as Dio2Signal>::VALUE,
            DIO_MAPPING_1_DIO2_MASK,
            DIO_MAPPING_1_DIO2_OFFSET,
        )
        .await
    }

    /// Maps the DIO3 pin signal source.
    ///
    /// See: datasheet table 18
    #[maybe_async::maybe_async]
    pub async fn map_dio3<S: Dio3Signal>(&mut self) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("map_dio3");
        self.map_dio(
            DIO_MAPPING_1,
            <S as Dio3Signal>::VALUE,
            DIO_MAPPING_1_DIO3_MASK,
            DIO_MAPPING_1_DIO3_OFFSET,
        )
        .await
    }

    /// Maps the DIO4 pin signal source.
    ///
    /// See: datasheet table 18
    #[maybe_async::maybe_async]
    pub async fn map_dio4<S: Dio4Signal>(&mut self) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.map_dio4");
        self.map_dio(
            DIO_MAPPING_2,
            <S as Dio4Signal>::VALUE,
            DIO_MAPPING_2_DIO4_MASK,
            DIO_MAPPING_2_DIO4_OFFSET,
        )
        .await
    }

    /// Maps the DIO5 pin signal source.
    ///
    /// See: datasheet table 18
    #[maybe_async::maybe_async]
    pub async fn map_dio5<S: Dio5Signal>(&mut self) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.map_dio5");
        self.map_dio(
            DIO_MAPPING_2,
            <S as Dio5Signal>::VALUE,
            DIO_MAPPING_2_DIO5_MASK,
            DIO_MAPPING_2_DIO5_OFFSET,
        )
        .await
    }

    /// Masks an interrupt
    ///
    /// See: datasheet section 4.1.2.4
    #[maybe_async::maybe_async]
    pub async fn mask_interrupt<I: IRQ>(&mut self) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.mask_interrupt");
        let byte = self.read(IRQ_FLAGS_MASK).await?;
        self.write(IRQ_FLAGS_MASK, byte | <I as IRQ>::MASK).await
    }

    /// Optimize for the current bandwidth.
    ///
    /// See: errata section 2.1
    #[maybe_async::maybe_async]
    pub async fn optimize_bandwidth(&mut self) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.optimize_bandwidth");
        let bandwidth = self.bandwidth().await?;
        self.set_optimize_bandwidth(bandwidth).await
    }

    /// Optimize receiver response for spurious reception of LoRa signal.
    ///
    /// See: errata section 2.3
    #[maybe_async::maybe_async]
    pub async fn optimize_rx_response(&mut self) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.optimize_rx_response");
        let bandwidth = self.bandwidth().await?;
        self.set_optimize_rx_response(bandwidth).await
    }

    /// Generates a random byte using a von Neumann extractor. Will transition to STDBY when done.
    ///
    /// See: https://learn.rakwireless.com/hc/en-us/articles/26580705507607-Random-Numbers-and-LoRa
    #[maybe_async::maybe_async]
    pub async fn random(&mut self) -> Result<u8, Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.random");
        // current config
        let device_mode = self.device_mode().await?;
        let header_mode = self.header_mode().await?;
        let sf = self.spreading_factor().await?;

        // setup for random gen
        self.set_device_mode(DeviceMode::RXCONTINUOUS).await?;
        self.set_header_mode(HeaderMode::Implicit).await?;
        self.set_spreading_factor(SpreadingFactor::Sf6).await?;

        // von Neumann extractor
        let mut byte;
        let mut res = 0u8;
        for _ in 0..8 {
            byte = self.read(RSSI_WIDEBAND).await? & 0x1;
            while byte == self.read(RSSI_WIDEBAND).await? & 0x1 {
                byte = self.read(RSSI_WIDEBAND).await? & 0x1;
            }
            res = (res << 1) | byte;
        }

        // reset config
        self.set_spreading_factor(sf).await?;
        self.set_header_mode(header_mode).await?;
        self.set_device_mode(device_mode).await?;

        Ok(res)
    }

    /// Performs a SPI read.
    #[maybe_async::maybe_async]
    pub async fn read(&mut self, addr: u8) -> Result<u8, Sx127xError<SPI::Error>> {
        self.spi.read(addr).await
    }

    /// Gets the received signal strength indicator (RSSI) in dBm. Can be read at any time (during packet reception or not), and should be averaged to give more
    /// precise results.
    ///
    /// See: datasheet section 3.5.5
    #[maybe_async::maybe_async]
    pub async fn rssi(&mut self) -> Result<i16, Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.rssi");
        Ok(calculate::rssi_dbm::<V>(
            self.frequency().await?,
            self.read(RSSI_VALUE).await? as i16,
        ))
    }

    /// Gets the received signal strength indicator (RSSI) wideband measurement.
    #[maybe_async::maybe_async]
    pub async fn rssi_wideband(&mut self) -> Result<u8, Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.rssi_wideband");
        self.read(RSSI_WIDEBAND).await
    }

    /// Reads the uncalibrated radio temperature in degrees Celsius.
    ///
    /// The result is only a relative indication. The SX127x temperature
    /// register uses one degree per LSB with an inverted sign and requires the
    /// frequency synthesizer to run during the measurement.
    #[maybe_async::maybe_async]
    pub async fn raw_temperature<D: DelayNs>(
        &mut self,
        mut delay: D,
    ) -> Result<i8, Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.raw_temperature");
        self.set_device_mode(DeviceMode::STDBY).await?;
        self.set_device_mode(DeviceMode::FSRX).await?;
        delay.delay_ms(140).await;
        self.set_device_mode(DeviceMode::STDBY).await?;

        let raw = self.read(TEMP).await?;
        Ok(-(raw as i8))
    }

    /// Gets the received payload from the FIFO buffer.
    ///
    /// See: datasheet figure 10
    #[maybe_async::maybe_async]
    pub async fn rx_packet(&mut self) -> Result<RxPacket, Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.rx_packet");
        let irq_flags = self.read(IRQ_FLAGS).await?;
        #[cfg(feature = "defmt")]
        debug!("irq_flags: 0x{:x}", irq_flags);

        if irq_flags & IRQ_FLAGS_RX_TIMEOUT_MASK != 0 {
            #[cfg(feature = "defmt")]
            error!("Packet termination failed due to RxTimeout");
            return Err(Sx127xError::PacketTermination);
        }
        if irq_flags & IRQ_FLAGS_RX_DONE_MASK == 0 {
            #[cfg(feature = "defmt")]
            debug!("packet is not ready");
            return Err(Sx127xError::PacketNotReady);
        }
        if irq_flags & IRQ_FLAGS_PAYLOAD_CRC_ERROR_MASK != 0 {
            #[cfg(feature = "defmt")]
            error!("Packet termination failed due to PayloadCrcError");
            return Err(Sx127xError::PacketTermination);
        }

        let rx_fifo_addr = self.read(FIFO_RX_CURRENT_ADDR).await?;
        self.write(FIFO_ADDR_PTR, rx_fifo_addr).await?;

        let num_bytes = self.read(RX_NB_BYTES).await?;
        if num_bytes > PAYLOAD_SIZE as u8 {
            #[cfg(feature = "defmt")]
            error!(
                "received {} bytes but buffer size is only {} bytes",
                num_bytes, PAYLOAD_SIZE
            );
            return Err(Sx127xError::InvalidPayloadLength);
        }

        let mut buffer = [0; PAYLOAD_SIZE];
        self.read_fifo(&mut buffer[..num_bytes as usize]).await?;

        let coding_rate = CodingRate::from(get_bits(
            self.read(MODEM_STAT).await?,
            MODEM_STAT_RX_CODING_RATE_MASK,
            MODEM_STAT_RX_CODING_RATE_OFFSET,
        ));
        let snr_raw = self.last_packet_snr_raw().await?;
        let rssi = self.last_packet_rssi_with_snr(snr_raw).await?;
        Ok(RxPacket::new(
            coding_rate,
            num_bytes as usize,
            buffer,
            rssi,
            snr_raw as i16 / 4,
            snr_raw,
        ))
    }

    /// Enables receive mode and searches for a preamble. If a `timeout` is specified, the device
    /// enter RXSINGLE mode, else RXCONTINUOUS mode.
    ///
    /// See: datasheet pages 40-42
    #[maybe_async::maybe_async]
    pub async fn rx(
        &mut self,
        timeout: Option<TimeoutSymbols>,
    ) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.rx: timeout={:?}", timeout);
        self.set_device_mode(DeviceMode::STDBY).await?;
        let mut mode = DeviceMode::RXCONTINUOUS;

        if let Some(timeout) = timeout {
            mode = DeviceMode::RXSINGLE;

            let mut modem_config_2 = self.read(MODEM_CONFIG_2).await?;
            set_bits(
                &mut modem_config_2,
                (timeout.0 >> 8) as u8,
                MODEM_CONFIG_2_SYMB_TIMEOUT_MASK,
                MODEM_CONFIG_2_SYMB_TIMEOUT_OFFSET,
            );
            self.write(MODEM_CONFIG_2, modem_config_2).await?;

            self.write(SYMB_TIMEOUT_LSB, (timeout.0 & 0xff) as u8)
                .await?;
        }

        self.clear_all_interrupts().await?;
        self.write(FIFO_ADDR_PTR, FIFO_RX_BASE_ADDR_VALUE).await?;
        self.set_device_mode(mode).await
    }

    /// Gets the modem rx status.
    ///
    /// See: datasheet section 2.0.2
    #[maybe_async::maybe_async]
    pub async fn rx_status(&mut self) -> Result<RxStatus, Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.rx_status");
        Ok(RxStatus::from(
            self.read(MODEM_STAT).await? & MODEM_STAT_MODEM_STATUS_MASK,
        ))
    }

    /// Sets cyclic redundancy check (CRC) generation and verification on/off.
    ///
    /// See: section 4.1.1.6
    #[maybe_async::maybe_async]
    pub async fn set_crc(&mut self, on: bool) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.set_crc: {}", on);
        let mut byte = self.read(V::CRC_REGISTER).await?;
        set_bits(&mut byte, on as u8, V::CRC_MASK, V::CRC_OFFSET);
        self.write(V::CRC_REGISTER, byte).await
    }

    /// Sets the carrier frequency.
    ///
    /// See: datasheet section 4.1.4, datasheet tables 43-44
    #[maybe_async::maybe_async]
    pub async fn set_frequency(&mut self, hz: Hz) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.set_frequency: {}", hz);
        let frf = sx127x_common::calculate::frf(hz, FSTEP);
        self.write(FRF_MSB, (frf >> 16) as u8).await?;
        self.write(FRF_MID, (frf >> 8) as u8).await?;
        self.write(FRF_LSB, frf as u8).await?;
        self.configure_band_specific_registers().await
    }

    /// Sets the symbol period between frequency hops. If `period` > 0 FHSS will be enabled.
    ///
    /// See: datasheet section 4.1.1.8
    #[maybe_async::maybe_async]
    pub async fn set_hop_period(&mut self, period: u8) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.set_hop_period: {}", period);
        self.write(HOP_PERIOD, period).await
    }

    /// Sets the invert I and Q signals in the Rx or TX path.
    #[maybe_async::maybe_async]
    pub async fn set_invert_iq(&mut self, on: bool) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.set_invert_iq: {}", on);
        let mut byte = self.read(INVERT_IQ).await?;
        set_bits(&mut byte, on as u8, INVERT_IQ_RX_MASK, INVERT_IQ_RX_OFFSET);
        if V::INVERT_IQ_TX_SUPPORTED {
            // see https://github.com/jgromes/RadioLib/issues/778
            set_bits(&mut byte, !on as u8, INVERT_IQ_TX_MASK, INVERT_IQ_TX_OFFSET);
        }
        self.write(INVERT_IQ, byte).await?;
        if V::INVERT_IQ_2_SUPPORTED {
            self.write(
                INVERT_IQ_2,
                if on { INVERT_IQ_2_ON } else { INVERT_IQ_2_OFF },
            )
            .await?;
        }
        Ok(())
    }

    /// Sets the gain and high frequency boost for the low noise receiver amplifier (LNA).
    ///
    /// See: datasheet page 110
    #[maybe_async::maybe_async]
    pub async fn set_lna(&mut self, lna: LNA) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.set_lna: {}", lna);
        let mut byte = self.read(LNA).await?;
        set_bits(
            &mut byte,
            lna.boost_hf as u8,
            LNA_BOOST_HF_MASK,
            LNA_BOOST_HF_OFFSET,
        );
        if lna.gain == LNAGain::Auto {
            self.set_agc_auto(true).await?;
        } else {
            self.set_agc_auto(false).await?;
            set_bits(&mut byte, lna.gain as u8, LNA_GAIN_MASK, LNA_GAIN_OFFSET);
        }
        self.write(LNA, byte).await
    }

    /// Enables/disables low data rate optimization.
    ///
    /// See: datasheet section 4.1.1.6
    #[maybe_async::maybe_async]
    pub async fn set_optimize_data_rate(
        &mut self,
        on: bool,
    ) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("set_optimize_data_rate: {}", on);

        let mut byte = self.read(V::LOW_DATA_RATE_OPTIMIZE_REGISTER).await?;
        set_bits(
            &mut byte,
            on as u8,
            V::LOW_DATA_RATE_OPTIMIZE_MASK,
            V::LOW_DATA_RATE_OPTIMIZE_OFFSET,
        );
        self.write(V::LOW_DATA_RATE_OPTIMIZE_REGISTER, byte).await
    }

    /// Sets the payload length.
    #[maybe_async::maybe_async]
    pub async fn set_payload_length(&mut self, len: u8) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.set_payload_length: {}", len);
        self.write(PAYLOAD_LENGTH, len).await
    }

    /// Determines if low data rate optimization is necessary.
    ///
    /// See: datasheet page 31 section Low Data Rate Optimization
    #[maybe_async::maybe_async]
    pub async fn should_optimize_low_data_rate(
        &mut self,
        bandwidth: Bandwidth,
        spreading_factor: SpreadingFactor,
    ) -> Result<bool, Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!(
            "should_optimize_low_data_rate: {}, {}",
            bandwidth, spreading_factor
        );
        Ok(check::should_optimize_for_low_data_rate(
            calculate::symbol_period(calculate::symbol_rate(
                bandwidth.hz(),
                spreading_factor as u32,
            )),
        ))
    }

    /// Starts the Channel Activity Detector (CAD).
    #[maybe_async::maybe_async]
    pub async fn start_cad(&mut self) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.start_cad");
        self.set_device_mode(DeviceMode::CAD).await
    }

    /// Calculates the symbol rate in chips/s.
    #[maybe_async::maybe_async]
    pub async fn symbol_rate(&mut self) -> Result<u16, Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.symbol_rate");
        let bandwidth = self.bandwidth().await?;
        let spreading_factor = self.spreading_factor().await?;

        Ok(calculate::symbol_rate(bandwidth.hz(), spreading_factor as u32) as u16)
    }

    /// Starts transmitting a payload and returns after the radio enters TX mode.
    ///
    /// The radio transitions to standby after the transmission completes. The
    /// caller can observe that transition through the mapped `TxDone` signal.
    ///
    /// See: datasheet figure 9
    #[maybe_async::maybe_async]
    pub async fn tx(&mut self, payload: &[u8]) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.tx: {:a}", payload);
        let payload_len = payload.len();
        if payload_len > PAYLOAD_SIZE {
            #[cfg(feature = "defmt")]
            error!(
                "payload length {} bytes is greater than max allowed {} bytes",
                payload_len, PAYLOAD_SIZE
            );
            return Err(Sx127xError::InvalidPayloadLength);
        }

        self.write(FIFO_TX_BASE_ADDR, FIFO_TX_BASE_ADDR_VALUE)
            .await?;

        self.set_device_mode(DeviceMode::STDBY).await?;
        self.clear_all_interrupts().await?;
        self.write(FIFO_ADDR_PTR, FIFO_TX_BASE_ADDR_VALUE).await?;
        self.write_fifo(payload).await?;
        self.set_payload_length(payload_len as u8).await?;
        self.set_device_mode(DeviceMode::TX).await
    }

    /// Reads bytes from the current FIFO address and advances the FIFO pointer.
    #[maybe_async::maybe_async]
    pub async fn read_fifo(&mut self, buffer: &mut [u8]) -> Result<(), Sx127xError<SPI::Error>> {
        if buffer.len() > PAYLOAD_SIZE {
            return Err(Sx127xError::InvalidPayloadLength);
        }
        self.spi.read_burst(FIFO, buffer).await
    }

    /// Writes bytes at the current FIFO address and advances the FIFO pointer.
    #[maybe_async::maybe_async]
    pub async fn write_fifo(&mut self, payload: &[u8]) -> Result<(), Sx127xError<SPI::Error>> {
        if payload.len() > PAYLOAD_SIZE {
            return Err(Sx127xError::InvalidPayloadLength);
        }
        self.spi.write_burst(FIFO, payload).await
    }

    /// Unmasks an interrupt.
    ///
    /// See: datasheet section 4.1.2.4
    #[maybe_async::maybe_async]
    pub async fn unmask_interrupt<I: IRQ>(&mut self) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.unmask_interrupt");
        let byte = self.read(IRQ_FLAGS_MASK).await?;
        self.write(IRQ_FLAGS_MASK, byte & !<I as IRQ>::MASK).await
    }

    /// Gets the number of valid headers received since last transition into Rx mode. Counter is reset in Sleep mode.
    #[maybe_async::maybe_async]
    pub async fn valid_rx_headers(&mut self) -> Result<u16, Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.valid_rx_headers");
        let msb = self.read(RX_HEADER_CNT_VALUE_MSB).await? as u16;
        let lsb = self.read(RX_HEADER_CNT_VALUE_LSB).await? as u16;
        Ok((msb << 8) | lsb)
    }

    /// Gets the silicon's valid-packet counter since the last transition into Rx mode.
    ///
    /// The SX1272/73 V2b errata says this counter is unreliable. Count RxDone
    /// events and inspect the CRC status when using that variant.
    #[maybe_async::maybe_async]
    pub async fn valid_rx_packets(&mut self) -> Result<u16, Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.valid_rx_packet");
        let msb = self.read(RX_PACKET_CNT_VALUE_MSB).await? as u16;
        let lsb = self.read(RX_PACKET_CNT_VALUE_LSB).await? as u16;
        Ok((msb << 8) | lsb)
    }

    /// Performs a SPI write.
    #[maybe_async::maybe_async]
    pub async fn write(&mut self, addr: u8, data: u8) -> Result<(), Sx127xError<SPI::Error>> {
        self.spi.write(addr, data).await
    }

    // PRIVATE -------------------------------------------------------------------------------------

    #[maybe_async::maybe_async]
    async fn last_packet_rssi_with_snr(
        &mut self,
        snr_raw: i8,
    ) -> Result<i16, Sx127xError<SPI::Error>> {
        Ok(calculate::last_packet_rssi_dbm_with_raw_snr::<V>(
            self.frequency().await?,
            self.read(PKT_RSSI_VALUE).await? as i16,
            snr_raw,
            self.read(RSSI_VALUE).await? as i16,
        ))
    }

    /// Gets the bandwidth.
    ///
    /// See: datasheet section 4.1.1.4
    #[maybe_async::maybe_async]
    pub async fn bandwidth(&mut self) -> Result<Bandwidth, Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.bandwidth");
        Ok(Bandwidth::from(V::bandwidth_from_register(
            (self.read(MODEM_CONFIG_1).await? & V::BANDWIDTH_MASK) >> V::BANDWIDTH_OFFSET,
        )))
    }

    /// Gets the cyclic error coding rate (CR).
    ///
    /// See: datasheet section 4.1.1.3
    #[maybe_async::maybe_async]
    pub async fn coding_rate(&mut self) -> Result<CodingRate, Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.coding_rate");
        Ok(CodingRate::from(get_bits(
            self.read(MODEM_CONFIG_1).await?,
            V::CODING_RATE_MASK,
            V::CODING_RATE_OFFSET,
        )))
    }

    /// Configures the LoRa driver.
    #[maybe_async::maybe_async]
    async fn configure(&mut self, config: Sx127xLoraConfig) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.config: {:#?}", config);
        if !config.is_valid() {
            #[cfg(feature = "defmt")]
            error!("SF6 requires implicit header mode");
            return Err(Sx127xError::SF6RequiresImplicitHeaderMode);
        }
        self.set_bandwidth(config.bandwidth).await?;
        self.set_coding_rate(config.coding_rate).await?;
        self.set_frequency(config.frequency).await?;
        if config.spreading_factor == SpreadingFactor::Sf6 {
            self.set_header_mode(config.header_mode).await?;
            self.set_spreading_factor(config.spreading_factor).await?;
        } else {
            self.set_spreading_factor(config.spreading_factor).await?;
            self.set_header_mode(config.header_mode).await?;
        }
        self.set_invert_iq(config.invert_iq).await?;
        self.set_preamble_length(config.preamble_length).await?;
        self.set_sync_word(config.sync_word).await?;
        self.set_crc(config.use_crc).await?;

        if config.auto_optimize {
            self.set_optimize_bandwidth(config.bandwidth).await?;
            let on = self
                .should_optimize_low_data_rate(config.bandwidth, config.spreading_factor)
                .await?;
            self.set_optimize_data_rate(on).await?;
            self.set_optimize_rx_response(config.bandwidth).await?;
        }

        Ok(())
    }

    /// Gets the device mode.
    ///
    /// See: datasheet table 16
    #[maybe_async::maybe_async]
    pub async fn device_mode(&mut self) -> Result<DeviceMode, Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.device_mode");
        Ok(DeviceMode::from(get_bits(
            self.read(OP_MODE).await?,
            OP_MODE_MODE_MASK,
            OP_MODE_MODE_OFFSET,
        )))
    }

    /// Gets the header mode.
    ///
    /// See: datasheet section 4.1.1.6
    #[maybe_async::maybe_async]
    pub async fn header_mode(&mut self) -> Result<HeaderMode, Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.header_mode");
        Ok(HeaderMode::from(get_bits(
            self.read(MODEM_CONFIG_1).await?,
            V::HEADER_MODE_MASK,
            V::HEADER_MODE_OFFSET,
        )))
    }

    /// Maps a signal to a DIO pin.
    #[maybe_async::maybe_async]
    async fn map_dio(
        &mut self,
        register: u8,
        value: u8,
        mask: u8,
        offset: u8,
    ) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!(
            "Sx127xLora.map_dio: {}, {}, {}, {}",
            register, value, mask, offset
        );
        let mut byte = self.read(register).await?;
        set_bits(&mut byte, value, mask, offset);
        self.write(register, byte).await
    }

    /// Sets the automatic gain control (AGC) on/off. Turning this on will drive the LNA gain by the AGC loop as opposed to the configured LnaGain.
    ///
    /// See: datasheet table 24
    #[maybe_async::maybe_async]
    async fn set_agc_auto(&mut self, on: bool) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.set_agc_auto: {}", on);
        let mut byte = self.read(V::AGC_AUTO_REGISTER).await?;
        set_bits(&mut byte, on as u8, V::AGC_AUTO_MASK, V::AGC_AUTO_OFFSET);
        self.write(V::AGC_AUTO_REGISTER, byte).await
    }

    /// Sets the bandwidth.
    ///
    /// See: datasheet section 4.1.1.4
    #[maybe_async::maybe_async]
    pub async fn set_bandwidth(
        &mut self,
        bandwidth: Bandwidth,
    ) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.set_bandwidth: {}", bandwidth);
        let mut byte = self.read(MODEM_CONFIG_1).await?;
        let value = V::bandwidth_register_value(bandwidth as u8).ok_or(InvalidInput)?;
        set_bits(&mut byte, value, V::BANDWIDTH_MASK, V::BANDWIDTH_OFFSET);
        self.write(MODEM_CONFIG_1, byte).await
    }

    /// Sets the cyclic error coding rate.
    ///
    /// See: datasheet section 4.1.1.3
    #[maybe_async::maybe_async]
    pub async fn set_coding_rate(
        &mut self,
        coding_rate: CodingRate,
    ) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.set_coding_rate: {}", coding_rate);
        let mut byte = self.read(MODEM_CONFIG_1).await?;
        set_bits(
            &mut byte,
            coding_rate as u8,
            V::CODING_RATE_MASK,
            V::CODING_RATE_OFFSET,
        );
        self.write(MODEM_CONFIG_1, byte).await
    }

    /// Sets the device mode.
    ///
    /// See: datasheet table 16
    #[maybe_async::maybe_async]
    pub async fn set_device_mode(
        &mut self,
        device_mode: DeviceMode,
    ) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.set_device_mode: {}", device_mode);
        let mut byte = self.read(OP_MODE).await?;
        set_bits(
            &mut byte,
            device_mode as u8,
            OP_MODE_MODE_MASK,
            OP_MODE_MODE_OFFSET,
        );
        self.write(OP_MODE, byte).await
    }

    /// Sets the header mode to explicit or implicit.
    ///
    /// See: datasheet section 4.1.1.6
    #[maybe_async::maybe_async]
    pub async fn set_header_mode(
        &mut self,
        mode: HeaderMode,
    ) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.set_header_mode: {}", mode);
        let sf = self.spreading_factor().await?;
        if !validate::header_mode_sf(mode, sf) {
            #[cfg(feature = "defmt")]
            error!("SF6 requires implicit header mode");
            return Err(Sx127xError::InvalidInput);
        }

        let mut byte = self.read(MODEM_CONFIG_1).await?;
        set_bits(
            &mut byte,
            mode as u8,
            V::HEADER_MODE_MASK,
            V::HEADER_MODE_OFFSET,
        );
        self.write(MODEM_CONFIG_1, byte).await
    }

    #[maybe_async::maybe_async]
    async fn set_ocp(&mut self, ocp: OCP) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.set_ocp: {}", ocp);
        let mut byte = ocp.trim();
        set_bits(&mut byte, ocp.on as u8, OCP_ON_MASK, OCP_ON_OFFSET);
        self.write(OCP, byte).await
    }

    /// Optimize receiver response for spurious reception of LoRa signal.
    ///
    /// See: errata section 2.3
    #[maybe_async::maybe_async]
    async fn set_optimize_rx_response(
        &mut self,
        bandwidth: Bandwidth,
    ) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("set_optimize_rx_response: {}", bandwidth);
        let config = bandwidth.optimized_rx_response();
        self.set_device_mode(DeviceMode::STDBY).await?;

        if let Some(offset) = config.frequency_offset {
            let mut frequency = self.frequency().await?;
            frequency = (frequency as i32 + offset) as u32;
            self.set_frequency(frequency).await?;
        }

        if !V::IF_FREQUENCY_OPTIMIZATION {
            return Ok(());
        }
        self.set_automatic_if(config.automatic_if).await?;
        if let Some(if_freq_2) = config.if_freq_2 {
            self.write(IF_FREQ_2, if_freq_2).await?;
        }
        if let Some(if_freq_1) = config.if_freq_1 {
            self.write(IF_FREQ_1, if_freq_1).await?;
        }
        Ok(())
    }

    #[maybe_async::maybe_async]
    async fn set_automatic_if(&mut self, on: bool) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.set_automatic_if: {}", on);
        let mut byte = self.read(DETECT_OPTIMIZE).await?;
        set_bits(
            &mut byte,
            on as u8,
            DETECT_OPTIMIZE_AUTOMATIC_IF_ON_MASK,
            DETECT_OPTIMIZE_AUTOMATIC_IF_ON_OFFSET,
        );
        self.write(DETECT_OPTIMIZE, byte).await
    }

    /// Optimize for the current bandwidth.
    ///
    /// See: errata section 2.1
    #[maybe_async::maybe_async]
    async fn set_optimize_bandwidth(
        &mut self,
        bandwidth: Bandwidth,
    ) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("set_optimize_bandwidth: {}", bandwidth);
        if !V::HIGH_BANDWIDTH_OPTIMIZATION {
            return Ok(());
        }
        if bandwidth == Bandwidth::Bw500kHz {
            match self.frequency().await? {
                410_000_000..=525_000_000 => {
                    self.write(HIGH_BW_OPTIMIZE_1, 0x02).await?;
                    self.write(HIGH_BW_OPTIMIZE_2, 0x7f).await
                }
                862_000_000..=1_020_000_000 => {
                    self.write(HIGH_BW_OPTIMIZE_1, 0x02).await?;
                    self.write(HIGH_BW_OPTIMIZE_2, 0x64).await
                }
                _ => Ok(()),
            }
        } else {
            self.write(HIGH_BW_OPTIMIZE_1, 0x03).await
        }
    }

    /// Sets the rise/fall time of the power amplifier (PA).
    #[maybe_async::maybe_async]
    async fn set_power_ramp(&mut self, pa_ramp: PowerRamp) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.set_power_ramp: {}", pa_ramp);
        let mut byte = self.read(PA_RAMP).await?;
        set_bits(&mut byte, pa_ramp as u8, PA_RAMP_MASK, PA_RAMP_OFFSET);
        self.write(PA_RAMP, byte).await
    }

    /// Sets the preamble length, minus 4 symbols of fixed overhead. A `length` of 6, which is the
    /// minimum valid preamble length, will yield a total of 10 symbols, and a `length` of 65535
    /// will yield a total of 65539 symbols.
    ///
    /// See: datasheet section 4.1.1.6
    #[maybe_async::maybe_async]
    async fn set_preamble_length(
        &mut self,
        preamble_length: PreambleLength,
    ) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.set_preamble_length: {}", preamble_length);
        self.write(PREAMBLE_MSB, (preamble_length.0 >> 8) as u8)
            .await?;
        self.write(PREAMBLE_LSB, (preamble_length.0 & 0xff) as u8)
            .await
    }

    /// Sets the spreading factor. If SF6, implicit header mode must already be set.
    ///
    /// See: datasheet section 4.1.1.2
    #[maybe_async::maybe_async]
    pub async fn set_spreading_factor(
        &mut self,
        spreading_factor: SpreadingFactor,
    ) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.set_spreading_factor: {}", spreading_factor);
        if spreading_factor == SpreadingFactor::Sf6
            && self.header_mode().await? != HeaderMode::Implicit
        {
            #[cfg(feature = "defmt")]
            error!("SF6 requires implicit header mode");
            return Err(Sx127xError::SF6RequiresImplicitHeaderMode);
        }

        let mut modem_config_2 = self.read(MODEM_CONFIG_2).await?;
        set_bits(
            &mut modem_config_2,
            spreading_factor as u8,
            MODEM_CONFIG_2_SPREADING_FACTOR_MASK,
            MODEM_CONFIG_2_SPREADING_FACTOR_OFFSET,
        );
        self.write(MODEM_CONFIG_2, modem_config_2).await?;

        let mut detect_optimize = self.read(DETECT_OPTIMIZE).await?;
        detect_optimize &= !DETECT_OPTIMIZE_DETECTION_OPTIMIZE_MASK;

        if spreading_factor == SpreadingFactor::Sf6 {
            detect_optimize |= DETECT_OPTIMIZE_DETECTION_OPTIMIZE_SF6;
            self.write(DETECTION_THRESHOLD, DETECTION_THRESHOLD_SF6)
                .await?;
        } else {
            detect_optimize |= DETECT_OPTIMIZE_DETECTION_OPTIMIZE_SF7_TO_SF12;
            self.write(DETECTION_THRESHOLD, DETECTION_THRESHOLD_SF7_TO_SF12)
                .await?;
        }
        self.write(DETECT_OPTIMIZE, detect_optimize).await
    }

    /// Sets the LoRa sync word.
    #[maybe_async::maybe_async]
    async fn set_sync_word(&mut self, sync_word: u8) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.set_sync_word: {}", sync_word);
        self.write(SYNC_WORD, sync_word).await
    }

    /// Gets the spreading factor.
    #[maybe_async::maybe_async]
    pub async fn spreading_factor(&mut self) -> Result<SpreadingFactor, Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.spreading_factor");
        Ok(SpreadingFactor::from(get_bits(
            self.read(MODEM_CONFIG_2).await?,
            MODEM_CONFIG_2_SPREADING_FACTOR_MASK,
            MODEM_CONFIG_2_SPREADING_FACTOR_OFFSET,
        )))
    }

    /// Sets the active modem.
    #[maybe_async::maybe_async]
    async fn set_modem(&mut self, modem: Modem) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xLora.set_modem: {}", modem);
        self.set_device_mode(DeviceMode::SLEEP).await?;

        let mut byte = self.read(OP_MODE).await?;
        set_bits(
            &mut byte,
            modem as u8,
            OP_MODE_LONG_RANGE_MODE_MASK,
            OP_MODE_LONG_RANGE_MODE_OFFSET,
        );
        self.write(OP_MODE, byte).await?;

        self.set_device_mode(DeviceMode::STDBY).await
    }

    /// Verifies the chip version.
    #[maybe_async::maybe_async]
    pub async fn verify_version(&mut self) -> Result<(), Sx127xError<SPI::Error>> {
        let version = self.spi.read(VERSION).await?;
        if version != V::CHIP_VERSION {
            #[cfg(feature = "defmt")]
            error!("invalid chip version: {} != {}", version, V::CHIP_VERSION);
            return Err(InvalidVersion);
        }
        Ok(())
    }
}
