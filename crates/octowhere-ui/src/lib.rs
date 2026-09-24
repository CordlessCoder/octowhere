//! Everything between the sensors and the framebuffer: the screens, their drawing, and the
//! handling of touch input. Nothing here touches the board, so the same code runs on the host.
#![cfg_attr(not(test), no_std)]
#![cfg_attr(feature = "allocator-api", feature(allocator_api))]
#![deny(clippy::mem_forget)]
#![warn(unused_must_use)]
extern crate alloc;

pub use fontdue;
pub use octowhere_tz as tz;

pub mod board;
pub mod chrome;
pub mod framebuffer;
pub mod ui;
pub mod part_timing;
