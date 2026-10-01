//! Fields packed most significant bit first, across byte boundaries.

/// The buffer has no room for the field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Full;

pub struct BitWriter<'a> {
    buf: &'a mut [u8],
    bit: usize,
}

impl<'a> BitWriter<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, bit: 0 }
    }

    /// Appends the low `bits` of `value`.
    pub fn put(&mut self, value: u64, bits: u32) -> Result<(), Full> {
        if self.bit + bits as usize > self.buf.len() * 8 {
            return Err(Full);
        }
        for i in (0..bits).rev() {
            let (byte, shift) = (self.bit / 8, 7 - self.bit % 8);
            if shift == 7 {
                self.buf[byte] = 0;
            }
            self.buf[byte] |= (((value >> i) & 1) as u8) << shift;
            self.bit += 1;
        }
        Ok(())
    }

    /// The bytes written, the last one padded with zeros.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bit.div_ceil(8)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bit == 0
    }
}

pub struct BitReader<'a> {
    buf: &'a [u8],
    bit: usize,
}

impl<'a> BitReader<'a> {
    #[must_use]
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, bit: 0 }
    }

    /// Takes the next `bits` as the low bits of a value.
    pub fn take(&mut self, bits: u32) -> Option<u64> {
        if self.remaining() < bits as usize {
            return None;
        }
        let mut value = 0;
        for _ in 0..bits {
            let bit = (self.buf[self.bit / 8] >> (7 - self.bit % 8)) & 1;
            value = value << 1 | u64::from(bit);
            self.bit += 1;
        }
        Some(value)
    }

    #[must_use]
    pub fn remaining(&self) -> usize {
        self.buf.len() * 8 - self.bit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_read_back_across_byte_boundaries() {
        let mut buf = [0xff; 4];
        let mut writer = BitWriter::new(&mut buf);
        writer.put(0b101, 3).unwrap();
        writer.put(0x1_2345, 17).unwrap();
        writer.put(1, 1).unwrap();
        assert_eq!(writer.len(), 3);
        assert_eq!(writer.put(0, 12), Err(Full));
        let mut reader = BitReader::new(&buf[..3]);
        assert_eq!(reader.take(3), Some(0b101));
        assert_eq!(reader.take(17), Some(0x1_2345));
        assert_eq!(reader.take(1), Some(1));
        assert_eq!(reader.take(3), Some(0), "the padding is zero");
        assert_eq!(reader.take(1), None);
    }
}
