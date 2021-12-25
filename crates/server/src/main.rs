#[cfg(feature = "legacy")]
mod legacy;

use rendezvous_common::{anyhow, tracing};

fn main() -> anyhow::Result<()> {
    tracing::init()?;

    serve_legacy()?;

    Ok(())
}

#[cfg(feature = "legacy")]
fn serve_legacy() -> anyhow::Result<()> {
    crate::legacy::serve()
}

#[cfg(not(feature = "legacy"))]
fn serve_legacy() -> anyhow::Result<()> {
    Ok(())
}
