pub mod framebuffer;
pub mod qspi_bus;

/// The panel's controller as the board wires it.
pub type Display<'d, C> = co5300::Co5300<
    qspi_bus::QspiBus<'d>,
    esp_hal::gpio::Output<'d>,
    esp_hal::gpio::Input<'d>,
    embassy_time::Delay,
    C,
>;
