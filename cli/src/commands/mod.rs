use anyhow::Result;
use clap::{Parser, Subcommand};
use ent_proto::ent::{
    graph_service_client::GraphServiceClient, schema_service_client::SchemaServiceClient,
};

mod admin;
mod edge;
mod object;

#[derive(Parser)]
#[command(name = "ent")]
#[command(about = "CLI for Ent graph database", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Administrative commands
    Admin(admin::AdminCommands),

    /// Get an object by ID
    GetObject(object::GetObjectCommand),

    /// Get an edge from an object
    GetEdge(edge::GetEdgeCommand),

    /// Get multiple edges from an object
    GetEdges(edge::GetEdgesCommand),
}

pub async fn execute(cli: Cli) -> Result<()> {
    // TODO: Get from --endpoint flag
    // TODO: have a context system, similar to kubectl
    let addr = "http://127.0.0.1:50051";

    match cli.command {
        Commands::Admin(cmd) => {
            let mut client = SchemaServiceClient::connect(addr).await?;
            admin::execute(cmd, &mut client).await
        }

        Commands::GetObject(cmd) => {
            let mut client = GraphServiceClient::connect(addr).await?;
            object::execute(cmd, &mut client).await
        }

        Commands::GetEdge(cmd) => {
            let mut client = GraphServiceClient::connect(addr).await?;
            edge::execute_get_edge(cmd, &mut client).await
        }

        Commands::GetEdges(cmd) => {
            let mut client = GraphServiceClient::connect(addr).await?;
            edge::execute_get_edges(cmd, &mut client).await
        }
    }
}
