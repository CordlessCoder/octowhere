//! A node of the location mesh in `context/LORA-PROTOCOL.md`: what it shows the screens of the
//! mesh and takes from them ([`view`]), and the changes it stores ([`GroupWrite`]). It has no
//! radio or board dependency, so it builds and tests on the host.

#![no_std]

extern crate alloc;

pub mod view;
mod write;

pub use write::GroupWrite;
