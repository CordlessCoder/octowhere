#[derive(Debug)]
pub enum Sx127xError<SPI> {
    InvalidInput,
    InvalidPayloadLength,
    InvalidState,
    InvalidVersion,
    ModeNotReady,
    PacketNotReady,
    PacketTermination,
    SF6RequiresImplicitHeaderMode,
    SPI(SPI),
}
