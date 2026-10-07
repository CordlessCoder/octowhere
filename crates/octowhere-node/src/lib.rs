//! A node of the location mesh in `context/LORA-PROTOCOL.md`: what it shows the screens of the
//! mesh and takes from them ([`view`]), the changes it stores ([`GroupWrite`]), and with the
//! `run` feature the node itself, which runs on whatever radio, clock, random source, device and
//! store it is given. It has no board dependency, so it builds and tests on the host; `run`
//! needs nightly, for the allocator its large stores are made in.

#![no_std]
#![cfg_attr(feature = "run", feature(allocator_api))]
// From Rust 1.100, nightly names the part still unstable `allocator_ext`, which the `esp`
// toolchain the firmware builds with does not know yet.
#![cfg_attr(feature = "run", allow(stable_features))]
// The node runs on one executor, so its seams' futures need no `Send` bound.
#![allow(async_fn_in_trait)]

extern crate alloc;

#[cfg(feature = "run")]
#[macro_use]
mod fmt;
#[cfg(feature = "run")]
pub mod access;
#[cfg(feature = "run")]
mod inbox;
#[cfg(feature = "run")]
mod node;
#[cfg(feature = "run")]
pub mod removals;
#[cfg(feature = "run")]
mod unsaved;
pub mod view;
mod write;

#[cfg(feature = "run")]
pub use node::{
    Command, Commands, Device, Fix, GpsTime, GroupStore, Mesh, RECEIVED_MAX, Radio, Random,
    Received, Sent, Start, Time, blank_view, offline, publish_start,
};
pub use write::{GroupWrite, KeptRow};
