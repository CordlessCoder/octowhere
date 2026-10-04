//! The board's random source: the hardware's true random source, which `async_main` enables
//! with the ADC's noise at boot.

use super::Random;

pub struct BoardRandom;

impl Random for BoardRandom {
    fn fill(&mut self, out: &mut [u8]) -> bool {
        let Ok(trng) = esp_hal::rng::Trng::try_new() else {
            return false;
        };
        trng.read(out);
        true
    }
}

/// Random bytes from the board's source, or `None` without it.
pub fn random<const N: usize>() -> Option<[u8; N]> {
    BoardRandom.bytes()
}
