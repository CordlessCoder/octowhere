//! The mesh's radio: the SX1272, its `DIO0` line, and the RF switch on the I/O expander.

use defmt::warn;
use embassy_futures::select::{Either, select};
use embassy_time::{Duration, Timer};
use esp_hal::gpio::Input;
use sx127xlora::{
    driver::Sx127xError,
    types::{DeviceMode, IRQ, OCP, PowerRamp, RxDone, TxConfig, TxDone},
};

use octowhere_node::{Radio, Received, Sent};

use super::time::{local, until};
use crate::{HeldPath, LoraPath, SensorLora};

/// The longest a transmission can take, with margin for `DIO0`.
const SEND_TIMEOUT_US: i64 = 500_000;
/// How often the radio's flags are read where `DIO0` does not follow them.
const POLL_US: u64 = 1_000;

pub struct BoardRadio {
    modem: Modem,
    path: LoraPath,
}

/// The SX1272 and its `DIO0` line.
struct Modem {
    lora: SensorLora,
    dio0: Input<'static>,
    /// `DIO0` rose for the last flag it was mapped to. Where it did not, the flags are polled.
    dio0_follows: bool,
    /// In continuous receive since the last mode change. Starting it again would restart the
    /// receiver, and lose a packet under way.
    receiving: bool,
}

impl BoardRadio {
    pub fn new(lora: SensorLora, dio0: Input<'static>, path: LoraPath) -> Self {
        Self {
            modem: Modem {
                lora,
                dio0,
                dio0_follows: true,
                receiving: false,
            },
            path,
        }
    }

    /// Waits for the transmission started at local time `started` to end, and puts the radio
    /// back to receive. Returns whether TxDone was seen.
    async fn finish(&mut self, started: i64) -> bool {
        let done = self.modem.wait_sent(started).await;
        self.idle_receive().await;
        done
    }
}

impl Modem {
    /// Waits until local time `deadline` for the radio to raise `I`, the interrupt `DIO0` is
    /// mapped to. Returns whether it did.
    async fn wait_for<I: IRQ>(&mut self, deadline: i64) -> bool {
        if self.dio0_follows {
            return matches!(
                select(self.dio0.wait_for_high(), until(deadline)).await,
                Either::First(())
            );
        }
        loop {
            if self.lora.interrupt_flag::<I>().await.unwrap_or(false) {
                return true;
            }
            if local() >= deadline {
                return false;
            }
            Timer::after(Duration::from_micros(POLL_US)).await;
        }
    }

    /// Whether the channel is clear, or `Err` where the modem could not be read.
    async fn is_clear(&mut self) -> Result<bool, ()> {
        if self.dio0_follows && self.dio0.is_high() {
            return Ok(false);
        }
        self.lora.rx_busy().await.map(|busy| !busy).map_err(|_| ())
    }

    /// Waits for the transmission started at local time `started` to end. Returns whether
    /// TxDone was seen.
    async fn wait_sent(&mut self, started: i64) -> bool {
        let done = self.wait_for::<TxDone>(started + SEND_TIMEOUT_US).await;
        let flags = self.lora.irq_flags().await.ok();
        if !done {
            warn!("[MESH] TxDone not seen flags={}", flags);
        }
        if self.dio0_follows && !done && flags.is_some_and(|flags| flags.contains::<TxDone>()) {
            warn!("[MESH] DIO0 did not rise for TxDone; polling the radio's flags from here");
            self.dio0_follows = false;
        }
        done
    }

    /// Puts the packet in the radio's FIFO, ready to send on one mode change.
    async fn load(&mut self, packet: &[u8]) -> Result<(), ()> {
        self.receiving = false;
        self.lora.load_tx(packet).await.map_err(|_| ())?;
        self.lora.map_dio0::<TxDone>().await.map_err(|_| ())
    }

