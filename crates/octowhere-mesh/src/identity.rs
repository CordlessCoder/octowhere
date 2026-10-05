//! A device's identity: an Ed25519 key pair from a stored seed, which signs its records, with the
//! X25519 key for pairing and pairwise keys derived from it (`context/LORA-PROTOCOL.md`,
//! "Signatures").

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use x25519_dalek::{PublicKey, StaticSecret};

use crate::members::{MAC_LEN, Name, PUBLIC_LEN};

pub const SEED_LEN: usize = 32;
pub const SIGNATURE_LEN: usize = 64;
/// The most bytes a signature covers: a key message's domain string, fields and sealed body.
const SIGNED_MAX: usize = 256;

#[derive(Clone)]
pub struct Identity {
    signing: SigningKey,
    pub mac: [u8; MAC_LEN],
    pub name: Name,
}

impl Identity {
    #[must_use]
    pub fn new(seed: [u8; SEED_LEN], mac: [u8; MAC_LEN], name: Name) -> Self {
        Self {
            signing: SigningKey::from_bytes(&seed),
            mac,
            name,
        }
    }

    /// What is stored to make the same identity again.
    #[must_use]
    pub fn seed(&self) -> [u8; SEED_LEN] {
        self.signing.to_bytes()
    }

    /// The Ed25519 public key, which names this device in its records.
    #[must_use]
    pub fn public(&self) -> [u8; PUBLIC_LEN] {
        self.signing.verifying_key().to_bytes()
    }

    /// The X25519 secret whose public key [`dh_public`] gives from [`Identity::public`].
    pub(crate) fn dh(&self) -> StaticSecret {
        StaticSecret::from(self.signing.to_scalar_bytes())
    }

    /// Signs the concatenation of `parts`, which must come to at most [`SIGNED_MAX`] bytes.
    pub(crate) fn sign(&self, parts: &[&[u8]]) -> [u8; SIGNATURE_LEN] {
        let mut buffer = [0; SIGNED_MAX];
        let len = join(parts, &mut buffer).expect("what this device signs fits");
        self.signing.sign(&buffer[..len]).to_bytes()
    }
}

/// The X25519 public key of the device whose identity is `public`, or `None` for bytes that are
/// no Ed25519 public key.
#[must_use]
pub fn dh_public(public: &[u8; PUBLIC_LEN]) -> Option<PublicKey> {
    let key = VerifyingKey::from_bytes(public).ok()?;
    Some(PublicKey::from(key.to_montgomery().to_bytes()))
}

/// Whether `signature` is `public`'s over the concatenation of `parts`. Strict: a key of small
/// order or a signature with another encoding of the same value is refused.
#[must_use]
pub fn verify(public: &[u8; PUBLIC_LEN], parts: &[&[u8]], signature: &[u8; SIGNATURE_LEN]) -> bool {
    let mut buffer = [0; SIGNED_MAX];
    let Some(len) = join(parts, &mut buffer) else {
        return false;
    };
    VerifyingKey::from_bytes(public).is_ok_and(|key| {
        key.verify_strict(&buffer[..len], &Signature::from_bytes(signature))
            .is_ok()
    })
}

fn join(parts: &[&[u8]], out: &mut [u8; SIGNED_MAX]) -> Option<usize> {
    let mut len = 0;
    for part in parts {
        out.get_mut(len..len + part.len())?.copy_from_slice(part);
        len += part.len();
    }
    Some(len)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(n: u8) -> Identity {
        let mac = [0x10, 0, 0, 0, 0, n];
        Identity::new([n; SEED_LEN], mac, Name::from_mac(&mac))
    }

    #[test]
    fn the_derived_x25519_keys_agree() {
        let (a, b) = (identity(1), identity(2));
        let ab = a.dh().diffie_hellman(&dh_public(&b.public()).unwrap());
        let ba = b.dh().diffie_hellman(&dh_public(&a.public()).unwrap());
        assert_eq!(ab.as_bytes(), ba.as_bytes());
        assert_eq!(
            PublicKey::from(&a.dh()).to_bytes(),
            dh_public(&a.public()).unwrap().to_bytes()
        );
    }

    #[test]
    fn a_signature_holds_only_for_its_bytes_and_key() {
        let a = identity(1);
        let signature = a.sign(&[b"octowhere test", &[1, 2, 3]]);
        assert!(verify(
            &a.public(),
            &[b"octowhere test", &[1, 2, 3]],
            &signature
        ));
        assert!(
            verify(&a.public(), &[b"octowhere test", &[1, 2], &[3]], &signature),
            "the parts are joined"
        );
        assert!(!verify(
            &a.public(),
            &[b"octowhere test", &[1, 2, 4]],
            &signature
        ));
        assert!(!verify(
            &identity(2).public(),
            &[b"octowhere test", &[1, 2, 3]],
            &signature
        ));
        assert!(!verify(&[0xff; 32], &[b"octowhere test"], &signature));
        assert!(!verify(&a.public(), &[&[0; SIGNED_MAX + 1]], &signature));
    }

    #[test]
    fn a_seed_gives_back_its_identity() {
        let a = identity(7);
        let again = Identity::new(a.seed(), a.mac, a.name);
        assert_eq!(again.public(), a.public());
    }
}
