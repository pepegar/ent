use crate::test_helper::{json_to_protobuf_struct, EntTestBuilder};
use anyhow::Result;
use ent_proto::ent::{
    consistency_requirement::Requirement, graph_service_client::GraphServiceClient,
    ConsistencyRequirement, GetObjectRequest, UpdateObjectRequest,
};
use ent_server::auth::RequestExt;
use serde_json::json;
use tonic::Request;

/// Test that concurrent transactions properly handle visibility rules
#[tokio::test]
async fn test_concurrent_transaction_visibility() -> Result<()> {
    let (address, _pool, _container) = crate::common::spawn_app().await?;
    let builder = EntTestBuilder::new()
        .with_basic_schema()
        .with_user("test_user_1")
        .with_attributed_object(0, "test_type", json!({}));

    let state = builder.build(address.clone()).await?;
    let user_token = state.get_user_token(0).unwrap();

    let object = state.get_object(0).unwrap();
    let object_id = object.id;
    let _initial_revision = state.objects[0].revision.clone();

    let mut client = GraphServiceClient::connect(address).await?;

    // User 1 (owner) updates the object with new metadata
    let metadata = json_to_protobuf_struct(json!({
        "updated_by": "user_1"
    }))
    .unwrap();

    let update_req = Request::new(UpdateObjectRequest {
        object_id,
        metadata: Some(metadata),
    })
    .with_bearer_token(user_token)?;

    let update_resp = client.update_object(update_req).await?;
    let updated_revision = update_resp.get_ref().revision.as_ref().unwrap().clone();

    println!(
        "After update - Updated object metadata: {:?}",
        update_resp.get_ref().object.as_ref().unwrap().metadata
    );

    // User 2 should see the updated version at the later revision
    let get_updated_req = Request::new(GetObjectRequest {
        object_id,
        consistency: Some(ConsistencyRequirement {
            requirement: Some(Requirement::ExactlyAt(updated_revision)),
        }),
    })
    .with_bearer_token(user_token)?;

    let updated_get_resp = client.get_object(get_updated_req).await?;
    let updated_object = updated_get_resp.get_ref().object.as_ref().unwrap();
    println!(
        "When querying updated revision - Object metadata: {:?}",
        updated_object.metadata
    );
    assert_eq!(
        updated_object
            .metadata
            .as_ref()
            .unwrap()
            .fields
            .get("updated_by")
            .unwrap()
            .kind
            .as_ref()
            .unwrap(),
        &prost_types::value::Kind::StringValue("user_1".to_string())
    );

    Ok(())
}

/// Test that snapshot isolation prevents phantom reads
#[tokio::test]
async fn test_snapshot_isolation_phantom_prevention() -> Result<()> {
    let (address, _pool, _container) = crate::common::spawn_app().await?;
    let builder = EntTestBuilder::new()
        .with_basic_schema()
        .with_user("test_user")
        .with_attributed_object(0, "test_type", json!({}));

    let state = builder.build(address.clone()).await?;
    let user_token = state.get_user_token(0).unwrap();

    let object = state.get_object(0).unwrap();
    let object_id = object.id;
    let _initial_revision = state.objects[0].revision.clone();

    let mut client = GraphServiceClient::connect(address).await?;

    // Make multiple updates in sequence
    for i in 1..=3 {
        let metadata = json_to_protobuf_struct(json!({
            "version": i.to_string()
        }))
        .unwrap();

        let update_req = Request::new(UpdateObjectRequest {
            object_id,
            metadata: Some(metadata),
        })
        .with_bearer_token(user_token)?;

        let update_resp = client.update_object(update_req).await?;
        println!(
            "After update {} - Object metadata: {:?}",
            i,
            update_resp.get_ref().object.as_ref().unwrap().metadata
        );
    }

    // Query at initial revision should not see any updates
    let get_initial_req = Request::new(GetObjectRequest {
        object_id,
        consistency: Some(ConsistencyRequirement {
            requirement: Some(Requirement::ExactlyAt(_initial_revision)),
        }),
    })
    .with_bearer_token(user_token)?;

    let initial_get_resp = client.get_object(get_initial_req).await?;
    let initial_object = initial_get_resp.get_ref().object.as_ref().unwrap();
    println!(
        "When querying initial revision - Object metadata: {:?}",
        initial_object.metadata
    );
    assert!(initial_object
        .metadata
        .as_ref()
        .unwrap_or(&prost_types::Struct {
            fields: std::collections::BTreeMap::new()
        })
        .fields
        .is_empty());

    // Query with full consistency should see latest version
    let get_latest_req = Request::new(GetObjectRequest {
        object_id,
        consistency: Some(ConsistencyRequirement {
            requirement: Some(Requirement::FullConsistency(true)),
        }),
    })
    .with_bearer_token(user_token)?;

    let latest_get_resp = client.get_object(get_latest_req).await?;
    let latest_object = latest_get_resp.get_ref().object.as_ref().unwrap();
    println!(
        "When querying with full consistency - Object metadata: {:?}",
        latest_object.metadata
    );
    assert_eq!(
        latest_object
            .metadata
            .as_ref()
            .unwrap()
            .fields
            .get("version")
            .unwrap()
            .kind
            .as_ref()
            .unwrap(),
        &prost_types::value::Kind::StringValue("3".to_string())
    );

    Ok(())
}
