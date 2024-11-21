use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let original_out_dir = PathBuf::from(env::var("OUT_DIR")?);
    tonic_build::configure()
        .file_descriptor_set_path(original_out_dir.join("ent.bin"))
        .compile_protos(&["proto/ent/ent.proto"], &["proto"])
        .unwrap();
    Ok(())
}
