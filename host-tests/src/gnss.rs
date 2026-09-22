use std::{cell::RefCell, rc::Rc};

use embedded_hal::i2c::{Error, ErrorKind, Operation, SevenBitAddress};
use embedded_hal_async::{delay::DelayNs, i2c::I2c};
use futures::executor::block_on;
use lc76g::{
    AicMode, DebugLogOutput, ElevationMaskDegrees, FixIntervalMs, GnssFixType, GnssSearchMode,
    Lc76g, LowPowerMode, MinimumSnrDb, NavigationMode, NmeaOutputRate, NmeaParser, NmeaSentence,
    NmeaUpdate, PairAck, PairAckStatus, PairCommandBuilder, StaticNavigationThreshold,
};

#[derive(Default)]
struct MockState {
    writes: Vec<(u8, Vec<u8>)>,
    reads: Vec<Vec<u8>>,
    read_failures: usize,
}

#[derive(Debug)]
struct MockError;

impl Error for MockError {
    fn kind(&self) -> ErrorKind {
        ErrorKind::Other
    }
}

struct MockI2c {
    state: Rc<RefCell<MockState>>,
}

impl embedded_hal::i2c::ErrorType for MockI2c {
    type Error = MockError;
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
                    if self.state.borrow().read_failures != 0 {
                        self.state.borrow_mut().read_failures -= 1;
                        return Err(MockError);
                    }
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

