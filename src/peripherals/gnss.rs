use embassy_time::{Duration, Timer};
use embedded_hal_async::i2c::I2c;

const CONFIG_ADDRESS: u8 = 0x50;
const DATA_ADDRESS: u8 = 0x54;
const WRITE_DATA_ADDRESS: u8 = 0x58;
const CONFIG_READ_NMEA_LENGTH: u32 = 0xAA51_0008;
const CONFIG_READ_DATA: u32 = 0xAA51_2000;
const CONFIG_WRITE_NMEA: u32 = 0xAA53_1000;
const CONFIG_READ_FREE_LENGTH: u32 = 0xAA51_0004;
const NMEA_LENGTH_BYTES: usize = 4;
const INTER_COMMAND_DELAY: Duration = Duration::from_millis(10);
const MAX_I2C_RETRIES: usize = 20;

#[derive(Debug, PartialEq, Eq)]
pub enum GnssOperation {
    WriteConfig,
    ReadLength,
    ReadData,
    WriteData,
}

#[derive(Debug, PartialEq, Eq)]
pub enum GnssError<E> {
    I2c { operation: GnssOperation, error: E },
    BufferTooSmall { required: usize, available: usize },
}

#[derive(Debug, PartialEq, Eq)]
pub enum NmeaError {
    LineTooLong,
    InvalidChecksum,
}

pub struct NmeaParser<const N: usize> {
    line: [u8; N],
    len: usize,
}

impl<const N: usize> NmeaParser<N> {
    pub const fn new() -> Self {
        Self {
            line: [0; N],
            len: 0,
        }
    }

    pub fn push(&mut self, byte: u8) -> Result<Option<&[u8]>, NmeaError> {
        if byte != b'\n' {
            if self.len == N {
                self.len = 0;
                return Err(NmeaError::LineTooLong);
            }
            self.line[self.len] = byte;
            self.len += 1;
            return Ok(None);
        }

        let line = &self.line[..self.len]
            .strip_suffix(b"\r")
            .unwrap_or(&self.line[..self.len]);
        let valid = validate_checksum(line);
        self.len = 0;
        if !valid {
            return Err(NmeaError::InvalidChecksum);
        }
        Ok(Some(line))
    }
}

impl<const N: usize> Default for NmeaParser<N> {
    fn default() -> Self {
        Self::new()
    }
}

