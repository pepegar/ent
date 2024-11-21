pub mod ent {
    tonic::include_proto!("ent");
}

mod graph_server;
mod schema_server;

pub use graph_server::GraphServer;
pub use schema_server::SchemaServer;

pub mod proto {
    tonic::include_proto!("ent");

    pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("ent");
}