    /// Loads `packet`, switches `path` to transmit and starts sending. Returns the local time
    /// it started, or `Err` where nothing went.
    async fn start(&mut self, packet: &[u8], path: &mut HeldPath<'_>) -> Result<i64, ()> {
        if self.load(packet).await.is_err() {
            warn!("[MESH] loading a {}-byte packet failed", packet.len());
            return Err(());
        }
        if path.transmit().await.is_err() {
            warn!("[MESH] switching the antenna to transmit failed");
            return Err(());
        }
        let started = local();
        if self.lora.set_device_mode(DeviceMode::TX).await.is_err() {
            warn!("[MESH] starting a transmission failed");
            return Err(());
        }
        Ok(started)
    }
}

impl Radio for BoardRadio {
    async fn tune(&mut self, frequency: u32, sync_word: u8, power: u8) -> bool {
        let lora = &mut self.modem.lora;
        let _ = lora.set_device_mode(DeviceMode::STDBY).await;
        let tuned = lora.set_frequency(frequency).await.is_ok()
            && lora.set_sync_word(sync_word).await.is_ok();
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
        let modem = &mut self.modem;
        modem.receiving = false;
        let _ = modem.lora.set_device_mode(DeviceMode::STDBY).await;
        let _ = modem.lora.clear_all_interrupts().await;
        let _ = modem.lora.map_dio0::<RxDone>().await;
        let _ = self.path.receive().await;
    }

    async fn standby(&mut self) {
        self.modem.receiving = false;
        let _ = self.modem.lora.set_device_mode(DeviceMode::STDBY).await;
    }

    async fn sleep(&mut self) {
        self.modem.receiving = false;
        let _ = self.modem.lora.set_device_mode(DeviceMode::SLEEP).await;
    }

    async fn start_receiving(&mut self) -> bool {
        let modem = &mut self.modem;
        if !modem.receiving {
            modem.receiving = modem.lora.rx(None).await.is_ok();
        }
        modem.receiving
    }

    /// Waits until local time `deadline` for a packet to arrive. Returns whether one did.
    async fn wait_received(&mut self, deadline: i64) -> bool {
        self.modem.wait_for::<RxDone>(deadline).await
    }

    /// How late, on average, a packet's end is seen after it: half a poll where the flags are
    /// polled.
    fn seen_late_us(&self) -> i64 {
        if self.modem.dio0_follows {
            0
        } else {
            POLL_US as i64 / 2
        }
    }

    /// Reads the packet `DIO0` reported, and when its RxDone was seen.
    async fn read_packet(&mut self) -> Option<(Received, i64)> {
        let modem = &mut self.modem;
        let done = local();
        let packet = modem.lora.rx_packet().await;
        let flags = modem.lora.irq_flags().await.ok();
        let _ = modem.lora.clear_all_interrupts().await;
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
                if matches!(error, Sx127xError::PacketNotReady) && modem.dio0_follows {
                    warn!(
                        "[MESH] DIO0 is high with no flag raised; polling the radio's flags from here"
                    );
                    modem.dio0_follows = false;
                }
                None
            }
        }
    }

    async fn transmit(&mut self, packet: &[u8]) -> Option<bool> {
        let mut held = self.path.lock().await;
        let started = self.modem.start(packet, &mut held).await;
        drop(held);
        match started {
            Ok(started) => Some(self.finish(started).await),
            Err(()) => {
                self.idle_receive().await;
                None
            }
        }
    }

    /// Holds the I2C bus from the check until the transmission has started. The switch to
    /// transmit is an I2C write, and a GNSS read can hold the bus for about 12 ms, long enough
    /// for another node's packet to start unseen.
    async fn transmit_if_clear(&mut self, packet: &[u8]) -> Sent {
        let mut held = self.path.lock().await;
        match self.modem.is_clear().await {
            Ok(true) => {}
            Ok(false) => return Sent::Busy,
            Err(()) => {
                warn!("[MESH] reading the channel's state failed");
                return Sent::Failed;
            }
        }
        let started = self.modem.start(packet, &mut held).await;
        drop(held);
        match started {
            Ok(started) => Sent::Done {
                started,
                finished: self.finish(started).await,
            },
            Err(()) => {
                self.idle_receive().await;
                Sent::Failed
            }
        }
    }
}
