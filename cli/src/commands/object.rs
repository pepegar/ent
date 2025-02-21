use anyhow::Result;
use clap::Args;
use ent_proto::ent::{
    consistency_requirement::Requirement, graph_service_client::GraphServiceClient,
    ConsistencyRequirement, CreateObjectRequest, GetObjectRequest,
};
use prost_types::{Struct, Value as ProstValue};
use serde_json::Value as JsonValue;
use std::fs;
use std::path::PathBuf;
use tonic::transport::Channel;

#[derive(Args)]
pub struct GetObjectCommand {
    /// Object ID to retrieve
    #[arg(long)]
    pub object_id: i64,

    /// Optional consistency requirement
    #[arg(long)]
    pub consistency: Option<String>,
}

#[derive(Args)]
pub struct CreateObjectCommand {
    /// Path to JSON file containing object metadata
    #[arg(long, short)]
    pub file: PathBuf,

    /// Type of object to create
    #[arg(long, short)]
    pub r#type: String,
}

pub async fn execute(
    cmd: GetObjectCommand,
    client: &mut GraphServiceClient<Channel>,
    auth: Option<String>,
) -> Result<()> {
    let consistency = parse_consistency(cmd.consistency)?;

    let mut request = tonic::Request::new(GetObjectRequest {
        object_id: cmd.object_id,
        consistency: None,
    });

    if let Some(token) = auth {
        request
            .metadata_mut()
            .insert("authorization", token.parse()?);
    }

    let response = client.get_object(request).await?;
    println!("{:#?}", response.get_ref());

    Ok(())
}

pub(super) fn json_value_to_prost_value(json_value: JsonValue) -> ProstValue {
    match json_value {
        JsonValue::Null => ProstValue {
            kind: Some(prost_types::value::Kind::NullValue(0)),
        },
        JsonValue::Bool(b) => ProstValue {
            kind: Some(prost_types::value::Kind::BoolValue(b)),
        },
        JsonValue::Number(n) => {
            if let Some(f) = n.as_f64() {
                ProstValue {
                    kind: Some(prost_types::value::Kind::NumberValue(f)),
                }
            } else {
                // Handle integers that don't fit in f64
                ProstValue {
                    kind: Some(prost_types::value::Kind::StringValue(n.to_string())),
                }
            }
        }
        JsonValue::String(s) => ProstValue {
            kind: Some(prost_types::value::Kind::StringValue(s)),
        },
        JsonValue::Array(arr) => {
            let values: Vec<ProstValue> = arr.into_iter().map(json_value_to_prost_value).collect();
            ProstValue {
                kind: Some(prost_types::value::Kind::ListValue(
                    prost_types::ListValue { values },
                )),
            }
        }
        JsonValue::Object(map) => {
            let mut fields = std::collections::BTreeMap::new();
            for (k, v) in map {
                fields.insert(k, json_value_to_prost_value(v));
            }
            ProstValue {
                kind: Some(prost_types::value::Kind::StructValue(Struct { fields })),
            }
        }
    }
}

pub async fn execute_create_object(
    cmd: CreateObjectCommand,
    client: &mut GraphServiceClient<Channel>,
    auth: Option<String>,
) -> Result<()> {
    let metadata_json: JsonValue = serde_json::from_str(&fs::read_to_string(cmd.file)?)?;

    let mut metadata_struct = Struct::default();
    if let JsonValue::Object(map) = metadata_json {
        for (k, v) in map {
            metadata_struct
                .fields
                .insert(k, json_value_to_prost_value(v));
        }
    }

    let mut request = tonic::Request::new(CreateObjectRequest {
        r#type: cmd.r#type,
        metadata: Some(metadata_struct),
    });

    if let Some(token) = auth {
        request
            .metadata_mut()
            .insert("authorization", token.parse()?);
    }

    let response = client.create_object(request).await?;
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
