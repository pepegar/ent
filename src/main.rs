use anyhow::anyhow;
use server::{
    ent::{graph_service_server::GraphServiceServer, schema_service_server::SchemaServiceServer},
    GraphServer, SchemaServer,
};
use sqlx::postgres::PgPoolOptions;
use tonic::transport::Server;
use tracing::info;

use config::Settings;

mod config;
mod db;
mod server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let settings = Settings::new()?;
    let addr = settings.server_address().parse()?;

    let pool = PgPoolOptions::new()
        .max_connections(settings.database.max_connections)
        .connect(&settings.database.url)
        .await?;

    let graph_pool = pool.clone();

    let (_, health) = tonic_health::server::health_reporter();
    let graph_server = GraphServer::new(graph_pool);
    let schema_server = SchemaServer::new(pool);

    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(server::proto::FILE_DESCRIPTOR_SET)
        .build_v1()
        .map_err(|e| anyhow!("failed to build grpc reflection service: {}", e))?;

    info!("Server listening on {}", addr);

    Server::builder()
        .add_service(GraphServiceServer::new(graph_server))
        .add_service(SchemaServiceServer::new(schema_server))
        .add_service(health)
        .add_service(reflection_service)
        .serve(addr)
        .await
        .map_err(|e| anyhow!("tonic server exited with error: {}", e))?;

    Ok(())
}