    assert!(matches!(update, Some(NmeaUpdate::Sentence(_))));
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
fn parser_returns_all_standard_sentences_supported_by_dependency() {
    let mut parser = NmeaParser::new();

    for sentence in [
        b"$GPGLL,4916.45,N,12311.12,W,225444,A*31\r\n".as_slice(),
        b"$GPVTG,089.0,T,,,15.2,N,,,A*12\r\n".as_slice(),
        b"$GNZDA,181604.456,12,09,2018,-01,15*6C\r\n".as_slice(),
    ] {
        let mut update = None;
        for &byte in sentence {
            update = parser.push(byte).unwrap().or(update);
        }
        assert!(matches!(update, Some(NmeaUpdate::Sentence(_))));
    }
}

#[test]
fn parser_reports_nmea_output_rate_queries() {
    let mut parser = NmeaParser::new();
    let mut update = None;
    for &byte in b"$PAIR063,2,3*3E\r\n" {
        update = parser.push(byte).unwrap().or(update);
    }

    assert_eq!(
        update,
        Some(NmeaUpdate::NmeaOutputRate {
            sentence: NmeaSentence::Gsa,
            rate: NmeaOutputRate::every(3).unwrap(),
        })
    );
}

#[test]
fn pair_command_builder_encodes_protocol_fields() {
    let mut builder = PairCommandBuilder::new(66).unwrap();
    for value in [1, 1, 1, 1, 0, 0] {
        builder.field_u32(value).unwrap();
    }
    let command = builder.finish().unwrap();

    assert_eq!(
        command.as_bytes(),
        b"$PAIR066,1,1,1,1,0,0*3A\r\n"
    );

    let mut builder = PairCommandBuilder::new(62).unwrap();
    builder.field_i32(-1).unwrap();
    assert_eq!(builder.finish().unwrap().as_bytes(), b"$PAIR062,-1*0E\r\n");
}

#[test]
fn parser_preserves_untyped_pair_responses() {
    let mut parser = NmeaParser::new();
    let mut update = None;
    for &byte in b"$PAIR067,1,1,1,1,1,0*3A\r\n" {
        update = parser.push(byte).unwrap().or(update);
    }

    let Some(NmeaUpdate::Pair(message)) = update else {
        panic!("expected a generic PAIR response");
    };
    assert_eq!(message.command(), 67);
    assert_eq!(message.fields(), b"1,1,1,1,1,0");
}

#[test]
fn parser_preserves_valid_unsupported_nmea_frames() {
    let mut parser = NmeaParser::new();
    let mut update = None;
    let sentence = b"$GARLM,9A22BE29630F010,125713.000,F,5402*3B\r\n";
    for &byte in sentence {
        update = parser.push(byte).unwrap().or(update);
    }

    let Some(NmeaUpdate::Raw(raw)) = update else {
        panic!("expected an untyped NMEA frame");
    };
    assert_eq!(raw.as_bytes(), sentence);
}

#[test]
fn parser_reports_satellite_acquisition_progress() {
    let mut parser = NmeaParser::new();
    for byte in b"$GNGSA,A,3,21,5,29,25,12,10,26,2,,,,,1.2,0.7,1.0*27\r\n" {
        parser.push(*byte).unwrap();
    }
    for byte in b"$GPGSV,8,1,25,21,44,141,47,15,14,049,44,6,31,255,46,3,25,280,44*75\r\n" {
        parser.push(*byte).unwrap();
    }

    let signal = parser.state().signal;
    assert_eq!(signal.fix_type, GnssFixType::Fix3D);
    assert_eq!(signal.satellites_used.get(), 8);
    assert_eq!(signal.satellites_in_view.get(), 25);
    assert_eq!(signal.satellites_with_signal.get(), 4);
    assert_eq!(signal.strongest_snr.map(|value| value.get()), Some(47));
    assert_eq!(signal.pdop.map(|value| value.get()), Some(1_200));
    assert_eq!(signal.hdop.map(|value| value.get()), Some(700));
    assert_eq!(signal.vdop.map(|value| value.get()), Some(1_000));
}

#[test]
fn read_nmea_chunk_uses_the_length_and_data_commands() {
    let state = Rc::new(RefCell::new(MockState {
        writes: Vec::new(),
        reads: vec![vec![3, 0, 0, 0], b"abc".to_vec()],
        read_failures: 0,
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
fn read_nmea_reissues_the_command_after_a_read_failure() {
    let state = Rc::new(RefCell::new(MockState {
        writes: Vec::new(),
        reads: vec![vec![3, 0, 0, 0], b"abc".to_vec()],
        read_failures: 1,
    }));
    let i2c = MockI2c {
        state: state.clone(),
    };
    let mut gnss = Lc76g::new(i2c, MockDelay::default());
    let mut buffer = [0; 8];

    assert_eq!(block_on(gnss.read_nmea_chunk(&mut buffer)).unwrap(), b"abc");

    let writes = &state.borrow().writes;
    assert_eq!(writes.len(), 3);
    assert_eq!(writes[0].0, 0x50);
    assert_eq!(writes[1], writes[0]);
    assert_eq!(writes[2], (0x50, vec![0, 0x20, 0x51, 0xAA, 3, 0, 0, 0]));
}

#[test]
fn adaptive_low_power_mode_sends_documented_prerequisites() {
    let state = Rc::new(RefCell::new(MockState {
        writes: Vec::new(),
        reads: vec![vec![64, 0, 0, 0]; 3],
        read_failures: 0,
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

#[test]
fn typed_receiver_configuration_uses_documented_wire_commands() {
    let state = Rc::new(RefCell::new(MockState {
        writes: Vec::new(),
        reads: vec![vec![64, 0, 0, 0]; 16],
        read_failures: 0,
    }));
    let i2c = MockI2c {
        state: state.clone(),
    };
    let mut gnss = Lc76g::new(i2c, MockDelay::default());

    block_on(gnss.set_fix_interval(FixIntervalMs::new(1_000).unwrap())).unwrap();
    block_on(gnss.query_fix_interval()).unwrap();
    block_on(gnss.set_minimum_snr(MinimumSnrDb::new(15).unwrap())).unwrap();
    block_on(gnss.query_minimum_snr()).unwrap();
    block_on(gnss.set_gnss_search_mode(GnssSearchMode::new(
        true, false, true, true, false,
    )))
    .unwrap();
    block_on(gnss.query_gnss_search_mode()).unwrap();
    block_on(gnss.set_static_navigation_threshold(
        StaticNavigationThreshold::new(4).unwrap(),
    ))
    .unwrap();
    block_on(gnss.query_static_navigation_threshold()).unwrap();
    block_on(gnss.set_elevation_mask(ElevationMaskDegrees::new(5).unwrap())).unwrap();
    block_on(gnss.query_elevation_mask()).unwrap();
    block_on(gnss.set_aic_mode(AicMode::Enabled)).unwrap();
    block_on(gnss.query_aic_mode()).unwrap();
    block_on(gnss.set_navigation_mode(NavigationMode::Fitness))
        .unwrap();
    block_on(gnss.query_navigation_mode()).unwrap();
    block_on(gnss.set_debug_log_output(DebugLogOutput::Full))
        .unwrap();
    block_on(gnss.query_debug_log_output()).unwrap();

    let command_data: Vec<Vec<u8>> = state
        .borrow()
        .writes
        .iter()
        .filter(|(address, _)| *address == 0x58)
        .map(|(_, data)| data.clone())
        .collect();
    assert_eq!(
        command_data,
        vec![
            b"$PAIR050,1000*12\r\n".to_vec(),
            b"$PAIR051*3E\r\n".to_vec(),
            b"$PAIR058,15*1F\r\n".to_vec(),
            b"$PAIR059*36\r\n".to_vec(),
            b"$PAIR066,1,0,1,1,0,0*3B\r\n".to_vec(),
            b"$PAIR067*3B\r\n".to_vec(),
            b"$PAIR070,4*25\r\n".to_vec(),
            b"$PAIR071*3C\r\n".to_vec(),
            b"$PAIR072,5*26\r\n".to_vec(),
            b"$PAIR073*3E\r\n".to_vec(),
            b"$PAIR074,1*24\r\n".to_vec(),
            b"$PAIR075*38\r\n".to_vec(),
            b"$PAIR080,1*2F\r\n".to_vec(),
            b"$PAIR081*33\r\n".to_vec(),
            b"$PAIR086,1*29\r\n".to_vec(),
            b"$PAIR087*35\r\n".to_vec(),
        ]
    );
}

#[test]
fn typed_receiver_configuration_rejects_out_of_range_values() {
    assert!(FixIntervalMs::new(99).is_none());
    assert!(FixIntervalMs::new(1_001).is_none());
    assert!(MinimumSnrDb::new(8).is_none());
    assert!(MinimumSnrDb::new(38).is_none());
    assert!(StaticNavigationThreshold::new(21).is_none());
    assert!(ElevationMaskDegrees::new(-91).is_none());
    assert!(ElevationMaskDegrees::new(91).is_none());
}

#[test]
fn nmea_output_rate_configures_any_sentence_type() {
    let state = Rc::new(RefCell::new(MockState {
        writes: Vec::new(),
        reads: vec![vec![64, 0, 0, 0]; 2],
        read_failures: 0,
    }));
    let i2c = MockI2c {
        state: state.clone(),
    };
    let mut gnss = Lc76g::new(i2c, MockDelay::default());

    block_on(gnss.set_nmea_output_rate(
        NmeaSentence::Gsa,
        NmeaOutputRate::EVERY_FIX,
    ))
    .unwrap();
    block_on(gnss.set_nmea_output_rate(
        NmeaSentence::Gsv,
        NmeaOutputRate::every(2).unwrap(),
    ))
    .unwrap();

    let writes = &state.borrow().writes;
    let command_data: Vec<&[u8]> = writes
        .iter()
        .filter(|(address, _)| *address == 0x58)
        .map(|(_, data)| data.as_slice())
        .collect();
    assert_eq!(
        command_data,
        vec![
            b"$PAIR062,2,1*3D\r\n".as_slice(),
            b"$PAIR062,3,2*3F\r\n".as_slice(),
        ]
    );
}

#[test]
fn nmea_output_rate_queries_use_documented_commands() {
    let state = Rc::new(RefCell::new(MockState {
        writes: Vec::new(),
        reads: vec![vec![64, 0, 0, 0]; 3],
        read_failures: 0,
    }));
    let i2c = MockI2c {
        state: state.clone(),
    };
    let mut gnss = Lc76g::new(i2c, MockDelay::default());

    block_on(gnss.query_nmea_output_rate(NmeaSentence::Gsa)).unwrap();
    block_on(gnss.query_all_nmea_output_rates()).unwrap();
    block_on(gnss.reset_nmea_output_rates()).unwrap();

    let writes = &state.borrow().writes;
    let command_data: Vec<&[u8]> = writes
        .iter()
        .filter(|(address, _)| *address == 0x58)
        .map(|(_, data)| data.as_slice())
        .collect();
    assert_eq!(
        command_data,
        vec![
            b"$PAIR063,2*21\r\n".as_slice(),
            b"$PAIR063,-1*0F\r\n".as_slice(),
            b"$PAIR062,-1*0E\r\n".as_slice(),
        ]
    );
}
