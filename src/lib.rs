#![no_std]
#![feature(allocator_api)]
#![deny(clippy::mem_forget)]
#![expect(unused)]
#![warn(unused_must_use)]
extern crate alloc;

pub mod board;
pub mod drivers;
pub mod peripherals;
pub mod settings;
pub mod util;

pub use octowhere_ui::{chrome, fontdue, framebuffer, tz, ui};
