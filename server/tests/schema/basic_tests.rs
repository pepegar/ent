use anyhow::Result;
use ent_proto::ent::{schema_service_client::SchemaServiceClient, CreateSchemaRequest};
use uuid::Uuid;

#[tokio::test]
async fn test_create_schema() -> Result<()> {
    // Spawn a new instance of the application
    let (address, _pool, _container) = crate::common::spawn_app().await?;

    // Create a gRPC client
    let mut client = SchemaServiceClient::connect(address).await?;

    let type_name = format!("test_type_{}", Uuid::new_v4());

    // Create a test schema
    let request = tonic::Request::new(CreateSchemaRequest {
        schema: r#"{
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "age": { "type": "number" }
            }
        }"#
        .to_string(),
        description: "Test schema".to_string(),
        type_name: type_name,
    });

    let response = client.create_schema(request).await;

    // Assert on the response
    assert!(response.is_ok());

    Ok(())
}

#[tokio::test]
async fn test_invalid_schema() -> Result<()> {
    let (address, _pool, _container) = crate::common::spawn_app().await?;
    let mut client = SchemaServiceClient::connect(address).await?;

    let type_name = format!("test_type_{}", Uuid::new_v4());

    // Try to create an invalid schema
    let request = tonic::Request::new(CreateSchemaRequest {
        schema: r#"{ invalid json }"#.to_string(),
        description: "Invalid schema".to_string(),
        type_name: type_name,
    });

    // Should return an error
    let response = client.create_schema(request).await;
    assert!(response.is_err());

    Ok(())
}