fn validate_checksum(line: &[u8]) -> bool {
    if line.len() < 7 || line[0] != b'$' {
        return false;
    }
    let Some(star) = line.iter().position(|&byte| byte == b'*') else {
        return false;
    };
    if star + 3 != line.len() {
        return false;
    }

    let checksum = line[1..star]
        .iter()
        .fold(0, |checksum, byte| checksum ^ byte);
    let Some(high) = hex_digit(line[star + 1]) else {
        return false;
    };
    let Some(low) = hex_digit(line[star + 2]) else {
        return false;
    };
    checksum == (high << 4 | low)
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

pub struct Lc76g<I> {
    i2c: I,
}

impl<I: I2c> Lc76g<I> {
    pub fn new(i2c: I) -> Self {
        Self { i2c }
    }

    pub async fn read_nmea<'a>(
        &mut self,
        buffer: &'a mut [u8],
    ) -> Result<&'a [u8], GnssError<I::Error>> {
        let available = self.read_nmea_length().await?;
        if available == 0 {
            return Ok(&buffer[..0]);
        }
        if available > buffer.len() {
            return Err(GnssError::BufferTooSmall {
                required: available,
                available: buffer.len(),
            });
        }

        self.write_config(CONFIG_READ_DATA, available as u32)
            .await?;
        Timer::after(INTER_COMMAND_DELAY).await;
        self.read_data(&mut buffer[..available]).await?;
        Ok(&buffer[..available])
    }

    pub async fn read_nmea_chunk<'a>(
        &mut self,
        buffer: &'a mut [u8],
    ) -> Result<&'a [u8], GnssError<I::Error>> {
        let available = self.read_nmea_length().await?;
        let read_len = available.min(buffer.len());
        if read_len == 0 {
            return Ok(&buffer[..0]);
        }

        self.write_config(CONFIG_READ_DATA, read_len as u32).await?;
        Timer::after(INTER_COMMAND_DELAY).await;
        self.read_data(&mut buffer[..read_len]).await?;
        Ok(&buffer[..read_len])
    }

    pub async fn write_nmea(&mut self, data: &[u8]) -> Result<(), GnssError<I::Error>> {
        let free = self.read_buffer_length(CONFIG_READ_FREE_LENGTH).await?;
        if data.len() > free {
            return Err(GnssError::BufferTooSmall {
                required: data.len(),
                available: free,
            });
        }

        self.write_config(CONFIG_WRITE_NMEA, data.len() as u32)
            .await?;
        Timer::after(INTER_COMMAND_DELAY).await;
        for attempt in 0..MAX_I2C_RETRIES {
            match self.i2c.write(WRITE_DATA_ADDRESS, data).await {
                Ok(()) => return Ok(()),
                Err(error) if attempt + 1 == MAX_I2C_RETRIES => {
                    return Err(GnssError::I2c {
                        operation: GnssOperation::WriteData,
                        error,
                    });
                }
                Err(_) => Timer::after(INTER_COMMAND_DELAY).await,
            }
        }
        unreachable!()
    }

    async fn read_nmea_length(&mut self) -> Result<usize, GnssError<I::Error>> {
        self.read_buffer_length(CONFIG_READ_NMEA_LENGTH).await
    }

    async fn read_buffer_length(&mut self, command: u32) -> Result<usize, GnssError<I::Error>> {
        self.write_config(command, NMEA_LENGTH_BYTES as u32).await?;
        Timer::after(INTER_COMMAND_DELAY).await;
        let mut length = [0; NMEA_LENGTH_BYTES];
        self.read_data(&mut length).await?;
        Ok(u32::from_le_bytes(length) as usize)
    }

    async fn read_data(&mut self, data: &mut [u8]) -> Result<(), GnssError<I::Error>> {
        for attempt in 0..MAX_I2C_RETRIES {
            match self.i2c.read(DATA_ADDRESS, data).await {
                Ok(()) => return Ok(()),
                Err(error) if attempt + 1 == MAX_I2C_RETRIES => {
                    return Err(GnssError::I2c {
                        operation: if data.len() == NMEA_LENGTH_BYTES {
                            GnssOperation::ReadLength
                        } else {
                            GnssOperation::ReadData
                        },
                        error,
                    });
                }
                Err(_) => Timer::after(INTER_COMMAND_DELAY).await,
            }
        }
        unreachable!()
    }

    async fn write_config(&mut self, command: u32, length: u32) -> Result<(), GnssError<I::Error>> {
        let mut request = [0; 8];
        request[..4].copy_from_slice(&command.to_le_bytes());
        request[4..].copy_from_slice(&length.to_le_bytes());
        for attempt in 0..MAX_I2C_RETRIES {
            match self.i2c.write(CONFIG_ADDRESS, &request).await {
                Ok(()) => return Ok(()),
                Err(error) if attempt + 1 == MAX_I2C_RETRIES => {
                    return Err(GnssError::I2c {
                        operation: GnssOperation::WriteConfig,
                        error,
                    });
                }
                Err(_) => Timer::after(INTER_COMMAND_DELAY).await,
            }
        }
        unreachable!()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CONFIG_READ_DATA, CONFIG_READ_FREE_LENGTH, CONFIG_READ_NMEA_LENGTH, NmeaError, NmeaParser,
    };

    #[test]
    fn commands_match_quectel_i2c_protocol() {
        assert_eq!(
            CONFIG_READ_NMEA_LENGTH.to_le_bytes(),
            [0x08, 0x00, 0x51, 0xAA]
        );
        assert_eq!(CONFIG_READ_DATA.to_le_bytes(), [0x00, 0x20, 0x51, 0xAA]);
        assert_eq!(
            CONFIG_READ_FREE_LENGTH.to_le_bytes(),
            [0x04, 0x00, 0x51, 0xAA]
        );
    }

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
}
