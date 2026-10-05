//! The mesh's radio: the SX1272, its `DIO0` line, and the RF switch on the I/O expander.

use defmt::warn;
use embassy_futures::select::{Either, select};
use embassy_time::{Duration, Timer};
use esp_hal::gpio::Input;
use sx127xlora::{
    driver::Sx127xError,
    registers::{
        FIFO_ADDR_PTR, FIFO_TX_BASE_ADDR, FIFO_TX_BASE_ADDR_VALUE, IRQ_FLAGS,
        IRQ_FLAGS_VALID_HEADER_MASK, MODEM_STAT, MODEM_STAT_MODEM_STATUS_HEADER_INFO_VALID_MASK,
        MODEM_STAT_MODEM_STATUS_RX_ONGOING_MASK, MODEM_STAT_MODEM_STATUS_SIGNAL_DETECTED,
        MODEM_STAT_MODEM_STATUS_SIGNAL_SYNCHRONIZED, SYNC_WORD as SYNC_WORD_REGISTER,
    },
    types::{DeviceMode, OCP, PowerRamp, RxDone, TxConfig, TxDone},
};

use octowhere_node::{Radio, Received};

use super::time::{local, until};
use crate::{LoraPath, SensorLora};

/// The longest a transmission can take, with margin for `DIO0`.
const SEND_TIMEOUT_US: i64 = 500_000;
/// How often the radio's flags are read where `DIO0` does not follow them.
const POLL_US: u64 = 1_000;
const IRQ_TX_DONE: u8 = 0x08;
const IRQ_RX_DONE: u8 = 0x40;
/// The modem's status while a packet is under way: its preamble detected, then the modem
/// synchronised to it, receiving and its header read.
const MODEM_BUSY: u8 = MODEM_STAT_MODEM_STATUS_SIGNAL_DETECTED
    | MODEM_STAT_MODEM_STATUS_SIGNAL_SYNCHRONIZED
    | MODEM_STAT_MODEM_STATUS_RX_ONGOING_MASK
    | MODEM_STAT_MODEM_STATUS_HEADER_INFO_VALID_MASK;

pub struct BoardRadio {
    lora: SensorLora,
    dio0: Input<'static>,
    /// `DIO0` rose for the last flag it was mapped to. Where it did not, the flags are polled.
    dio0_follows: bool,
    /// In continuous receive since the last mode change. Starting it again would restart the
    /// receiver, and lose a packet under way.
    receiving: bool,
    path: LoraPath,
}

impl BoardRadio {
    pub fn new(lora: SensorLora, dio0: Input<'static>, path: LoraPath) -> Self {
        Self {
            lora,
            dio0,
            dio0_follows: true,
            receiving: false,
            path,
        }
    }

    /// Waits until local time `deadline` for the radio to raise `flag`, the one `DIO0` is mapped
    /// to. Returns whether it did.
    async fn wait_for(&mut self, flag: u8, deadline: i64) -> bool {
        if self.dio0_follows {
            return matches!(
                select(self.dio0.wait_for_high(), until(deadline)).await,
                Either::First(())
            );
        }
        loop {
            if self
                .lora
                .read(IRQ_FLAGS)
                .await
                .is_ok_and(|flags| flags & flag != 0)
            {
                return true;
            }
            if local() >= deadline {
                return false;
            }
            Timer::after(Duration::from_micros(POLL_US)).await;
        }
    }

    /// Puts the packet in the radio's FIFO, ready to send on one mode change.
    async fn load(&mut self, packet: &[u8]) -> Result<(), ()> {
        self.receiving = false;
        let lora = &mut self.lora;
        lora.set_device_mode(DeviceMode::STDBY)
            .await
            .map_err(|_| ())?;
        lora.map_dio0::<TxDone>().await.map_err(|_| ())?;
        lora.clear_all_interrupts().await.map_err(|_| ())?;
        lora.write(FIFO_TX_BASE_ADDR, FIFO_TX_BASE_ADDR_VALUE)
            .await
            .map_err(|_| ())?;
        lora.write(FIFO_ADDR_PTR, FIFO_TX_BASE_ADDR_VALUE)
            .await
            .map_err(|_| ())?;
        lora.write_fifo(packet).await.map_err(|_| ())?;
        lora.set_payload_length(packet.len() as u8)
            .await
            .map_err(|_| ())
    }
}

