use std::fs;

use anyhow::{anyhow, Result};
use ent_proto::ent::{
    graph_service_server::GraphServiceServer, schema_service_server::SchemaServiceServer,
};
use sqlx::postgres::PgPoolOptions;
use tonic::transport::Server;
use tracing::{error, info};

use ent_server::{auth::JwtValidator, config::Settings, GraphServer, SchemaServer};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let settings = Settings::new().map_err(|e| {
        error!(error = e.to_string());
        e
    })?;

    let addr = settings.server_address().parse().map_err(|e| {
        error!("Error parsing server address: {}", e);
        e
    })?;

    info!(path = &settings.jwt.public_key_path);

    let public_key = fs::read_to_string(&settings.jwt.public_key_path).map_err(|e| {
        error!("failed to read pem file: {}", e);
        e
    })?;

    JwtValidator::init(&public_key, settings.jwt.issuer.clone()).map_err(|e| {
        error!("failed to initialize JWT validator: {}", e);
        e
    })?;

    let pool = PgPoolOptions::new()
        .max_connections(settings.database.max_connections)
        .connect(&settings.database.url)
        .await?;

    let graph_pool = pool.clone();

    let (_, health) = tonic_health::server::health_reporter();
    let graph_server = GraphServer::new(graph_pool);
    let schema_server = SchemaServer::new(pool);

    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(ent_proto::proto::FILE_DESCRIPTOR_SET)
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
