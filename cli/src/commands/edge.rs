use anyhow::Result;
use clap::Args;
use ent_proto::ent::{
    graph_service_client::GraphServiceClient, CreateEdgeRequest, GetEdgeRequest, GetEdgesRequest,
};
use ent_server::auth::RequestExt;
use prost_types::Struct;
use serde_json::Value as JsonValue;
use std::fs;
use std::path::PathBuf;
use tonic::transport::Channel;

use super::object::{json_value_to_prost_value, parse_consistency};

#[derive(Args)]
pub struct GetEdgeCommand {
    /// Source object ID
    #[arg(long)]
    pub object_id: i64,

    /// Type of edge to retrieve
    #[arg(long, short)]
    pub edge_type: String,

    /// Optional consistency requirement
    #[arg(long)]
    pub consistency: Option<String>,
}

#[derive(Args)]
pub struct GetEdgesCommand {
    /// Source object ID
    #[arg(long)]
    pub object_id: i64,

    /// Type of edges to retrieve
    #[arg(long, short)]
    pub edge_type: String,

    /// Optional consistency requirement  
    #[arg(long)]
    pub consistency: Option<String>,
}

#[derive(Args)]
pub struct CreateEdgeCommand {
    /// Source object ID
    #[arg(long)]
    pub from_id: i64,

    /// Source object type
    #[arg(long)]
    pub from_type: String,

    /// Target object ID
    #[arg(long)]
    pub to_id: i64,

    /// Target object type
    #[arg(long)]
    pub to_type: String,

    /// Edge relation type
    #[arg(long)]
    pub relation: String,

    /// Optional path to JSON file containing edge metadata
    #[arg(long)]
    pub metadata_file: Option<PathBuf>,
}

pub async fn execute_get_edge(
    cmd: GetEdgeCommand,
    client: &mut GraphServiceClient<Channel>,
    auth: Option<String>,
) -> Result<()> {
    let _consistency = parse_consistency(cmd.consistency)?;

    let request = tonic::Request::new(GetEdgeRequest {
        object_id: cmd.object_id,
        edge_type: cmd.edge_type,
        consistency: None,
    });

    let request = if let Some(token) = auth {
        request.with_bearer_token(&token)?
    } else {
        request
    };

    let response = client.get_edge(request).await?;
    println!("{:#?}", response.get_ref());

    Ok(())
}

pub async fn execute_get_edges(
    cmd: GetEdgesCommand,
    client: &mut GraphServiceClient<Channel>,
    auth: Option<String>,
) -> Result<()> {
    let _consistency = parse_consistency(cmd.consistency)?;

    let request = tonic::Request::new(GetEdgesRequest {
        object_id: cmd.object_id,
        edge_type: cmd.edge_type,
        consistency: None,
    });

    let request = if let Some(token) = auth {
        request.with_bearer_token(&token)?
    } else {
        request
    };

    let response = client.get_edges(request).await?;
    println!("{:#?}", response.get_ref());

    Ok(())
}

pub async fn execute_create_edge(
    cmd: CreateEdgeCommand,
    client: &mut GraphServiceClient<Channel>,
    auth: Option<String>,
) -> Result<()> {
    let metadata = if let Some(path) = cmd.metadata_file {
        let metadata_json: JsonValue = serde_json::from_str(&fs::read_to_string(path)?)?;

        let mut metadata_struct = Struct::default();
        if let JsonValue::Object(map) = metadata_json {
            for (k, v) in map {
                metadata_struct
                    .fields
                    .insert(k, json_value_to_prost_value(v));
            }
        }
        Some(metadata_struct)
    } else {
        None
    };

    let request = tonic::Request::new(CreateEdgeRequest {
        from_id: cmd.from_id,
        from_type: cmd.from_type,
        to_id: cmd.to_id,
        to_type: cmd.to_type,
        relation: cmd.relation,
        metadata,
    });

    let request = if let Some(token) = auth {
        request.with_bearer_token(&token)?
    } else {
        request
    };

    let response = client.create_edge(request).await?;
    println!("{:#?}", response.get_ref());

    Ok(())
}
