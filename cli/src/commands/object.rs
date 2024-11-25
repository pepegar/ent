use anyhow::Result;
use clap::Args;
use ent_proto::ent::{
    consistency_requirement::Requirement, graph_service_client::GraphServiceClient,
    ConsistencyRequirement, GetObjectRequest,
};
use tonic::transport::Channel;

#[derive(Args)]
pub struct GetObjectCommand {
    /// Object ID to retrieve
    pub id: i64,

    /// Optional consistency requirement
    #[arg(long)]
    pub consistency: Option<String>,
}

pub async fn execute(
    cmd: GetObjectCommand,
    client: &mut GraphServiceClient<Channel>,
) -> Result<()> {
    let consistency = parse_consistency(cmd.consistency)?;

    let request = tonic::Request::new(GetObjectRequest {
        object_id: cmd.id,
        user_token: String::new(), // TODO: Add user token support
        consistency,
    });

    let response = client.get_object(request).await?;
    println!("{:#?}", response.get_ref());

    Ok(())
}

pub(super) fn parse_consistency(
    consistency: Option<String>,
) -> Result<Option<ConsistencyRequirement>> {
    match consistency {
        None => Ok(None),
        Some(c) => match c.as_str() {
            "full" => Ok(Some(ConsistencyRequirement {
                requirement: Some(Requirement::FullConsistency(true)),
            })),
            "minimum" => Ok(Some(ConsistencyRequirement {
                requirement: Some(Requirement::MinimizeLatency(true)),
            })),
            _ => Err(anyhow::anyhow!("Invalid consistency requirement")),
        },
    }
}
