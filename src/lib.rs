//! This is a library implementation that implements the PLS (Private Law Society) Bitcoin
//! contracts protocol.
//! It's responsible for defining the multisig generation part.
//! That's a beta lib. Understand that it's under progressive development and it can have
//! compatibility issues with future protocol versions.
//! Major updates means protocol incompatibility with older versions.
//! Middle ones means possible incompatible structs and/or functions definitions or perhaps new
//! developed features.

pub mod multisig;

pub use multisig::*;

pub mod utils;
