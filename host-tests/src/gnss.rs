use lc76g::{NmeaError, NmeaParser};

#[test]
fn parser_returns_a_valid_nmea_sentence_without_line_ending() {
    let mut parser = NmeaParser::<128>::new();
    let mut found = false;
    for byte in b"$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47\r\n" {
        if let Some(sentence) = parser.push(*byte).unwrap() {
            assert_eq!(
                sentence,
                &b"$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47"[..]
            );
            found = true;
        }
    }
    assert!(found);
}

#[test]
fn parser_rejects_a_bad_checksum_and_recovers() {
    let mut parser = NmeaParser::<128>::new();
    for byte in b"$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*00\r\n" {
        if *byte == b'\n' {
            assert_eq!(parser.push(*byte), Err(NmeaError::InvalidChecksum));
        } else {
            parser.push(*byte).unwrap();
        }
    }
    for byte in b"$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47\r\n" {
        if *byte == b'\n' {
            assert!(parser.push(*byte).unwrap().is_some());
        } else {
            parser.push(*byte).unwrap();
        }
    }
}
