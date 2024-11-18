pub mod ent {
    tonic::include_proto!("ent");
}

mod graph_server;
mod schema_server;

pub use graph_server::GraphServer;
pub use schema_server::SchemaServer;
