fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::fs::canonicalize(std::env::var("CARGO_MANIFEST_DIR")?)?;
    let workspace_dir = dir
        .parent()
        .ok_or("unexpected")?
        .parent()
        .ok_or("unexpected")?;
    tonic_build::compile_protos(workspace_dir.join("proto").join("base.proto"))?;
    Ok(())
}
