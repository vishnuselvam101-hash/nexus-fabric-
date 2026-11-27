//! Synthetic engineering-drawing generator with exact ground truth. Training data for the NF Reader.
//! Not linked into nf-server. Every printed number is an engine value (see `source`).
pub mod compose;
pub mod layout;
pub mod params;
pub mod render;
pub mod rng;
pub mod run;
pub mod source;