impl Radio for BoardRadio {
    async fn tune(&mut self, frequency: u32, sync_word: u8, power: u8) -> bool {
        let lora = &mut self.lora;
        let _ = lora.set_device_mode(DeviceMode::STDBY).await;
        let tuned = lora.set_frequency(frequency).await.is_ok()
            && lora.write(SYNC_WORD_REGISTER, sync_word).await.is_ok();
        let powered = match TxConfig::new(OCP::new(true, 120), power, PowerRamp::Us40, false) {
            Ok(config) => lora.configure_tx(config).await.is_ok(),
            Err(_) => false,
        };
        self.idle_receive().await;
        tuned && powered
    }

    /// Puts the radio in standby with its antenna on the receive path and `DIO0` on RxDone,
    /// whatever was interrupted.
    async fn idle_receive(&mut self) {
        self.receiving = false;
        let _ = self.lora.set_device_mode(DeviceMode::STDBY).await;
        let _ = self.lora.clear_all_interrupts().await;
        let _ = self.lora.map_dio0::<RxDone>().await;
        let _ = self.path.receive().await;
    }

    async fn standby(&mut self) {
        self.receiving = false;
        let _ = self.lora.set_device_mode(DeviceMode::STDBY).await;
    }

    async fn sleep(&mut self) {
        self.receiving = false;
        let _ = self.lora.set_device_mode(DeviceMode::SLEEP).await;
    }

    async fn start_receiving(&mut self) -> bool {
        if !self.receiving {
            self.receiving = self.lora.rx(None).await.is_ok();
        }
        self.receiving
    }

    /// Reads the modem's status over SPI. The modem sees a preamble a few symbols in, so a
    /// packet that started just before goes unseen.
    async fn is_clear(&mut self) -> bool {
        if self.dio0_follows && self.dio0.is_high() {
            return false;
        }
        match self.lora.read(IRQ_FLAGS).await {
            Ok(flags) if flags & (IRQ_RX_DONE | IRQ_FLAGS_VALID_HEADER_MASK) == 0 => {}
            _ => return false,
        }
        self.lora
            .read(MODEM_STAT)
            .await
            .is_ok_and(|status| status & MODEM_BUSY == 0)
    }

    /// Waits until local time `deadline` for a packet to arrive. Returns whether one did.
    async fn wait_received(&mut self, deadline: i64) -> bool {
        self.wait_for(IRQ_RX_DONE, deadline).await
    }

    /// How late, on average, a packet's end is seen after it: half a poll where the flags are
    /// polled.
    fn seen_late_us(&self) -> i64 {
        if self.dio0_follows {
            0
        } else {
            POLL_US as i64 / 2
        }
    }

    /// Reads the packet `DIO0` reported, and when its RxDone was seen.
    async fn read_packet(&mut self) -> Option<(Received, i64)> {
        let done = local();
        let packet = self.lora.rx_packet().await;
        let flags = self.lora.read(IRQ_FLAGS).await.ok();
        let _ = self.lora.clear_all_interrupts().await;
        match packet {
            #[cfg(feature = "pair-inject")]
            Ok(packet) if super::inject::is_deaf() => {
                defmt::info!("[RADIO] deaf to len={}", packet.length);
                None
            }
            Ok(packet) => Some((
                Received {
                    payload: packet.payload,
                    length: packet.length,
                    rssi: packet.rssi,
                    snr: packet.snr,
                },
                done,
            )),
            Err(error) => {
                warn!(
                    "[MESH] receive failed: {} flags={}",
                    defmt::Debug2Format(&error),
                    flags
                );
                // The driver finds no RxDone, so DIO0 rose for nothing.
                if matches!(error, Sx127xError::PacketNotReady) && self.dio0_follows {
                    warn!(
                        "[MESH] DIO0 is high with no flag raised; polling the radio's flags from here"
                    );
                    self.dio0_follows = false;
                }
                None
            }
        }
    }

    async fn transmit(&mut self, packet: &[u8]) -> Option<bool> {
        if self.load(packet).await.is_err() {
            warn!("[MESH] loading a {}-byte packet failed", packet.len());
            self.idle_receive().await;
            return None;
        }
        let _ = self.path.transmit().await;
        let started = local();
        let _ = self.lora.set_device_mode(DeviceMode::TX).await;
        let done = self.wait_for(IRQ_TX_DONE, started + SEND_TIMEOUT_US).await;
        let flags = self.lora.read(IRQ_FLAGS).await.ok();
        if !done {
            warn!("[MESH] TxDone not seen flags={}", flags);
        }
        if self.dio0_follows && !done && flags.is_some_and(|flags| flags & IRQ_TX_DONE != 0) {
            warn!("[MESH] DIO0 did not rise for TxDone; polling the radio's flags from here");
            self.dio0_follows = false;
        }
        self.idle_receive().await;
        Some(done)
    }
}
