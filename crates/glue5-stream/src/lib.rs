//! The game's files over the network.
//!
//! A [`server`] serves a game folder (`data/`, `b/<build>/shaders/`); a
//! [`client`] keeps a [`cache`] of it on the device, a partial copy filled as
//! the engine reads. The engine's file system already works this way in the
//! browser build (its HTTP device with read announcements); this is the same
//! for the native app, on a protocol made for it ([`protocol`]): one
//! connection, requests in strict priority order on the server, small files
//! in compressed [`pack`]s.

pub mod cache;
pub mod catalog;
pub mod client;
pub mod discovery;
pub mod pack;
pub mod packages;
pub mod protocol;
pub mod server;
mod sys;

pub use client::{Check, Client, Options, Phase, Status, after_stopped, check};
pub use packages::{GamePackage, PackageFile, PackageManifest, TransferControl, TransferPhase, TransferProgress};
pub use protocol::PORT;
pub use sys::lan_addresses;

#[cfg(test)]
mod tests;
