#![deny(rust_2018_idioms)]
#![deny(proc_macro_derive_resolution_fallback)]

pub mod data;
#[cfg(feature = "legacy")]
pub mod ipc;
pub mod proto;
pub mod tracing;

pub use anyhow;
pub use futures;
#[cfg(feature = "legacy")]
pub use nng;
pub use parking_lot;
pub use serde;
pub use serde_cbor;
pub use tokio;
pub use tonic;
