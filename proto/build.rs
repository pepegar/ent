fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = std::env::var("OUT_DIR")?;
    let out_path = std::path::PathBuf::from(&out_dir);

    tonic_build::configure()
        .file_descriptor_set_path(out_path.join("ent.bin"))
        .compile_protos(&["src/ent.proto"], &["src"])?;

    Ok(())
}
