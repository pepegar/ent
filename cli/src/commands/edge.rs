use anyhow::Result;
use clap::Args;
use ent_proto::ent::{graph_service_client::GraphServiceClient, GetEdgeRequest, GetEdgesRequest};
use tonic::transport::Channel;

use super::object::parse_consistency;

#[derive(Args)]
pub struct GetEdgeCommand {
    /// Source object ID
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
    pub object_id: i64,

    /// Type of edges to retrieve
    #[arg(long, short)]
    pub edge_type: String,

    /// Optional consistency requirement  
    #[arg(long)]
    pub consistency: Option<String>,
}

pub async fn execute_get_edge(
    cmd: GetEdgeCommand,
    client: &mut GraphServiceClient<Channel>,
) -> Result<()> {
    let consistency = parse_consistency(cmd.consistency)?;

    let request = tonic::Request::new(GetEdgeRequest {
        object_id: cmd.object_id,
        user_token: String::new(), // TODO: Add user token support
        edge_type: cmd.edge_type,
        consistency,
    });

    let response = client.get_edge(request).await?;
    println!("{:#?}", response.get_ref());

    Ok(())
}

pub async fn execute_get_edges(
    cmd: GetEdgesCommand,
    client: &mut GraphServiceClient<Channel>,
) -> Result<()> {
    let consistency = parse_consistency(cmd.consistency)?;

    let request = tonic::Request::new(GetEdgesRequest {
        object_id: cmd.object_id,
        user_token: String::new(), // TODO: Add user token support
        edge_type: cmd.edge_type,
        consistency,
    });

    let response = client.get_edges(request).await?;
    println!("{:#?}", response.get_ref());

    Ok(())
}
