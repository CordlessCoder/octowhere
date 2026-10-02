//! Runs the location mesh on the radio: sends this node's packet in its slot, listens to the other
//! slots, and keeps the timebase the slots are placed on. `octowhere_mesh` holds the protocol and
//! `context/LORA-PROTOCOL.md` the design.

use core::cell::Cell;

use defmt::{info, warn};
use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::{Mutex as BlockingMutex, raw::CriticalSectionRawMutex};
use embassy_time::{Instant, Timer};
use esp_hal::gpio::Input;
use lc76g::FixQuality;
use octowhere_mesh::{
    clock::{Clock, Taken},
    packet::{Builder, Entry, Hdop, Header, MAX_PACKET, Plain, Quality, Record, Source, Timebase},
    schedule::{
        GUARD_US, ROUND_US, airtime_us, base_of, named_slot, next_any_slot, next_slot, round_at,
    },
    seal::{self, Key, SIV_LEN},
    table::{Merge, Table},
};
use sx127xlora::{
    driver::Sx127xError,
    registers::{
        FIFO_ADDR_PTR, FIFO_TX_BASE_ADDR, FIFO_TX_BASE_ADDR_VALUE, IRQ_FLAGS,
        SYNC_WORD as SYNC_WORD_REGISTER,
    },
    types::{DeviceMode, OCP, PowerRamp, RxDone, TxConfig, TxDone},
};

use super::{GPS_TIME, LoraPath, SensorLora};

/// The group key until pairing gives the node one. Anyone with the source can read and forge every
/// packet sealed under it.
const DEVELOPMENT_KEY: Key = Key::new(*b"octowhere development group key!");
/// Band O's lower 125 kHz channel.
const FREQUENCY_HZ: u32 = 869_462_500;
/// Off the SX127x's reset value `0x12`, which another network on this channel uses, and off
/// LoRaWAN's `0x34` and Meshtastic's `0x2B`. Neither nibble is zero.
const SYNC_WORD: u8 = 0x6C;
/// How long before its slot the node loads its packet and switches the antenna to transmit.
const PREPARE_US: i64 = 30_000;
/// How long after a packet ends `DIO0`'s RxDone is seen, plus how long after its slot's start a
/// sender's transmission begins: half how late a root hears the nodes timing from it, on two
/// boards whose `DIO0` follows the radio (2026-10-01). Polling the flags adds half a poll.
const ARRIVAL_LATENCY_US: i64 = 1_050;
/// The longest a transmission can take, with margin for `DIO0`.
const SEND_TIMEOUT_US: i64 = 500_000;
/// Positions a packet carries at most: as many as fit beside the neighbours record.
const MAX_ENTRIES: usize = 24;
/// How often the radio's flags are read where `DIO0` does not follow them.
const POLL_US: u64 = 1_000;
const IRQ_TX_DONE: u8 = 0x08;
const IRQ_RX_DONE: u8 = 0x40;

/// The latest fix and the UTC second it was made in, from `gnss_task`.
pub static FIX: BlockingMutex<CriticalSectionRawMutex, Cell<Option<Fix>>> =
    BlockingMutex::new(Cell::new(None));
/// The RTC's UTC seconds and when they were read, from `sensor_task`, while its oscillator has
/// not stopped.
pub static RTC_TIME: BlockingMutex<CriticalSectionRawMutex, Cell<Option<(i64, Instant)>>> =
    BlockingMutex::new(Cell::new(None));

#[derive(Clone, Copy)]
pub struct Fix {
    /// Degrees × 10⁷.
    pub latitude: i32,
    pub longitude: i32,
    /// UTC seconds.
    pub stamp: u32,
    pub quality: FixQuality,
    pub hdop_milli: Option<u32>,
}

/// The node's id until pairing gives it one: the low five bits of its MAC. Two boards can share
/// one.
pub fn provisional_id() -> u8 {
    esp_hal::efuse::base_mac_address().as_bytes()[5] & 0x1f
}

fn local() -> i64 {
    Instant::now().as_micros() as i64
}

fn until(local: i64) -> Timer {
    Timer::at(Instant::from_micros(local.max(0) as u64))
}

pub struct Mesh {
    lora: SensorLora,
    dio0: Input<'static>,
    /// `DIO0` rose for the last flag it was mapped to. Where it did not, the flags are polled.
    dio0_follows: bool,
    path: LoraPath,
    clock: Clock,
    table: Table,
    own: u8,
    /// The timebase time the next own slot is looked for from, past the last one decided.
    after: i64,
}

