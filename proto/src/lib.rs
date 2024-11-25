pub mod ent {
    tonic::include_proto!("ent");
}

pub mod proto {
    pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("ent");
}
