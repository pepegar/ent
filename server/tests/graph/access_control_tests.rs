use anyhow::Result;
use ent_proto::ent::{
    graph_service_client::GraphServiceClient, ConsistencyRequirement, GetObjectRequest,
    UpdateObjectRequest,
};
use ent_server::auth::RequestExt;
use serde_json::json;
use tonic::Status;

use crate::{
    common::spawn_app,
    test_helper::{json_to_protobuf_struct, EntTestBuilder},
};

#[tokio::test]
async fn test_object_ownership_access_control() -> Result<()> {
    // Spawn a test app instance
    let (addr, _pool, _pg) = spawn_app().await?;

    // Create test state with two users and objects
    let test_state = EntTestBuilder::new()
        .with_basic_schema()
        .with_user("user1")
        .with_user("user2")
        .with_object(
            0,
            "basic",
            json!({
                "name": "user1's object",
                "description": "This belongs to user1"
            }),
        )
        .with_object(
            1,
            "basic",
            json!({
                "name": "user2's object",
                "description": "This belongs to user2"
            }),
        )
        .build(addr.clone())
        .await?;

    let mut client = GraphServiceClient::connect(addr).await?;

    // Test: User1 trying to access User2's object
    let user1_token = test_state.get_user_token(0).unwrap();
    let user2_object = test_state.get_object(1).unwrap();

    let request = tonic::Request::new(GetObjectRequest {
        object_id: user2_object.id.clone(),
        consistency: Some(ConsistencyRequirement {
            requirement: Some(
                ent_proto::ent::consistency_requirement::Requirement::FullConsistency(true),
            ),
        }),
    })
    .with_bearer_token(user1_token)?;

    // This should fail with a permission denied error
    let response = client.get_object(request).await;
    assert!(response.is_err());
    assert_eq!(response.unwrap_err().code(), tonic::Code::PermissionDenied);

    // Test: User1 trying to update User2's object
    let update_request = tonic::Request::new(UpdateObjectRequest {
        object_id: user2_object.id.clone(),
        metadata: json_to_protobuf_struct(json!({
            "name": "attempted modification",
        })),
    })
    .with_bearer_token(user1_token)?;

    // This should also fail with a permission denied error
    let update_response = client.update_object(update_request).await;
    assert!(update_response.is_err());
    assert_eq!(
        update_response.unwrap_err().code(),
        tonic::Code::PermissionDenied
    );

    // Test: User2 accessing their own object (should succeed)
    let user2_token = test_state.get_user_token(1).unwrap();
    let owner_request = tonic::Request::new(GetObjectRequest {
        object_id: user2_object.id.clone(),
        consistency: Some(ConsistencyRequirement {
            requirement: Some(
                ent_proto::ent::consistency_requirement::Requirement::FullConsistency(true),
            ),
        }),
    })
    .with_bearer_token(user2_token)?;

    let owner_response = client.get_object(owner_request).await;
    assert!(owner_response.is_ok());

    Ok(())
}