impl Mesh {
    pub async fn new(mut lora: SensorLora, dio0: Input<'static>, path: LoraPath) -> Self {
        let own = provisional_id();
        let tuned = lora.set_frequency(FREQUENCY_HZ).await.is_ok()
            && lora.write(SYNC_WORD_REGISTER, SYNC_WORD).await.is_ok();
        // +17 dBm on PA_BOOST, which the module's antenna is most likely on.
        let powered = match TxConfig::new(OCP::new(true, 120), 17, PowerRamp::Us40, false) {
            Ok(config) => lora.configure_tx(config).await.is_ok(),
            Err(_) => false,
        };
        info!(
            "[MESH] id={} tuned={} powered={} listening for {}s first",
            own,
            tuned,
            powered,
            octowhere_mesh::clock::SWEEP_US / 1_000_000
        );
        let now = local();
        Self {
            lora,
            dio0,
            dio0_follows: true,
            path,
            clock: Clock::new(own, now),
            table: Table::new(own),
            own,
            after: i64::MIN,
        }
    }

    pub async fn run(mut self) -> ! {
        let _ = self.lora.map_dio0::<RxDone>().await;
        let _ = self.path.receive().await;
        let mut shown = None;
        loop {
            let now = local();
            self.take_readings();
            self.clock.tick(now, rtc_now(now));
            let timebase = self.clock.at(now).map(|(_, timebase)| timebase);
            if timebase != shown {
                log_timebase(timebase, self.clock.is_sweeping());
                shown = timebase;
            }
            let Some((time, _)) = self.clock.at(now) else {
                let end = self.clock.sweep_ends().unwrap_or(now + ROUND_US);
                self.listen(end).await;
                continue;
            };
            let (round, start) = next_slot((time + PREPARE_US).max(self.after), self.own);
            let send_at = start + (now - time);
            if self.listen(send_at - PREPARE_US).await {
                self.after = i64::MIN;
                continue;
            }
            self.after = start + 1;
            let sending = self.table.wants_to_send(round);
            info!("[MESH] round={} sending={}", round, sending);
            if sending {
                self.send(round, start, timebase, send_at).await;
            }
        }
    }

    fn take_readings(&mut self) {
        let gps = GPS_TIME.lock(Cell::get);
        if let Some(gps) = gps {
            self.clock.gps(gps.offset, gps.updated.as_micros() as i64);
            if let Some(fix) = FIX.lock(Cell::get) {
                self.table.set_own(Entry {
                    id: self.own,
                    latitude: fix.latitude,
                    longitude: fix.longitude,
                    stamp: fix.stamp,
                    quality: match fix.quality {
                        FixQuality::Autonomous => Quality::Autonomous,
                        FixQuality::Differential
                        | FixQuality::Pps
                        | FixQuality::Rtk
                        | FixQuality::FloatRtk => Quality::Differential,
                        _ => Quality::Estimated,
                    },
                    hdop: Hdop::from_milli(fix.hdop_milli.unwrap_or(u32::MAX)),
                });
            }
        }
        if let Some((time, _)) = self.clock.at(local()) {
            self.table.expire((time / 1_000_000) as u32);
        }
    }

    /// Listens until local time `end`: throughout while sweeping or without a timebase, and
    /// otherwise in a window round each other id's slot. Returns whether a packet moved the node
    /// to another timebase, which moves every slot.
    async fn listen(&mut self, end: i64) -> bool {
        loop {
            let now = local();
            if now >= end {
                return false;
            }
            let (open, close) = match self.clock.at(now) {
                Some((time, _)) if !self.clock.is_sweeping() => {
                    let offset = now - time;
                    let mut slot = next_any_slot(time - GUARD_US - airtime_us(MAX_PACKET) + 1);
                    if slot.0 == self.own {
                        slot = next_any_slot(slot.1 + 1);
                    }
                    let open = slot.1 - GUARD_US + offset;
                    let close = slot.1 + GUARD_US + airtime_us(MAX_PACKET) + offset;
                    (open.max(now), close.min(end))
                }
                _ => (now, end),
            };
            if open >= end {
                let _ = self.lora.set_device_mode(DeviceMode::STDBY).await;
                until(end).await;
                return false;
            }
            if open > now {
                let _ = self.lora.set_device_mode(DeviceMode::STDBY).await;
                until(open).await;
            }
            if self.lora.rx(None).await.is_err() {
                warn!("[MESH] receive start failed");
            }
            while self.wait_for(IRQ_RX_DONE, close).await {
                if self.receive().await {
                    let _ = self.lora.set_device_mode(DeviceMode::STDBY).await;
                    return true;
                }
            }
            let _ = self.lora.set_device_mode(DeviceMode::STDBY).await;
        }
    }

