use anyhow::Result;
use clap::{Args, Subcommand};
use ent_proto::ent::{schema_service_client::SchemaServiceClient, CreateSchemaRequest};
use std::path::PathBuf;
use tonic::transport::Channel;

#[derive(Args)]
pub struct AdminCommands {
    #[command(subcommand)]
    pub command: AdminSubcommands,
}

#[derive(Subcommand)]
pub enum AdminSubcommands {
    /// Create a new schema
    CreateSchema(CreateSchemaCommand),
}

#[derive(Args)]
pub struct CreateSchemaCommand {
    /// Path to schema file
    #[arg(long, short)]
    pub file: PathBuf,

    /// Type name for the schema
    #[arg(long)]
    pub type_name: String,

    /// Optional description of the schema
    #[arg(long, short)]
    pub description: Option<String>,
}

pub async fn execute(cmd: AdminCommands, client: &mut SchemaServiceClient<Channel>) -> Result<()> {
    match cmd.command {
        AdminSubcommands::CreateSchema(cmd) => create_schema(cmd, client).await,
    }
}

async fn create_schema(
    cmd: CreateSchemaCommand,
    client: &mut SchemaServiceClient<Channel>,
) -> Result<()> {
    let schema = std::fs::read_to_string(cmd.file)?;

    let request = tonic::Request::new(CreateSchemaRequest {
        schema: schema,
        description: cmd.description.unwrap_or_default(),
        type_name: cmd.type_name,
    });

    let response = client.create_schema(request).await?;
    println!("Created schema with ID: {}", response.get_ref().schema_id);

    Ok(())
}
