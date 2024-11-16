use server::{ent::graph_service_server::GraphServiceServer, EntServer};
use tonic::transport::Server;
use tracing::info;

mod server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let addr = "[::1]:50051".parse()?;
    let ent_server = EntServer::new();

    info!("Server listening on {}", addr);

    Server::builder()
        .add_service(GraphServiceServer::new(ent_server))
        .serve(addr)
        .await?;

    Ok(())
}
