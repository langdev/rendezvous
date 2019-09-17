pub use tracing::*;

pub fn init() -> Result<(), subscriber::SetGlobalDefaultError> {
    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt::Subscriber::builder()
            .compact()
            .finish(),
    )
}
