use std::{cell::RefCell, rc::Rc};

use embedded_hal::i2c::{Operation, SevenBitAddress};
use embedded_hal_async::{delay::DelayNs, i2c::I2c};
use futures::executor::block_on;
use lc76g::{Lc76g, LowPowerMode, NmeaParser, NmeaUpdate, PairAck, PairAckStatus};

#[derive(Default)]
struct MockState {
    writes: Vec<(u8, Vec<u8>)>,
    reads: Vec<Vec<u8>>,
}

struct MockI2c {
    state: Rc<RefCell<MockState>>,
}

impl embedded_hal::i2c::ErrorType for MockI2c {
    type Error = embedded_hal::i2c::ErrorKind;
}

impl I2c<SevenBitAddress> for MockI2c {
    async fn transaction(
        &mut self,
        address: u8,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        for operation in operations {
            match operation {
                Operation::Write(data) => {
                    self.state.borrow_mut().writes.push((address, data.to_vec()));
                }
                Operation::Read(data) => {
                    let value = self.state.borrow_mut().reads.remove(0);
                    data.copy_from_slice(&value);
                }
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct MockDelay {
    calls: usize,
}

impl DelayNs for MockDelay {
    async fn delay_ns(&mut self, ns: u32) {
        assert_eq!(ns, 10_000_000);
        self.calls += 1;
    }
}

#[test]
fn parser_decodes_rmc_position() {
    let mut parser = NmeaParser::new();
    let mut update = None;
    for byte in b"$GPRMC,125504.049,A,5542.2389,N,03741.6063,E,0.06,25.82,200906,,,A*56\r\n" {
        update = parser.push(*byte).unwrap().or(update);
    }

    assert_eq!(update, Some(NmeaUpdate::Rmc));
    let fix = parser.state().fix.unwrap();
    assert!((557_000_000..558_000_000).contains(&fix.latitude.get()));
    assert!((376_000_000..377_000_000).contains(&fix.longitude.get()));
}

#[test]
fn parser_rejects_a_bad_checksum_and_recovers() {
    let mut parser = NmeaParser::new();
    let mut error = None;
    for byte in b"$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*00\r\n" {
        if let Err(value) = parser.push(*byte) {
            error = Some(value);
        }
    }
    assert_eq!(error, Some("Checksum error!"));

    for byte in b"$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47\r\n" {
        parser.push(*byte).unwrap();
    }
    assert_eq!(
        parser.state().fix.unwrap().satellites.map(|value| value.get()),
        Some(8)
    );
}

#[test]
fn parser_reports_pair_acknowledgements() {
    let mut parser = NmeaParser::new();
    let mut update = None;
    for byte in b"$PAIR001,732,0*3D\r\n" {
        update = parser.push(*byte).unwrap().or(update);
    }

    assert_eq!(
        update,
        Some(NmeaUpdate::PairAck(PairAck {
            command: 732,
            status: PairAckStatus::Accepted,
        }))
    );
}

#[test]
fn read_nmea_chunk_uses_the_length_and_data_commands() {
    let state = Rc::new(RefCell::new(MockState {
        writes: Vec::new(),
        reads: vec![vec![3, 0, 0, 0], b"abc".to_vec()],
    }));
    let i2c = MockI2c {
        state: state.clone(),
    };
    let mut gnss = Lc76g::new(i2c, MockDelay::default());
    let mut buffer = [0; 8];

    let data = block_on(gnss.read_nmea_chunk(&mut buffer)).unwrap();

    assert_eq!(data, b"abc");
    assert_eq!(state.borrow().writes[0], (0x50, vec![0x08, 0, 0x51, 0xAA, 4, 0, 0, 0]));
    assert_eq!(state.borrow().writes[1], (0x50, vec![0, 0x20, 0x51, 0xAA, 3, 0, 0, 0]));
}

#[test]
fn adaptive_low_power_mode_sends_documented_prerequisites() {
    let state = Rc::new(RefCell::new(MockState {
        writes: Vec::new(),
        reads: vec![vec![64, 0, 0, 0]; 3],
    }));
    let i2c = MockI2c {
        state: state.clone(),
    };
    let mut gnss = Lc76g::new(i2c, MockDelay::default());

    block_on(gnss.set_low_power_mode(LowPowerMode::Adaptive)).unwrap();

    let writes = &state.borrow().writes;
    let command_data: Vec<&[u8]> = writes
        .iter()
        .filter(|(address, _)| *address == 0x58)
        .map(|(_, data)| data.as_slice())
        .collect();
    assert_eq!(
        command_data,
        vec![
            b"$PAIR080,0*2E\r\n".as_slice(),
            b"$PAIR050,1000*12\r\n".as_slice(),
            b"$PAIR732,1*21\r\n".as_slice(),
        ]
    );
}
