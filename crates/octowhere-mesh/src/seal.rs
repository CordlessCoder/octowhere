//! Sealing a packet under the group key with AES-SIV (RFC 5297, AES-CMAC-SIV with a 256-bit key),
//! with no associated data. The synthetic IV leads the packet and is its authentication tag.

use aes_siv::{KeyInit, siv::Aes128Siv};

/// The synthetic IV's length.
pub const SIV_LEN: usize = 16;

/// A group key: the CMAC key, then the CTR key.
#[derive(Clone)]
pub struct GroupKey([u8; 32]);

impl GroupKey {
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    fn cipher(&self) -> Aes128Siv {
        Aes128Siv::new(&self.0.into())
    }
}

/// The packet is not one sealed under this key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Inauthentic;

/// Seals the plaintext in `packet[SIV_LEN..SIV_LEN + plain_len]` in place, writes its IV in front,
/// and returns the packet's length.
pub fn seal(key: &GroupKey, packet: &mut [u8], plain_len: usize) -> usize {
    let len = SIV_LEN + plain_len;
    let (siv, plain) = packet[..len].split_at_mut(SIV_LEN);
    let tag = key
        .cipher()
        .encrypt_inout_detached::<[&[u8]; 0], &[u8]>([], plain.into())
        .expect("no associated data, so S2V cannot run out of components");
    siv.copy_from_slice(&tag);
    len
}

/// Opens a sealed packet in place, returning its plaintext.
pub fn open<'a>(key: &GroupKey, packet: &'a mut [u8]) -> Result<&'a [u8], Inauthentic> {
    if packet.len() < SIV_LEN {
        return Err(Inauthentic);
    }
    let (siv, sealed) = packet.split_at_mut(SIV_LEN);
    let tag = (&*siv).try_into().map_err(|_| Inauthentic)?;
    key.cipher()
        .decrypt_inout_detached::<[&[u8]; 0], &[u8]>([], sealed.into(), &tag)
        .map_err(|_| Inauthentic)?;
    Ok(sealed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex<const N: usize>(text: &str) -> [u8; N] {
        let mut digits = text.bytes().filter_map(|c| (c as char).to_digit(16));
        let bytes =
            core::array::from_fn(|_| (digits.next().unwrap() << 4 | digits.next().unwrap()) as u8);
        assert!(digits.next().is_none());
        bytes
    }

    /// RFC 5297, appendix A.1, which has one header; it checks the primitive `seal` uses.
    #[test]
    fn the_cipher_is_rfc_5297s() {
        let key = GroupKey::new(hex(
            "fffefdfc fbfaf9f8 f7f6f5f4 f3f2f1f0 f0f1f2f3 f4f5f6f7 f8f9fafb fcfdfeff",
        ));
        let ad: [u8; 24] = hex("10111213 14151617 18191a1b 1c1d1e1f 20212223 24252627");
        let mut plain: [u8; 14] = hex("11223344 55667788 99aabbcc ddee");
        let tag = key
            .cipher()
            .encrypt_inout_detached([&ad[..]], (&mut plain[..]).into())
            .unwrap();
        assert_eq!(tag[..], hex::<16>("85632d07 c6e8f37f 950acd32 0a2ecc93"));
        assert_eq!(plain, hex::<14>("40c02b96 90c4dc04 daef7f6a fe5c"));
    }

    #[test]
    fn a_sealed_packet_opens_only_untouched_and_under_its_key() {
        let key = GroupKey::new([7; 32]);
        let mut packet = [0u8; 64];
        packet[SIV_LEN..SIV_LEN + 20].copy_from_slice(b"twenty bytes of text");
        let len = seal(&key, &mut packet, 20);
        assert_eq!(len, SIV_LEN + 20);
        assert_ne!(&packet[SIV_LEN..len], b"twenty bytes of text");

        let mut copy = packet;
        assert_eq!(
            open(&key, &mut copy[..len]),
            Ok(&b"twenty bytes of text"[..])
        );

        let mut flipped = packet;
        flipped[len - 1] ^= 1;
        assert_eq!(open(&key, &mut flipped[..len]), Err(Inauthentic));

        let mut other = packet;
        assert_eq!(
            open(&GroupKey::new([8; 32]), &mut other[..len]),
            Err(Inauthentic)
        );
    }

    #[test]
    fn a_plaintext_shorter_than_a_block_seals() {
        let key = GroupKey::new([1; 32]);
        let mut packet = [0u8; SIV_LEN + 3];
        packet[SIV_LEN..].copy_from_slice(b"abc");
        let len = seal(&key, &mut packet, 3);
        assert_eq!(open(&key, &mut packet[..len]), Ok(&b"abc"[..]));
    }
}
