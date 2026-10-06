#![no_std]
#![feature(allocator_api)]
#![deny(clippy::mem_forget)]
#![expect(unused)]
#![warn(unused_must_use)]
extern crate alloc;

pub mod board;
pub mod drivers;
pub mod gnss_time;
pub mod settings;
pub mod settings_queue;
pub mod util;

pub use octowhere_peripherals as peripherals;
pub use octowhere_ui::{chrome, fontdue, framebuffer, motion, tz, ui};
