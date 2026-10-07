//! Several mesh nodes on the host, each running `octowhere_node`'s node unchanged on virtual
//! time. A simulated air carries their packets by a link matrix, with collisions and half-duplex
//! radios. Each node's clock has its own drift, its store can fail a write and restart it from
//! what it holds, and every random source is seeded, so that a run repeats. Needs nightly, as
//! the node does.

#![feature(allocator_api)]
// From Rust 1.100, nightly names the part still unstable `allocator_ext`.
#![allow(stable_features)]

pub mod air;
mod logs;
mod rng;
mod seams;
mod sim;
mod store;
mod world;

pub use logs::Line;
pub use sim::{Config, Sim, UTC0_S, alone, grouped};
pub use world::{CAPTURE_DB, Link, Receptions, Transmission};
