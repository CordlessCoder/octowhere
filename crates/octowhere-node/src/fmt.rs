//! The node's logging: through defmt on the board, through `log` on the host, or nowhere. A
//! format string has to read the same to both, so it keeps to `{}` and `{:?}`, with `#`, zero
//! padding and `x` as hints, and byte slices go through [`Mac`] and [`Ascii`].

macro_rules! log_at {
    ($level:ident, $($arg:tt)*) => {{
        #[cfg(feature = "defmt")]
        ::defmt::$level!($($arg)*);
        #[cfg(all(feature = "log", not(feature = "defmt")))]
        ::log::$level!($($arg)*);
        #[cfg(not(any(feature = "defmt", feature = "log")))]
        let _ = ::core::format_args!($($arg)*);
    }};
}

macro_rules! debug {
    ($($arg:tt)*) => { log_at!(debug, $($arg)*) };
}

macro_rules! info {
    ($($arg:tt)*) => { log_at!(info, $($arg)*) };
}

macro_rules! warn {
    ($($arg:tt)*) => { log_at!(warn, $($arg)*) };
}

use core::fmt;

/// Bytes as two hexadecimal digits each, as a hardware address is shown.
pub struct Mac<'a>(pub &'a [u8]);

/// Printable ASCII as text.
pub struct Ascii<'a>(pub &'a [u8]);

#[cfg(feature = "defmt")]
impl defmt::Format for Mac<'_> {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "{=[u8]:02x}", self.0);
    }
}

impl fmt::Display for Mac<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("[")?;
        for (i, byte) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{byte:02x}")?;
        }
        f.write_str("]")
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for Ascii<'_> {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "{=[u8]:a}", self.0);
    }
}

impl fmt::Display for Ascii<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0.escape_ascii())
    }
}
