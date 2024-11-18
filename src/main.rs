use server::{
    ent::{graph_service_server::GraphServiceServer, schema_service_server::SchemaServiceServer},
    GraphServer, SchemaServer,
};
use tonic::transport::Server;
use tracing::info;

use config::Settings;

mod config;
mod server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let settings = Settings::new()?;
    let addr = settings.server_address().parse()?;
    let graph_server = GraphServer::new();
    let schema_server = SchemaServer::new();

    info!("Server listening on {}", addr);

    Server::builder()
        .add_service(GraphServiceServer::new(graph_server))
        .add_service(SchemaServiceServer::new(schema_server))
        .serve(addr)
        .await?;

    Ok(())
}
