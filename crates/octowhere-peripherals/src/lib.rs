//! The board's I2C devices: the CST9217 touch controller, the AXP2101 power controller, the
//! PCF85063A RTC and the BMM350 magnetometer. Each driver is generic over `embedded-hal-async`'s
//! I2C, so it builds and tests on the host; the firmware owns bring-up and the shared bus.

#![cfg_attr(not(test), no_std)]
// The register maps keep registers the drivers do not use yet.
#![allow(dead_code)]

mod i2c_helper;
pub mod magnetometer;
pub mod power;
pub mod rtc;
pub mod touch;
