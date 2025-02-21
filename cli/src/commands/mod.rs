use clap::{Parser, Subcommand};

pub mod admin;
pub mod edge;
pub mod object;

#[derive(Parser)]
#[command(name = "ent")]
#[command(about = "CLI for Ent graph database", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// The endpoint to connect to
    #[arg(long, default_value = "http2://127.0.0.1:50051")]
    pub endpoint: String,

    /// The authentication token
    #[arg(long)]
    pub auth: Option<String>,
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

    /// Create a new object
    CreateObject(object::CreateObjectCommand),

    /// Create a new edge
    CreateEdge(edge::CreateEdgeCommand),
}