    /// Takes the packet `DIO0` reported. Returns whether it moved the node to its timebase.
    async fn receive(&mut self) -> bool {
        let done = local();
        let packet = self.lora.rx_packet().await;
        let flags = self.lora.read(IRQ_FLAGS).await.ok();
        let _ = self.lora.clear_all_interrupts().await;
        let packet = match packet {
            Ok(packet) => packet,
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
                return false;
            }
        };
        let len = packet.length;
        let mut bytes = packet.payload;
        let Ok(plain) = seal::open(&DEVELOPMENT_KEY, &mut bytes[..len]) else {
            info!("[MESH] not ours len={} rssi={}", len, packet.rssi);
            return false;
        };
        let Ok(plain) = Plain::parse(plain) else {
            warn!("[MESH] malformed len={}", len);
            return false;
        };
        let header = plain.header;
        let polled = if self.dio0_follows {
            0
        } else {
            POLL_US as i64 / 2
        };
        let start = done - airtime_us(len) - ARRIVAL_LATENCY_US - polled;
        let arrival = self.clock.arrival(&header, start, done);
        self.table.heard(
            header.sender,
            round_at(named_slot(header.base, header.sender)),
        );
        let now = self
            .clock
            .at(done)
            .map(|(time, _)| (time / 1_000_000) as u32);
        let (mut entries, mut news, mut neighbours) = (0, 0, 0);
        for record in plain.records() {
            match record {
                Record::Positions(positions) => {
                    for entry in positions {
                        entries += 1;
                        if matches!(self.table.merge(entry, now), Merge::New | Merge::Newer) {
                            news += 1;
                        }
                    }
                }
                Record::Neighbours(set) => neighbours = set,
                Record::Member(..) | Record::Other(..) => {}
            }
        }
        info!(
            "[MESH] heard id={} timebase={} hops={} taken={} late_us={} len={} rssi={} snr={} entries={} new={} neighbours={=u32:#010x}",
            header.sender,
            header.timebase.source,
            header.timebase.hops,
            arrival.taken,
            arrival.late_us,
            len,
            packet.rssi,
            packet.snr,
            entries,
            news,
            neighbours,
        );
        arrival.taken == Taken::Adopted
    }

    /// Sends this node's packet in its slot in `round`, which starts at timebase time `start` and
    /// local time `send_at`.
    async fn send(&mut self, round: i64, start: i64, timebase: Option<Timebase>, send_at: i64) {
        let Some(timebase) = timebase else { return };
        let base = base_of(start);
        let header = Header {
            sender: self.own,
            timebase,
            base,
            phase: 0,
        };
        let mut packet = [0u8; MAX_PACKET];
        let mut builder = Builder::new(&mut packet[SIV_LEN..], &header);
        let neighbours = self.table.neighbours(round);
        let _ = builder.neighbours(neighbours);
        let mut entries = [Entry {
            id: 0,
            latitude: 0,
            longitude: 0,
            stamp: 0,
            quality: Quality::Reserved,
            hdop: Hdop::from_milli(0),
        }; MAX_ENTRIES];
        let room = builder.room_for_entries().min(MAX_ENTRIES);
        let n = self.table.digest(base, &mut entries[..room]);
        if n > 0 && builder.positions(&entries[..n]).is_err() {
            warn!("[MESH] positions did not fit");
        }
        let plain_len = builder.finish();
        let len = seal::seal(&DEVELOPMENT_KEY, &mut packet, plain_len);

        if self.load(&packet[..len]).await.is_err() {
            warn!("[MESH] loading the packet failed");
            return;
        }
        let switched = self.path.transmit().await.is_ok();
        until(send_at).await;
        let late = local() - send_at;
        let started = self.lora.set_device_mode(DeviceMode::TX).await.is_ok();
        let done = self.wait_for(IRQ_TX_DONE, send_at + SEND_TIMEOUT_US).await;
        let flags = self.lora.read(IRQ_FLAGS).await.ok();
        if self.dio0_follows && !done && flags.is_some_and(|flags| flags & IRQ_TX_DONE != 0) {
            warn!("[MESH] DIO0 did not rise for TxDone; polling the radio's flags from here");
            self.dio0_follows = false;
        }
        let _ = self.lora.clear_all_interrupts().await;
        let _ = self.path.receive().await;
        let _ = self.lora.map_dio0::<RxDone>().await;
        self.table.sent(&entries[..n]);
        info!(
            "[MESH] sent round={} len={} entries={} neighbours={=u32:#010x} late_us={} switched={} started={} done={} flags={}",
            round, len, n, neighbours, late, switched, started, done, flags
        );
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
            Timer::after(embassy_time::Duration::from_micros(POLL_US)).await;
        }
    }

    /// Puts the packet in the radio's FIFO, ready to send on one mode change.
    async fn load(&mut self, packet: &[u8]) -> Result<(), ()> {
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

/// The RTC's UTC in microseconds at local time `now`, to the RTC's whole second.
fn rtc_now(now: i64) -> Option<i64> {
    RTC_TIME
        .lock(Cell::get)
        .map(|(seconds, read)| seconds * 1_000_000 + now - read.as_micros() as i64)
}

fn log_timebase(timebase: Option<Timebase>, sweeping: bool) {
    match timebase {
        None => info!("[MESH] no timebase, sweeping={}", sweeping),
        Some(Timebase {
            source: Source::Gps,
            hops,
        }) => info!("[MESH] timebase GPS hops={}", hops),
        Some(Timebase {
            source: Source::Node(root),
            hops,
        }) => info!("[MESH] timebase root={} hops={}", root, hops),
    }
}
