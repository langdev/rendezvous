#![deny(rust_2018_idioms)]
#![deny(proc_macro_derive_resolution_fallback)]

pub mod data;
pub mod discovery;
pub mod ipc;
pub mod tracing;

pub use anyhow;
pub use nng;
pub use parking_lot;
pub use serde;
pub use serde_cbor;
