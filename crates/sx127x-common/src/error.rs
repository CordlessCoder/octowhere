#[derive(Debug)]
pub enum Sx127xError<SPI> {
    InvalidInput,
    InvalidPayloadLength,
    InvalidState,
    /// The chip answered with this version, not the variant's.
    InvalidVersion(u8),
    ModeNotReady,
    PacketNotReady,
    PacketTermination,
    SF6RequiresImplicitHeaderMode,
    SPI(SPI),
}
