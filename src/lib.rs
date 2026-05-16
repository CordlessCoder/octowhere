#![no_std]
#![feature(allocator_api)]
#![deny(clippy::mem_forget)]
#![expect(unused)]
extern crate alloc;

pub mod board;
pub mod chrome;
pub mod drivers;
pub mod peripherals;
pub mod ui;
pub mod util;
