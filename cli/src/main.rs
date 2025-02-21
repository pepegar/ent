use anyhow::Result;
use clap::Parser;
use ent_proto::ent::{
    graph_service_client::GraphServiceClient, schema_service_client::SchemaServiceClient,
};

use commands::{admin, edge, object};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// The endpoint to connect to
    #[arg(long, default_value = "http2://127.0.0.1:50051")]
    endpoint: String,

    /// The authentication token
    #[arg(long)]
    auth: Option<String>,

    #[command(subcommand)]
    command: commands::Commands,
}

mod commands;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let mut client = GraphServiceClient::connect(cli.endpoint.clone()).await?;
    let mut schema_client = SchemaServiceClient::connect(cli.endpoint).await?;

    match cli.command {
        commands::Commands::Admin(cmd) => admin::execute(cmd, &mut schema_client).await,
        commands::Commands::GetObject(cmd) => object::execute(cmd, &mut client, cli.auth).await,
        commands::Commands::GetEdge(cmd) => {
            edge::execute_get_edge(cmd, &mut client, cli.auth).await
        }
        commands::Commands::GetEdges(cmd) => {
            edge::execute_get_edges(cmd, &mut client, cli.auth).await
        }
        commands::Commands::CreateObject(cmd) => {
            object::execute_create_object(cmd, &mut client, cli.auth).await
        }
        commands::Commands::CreateEdge(cmd) => {
            edge::execute_create_edge(cmd, &mut client, cli.auth).await
        }
    }
}
