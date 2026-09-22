#[derive(Debug)]
pub enum Sx127xError<SPI> {
    InvalidInput,
    InvalidPayloadLength,
    InvalidState,
    InvalidVersion,
    ModeNotReady,
    PacketTermination,
    SF6RequiresImplicitHeaderMode,
    SPI(SPI),
}
