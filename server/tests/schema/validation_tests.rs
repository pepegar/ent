use crate::test_helper::EntTestBuilder;
use anyhow::Result;
use serde_json::json;

#[tokio::test]
async fn test_schema_validation_comprehensive() -> Result<()> {
    let (address, _pool, _container) = crate::common::spawn_app().await?;

    let builder = EntTestBuilder::new()
        .with_schema_and_type(
            r#"{
            "type": "object",
            "required": ["username", "email", "address"],
            "properties": {
                "username": { 
                    "type": "string",
                    "minLength": 3,
                    "maxLength": 50
                },
                "email": {
                    "type": "string",
                    "format": "email"
                },
                "age": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 150
                },
                "tags": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    },
                    "maxItems": 10
                },
                "address": {
                    "type": "object",
                    "required": ["street", "city"],
                    "properties": {
                        "street": { "type": "string" },
                        "city": { "type": "string" },
                        "zipcode": { "type": "string" }
                    }
                }
            }
        }"#,
            "user_profile",
        )
        .with_user("test_user");

    // Test 1: Valid object with all required fields
    let state = builder
        .clone()
        .with_object(
            0,
            "user_profile",
            json!({
                "username": "johndoe",
                "email": "john@example.com",
                "address": {
                    "street": "123 Main St",
                    "city": "Springfield"
                },
                "tags": ["developer", "rust"]
            }),
        )
        .build(address.clone())
        .await?;

    assert!(state.get_object(0).is_some());

    // Test 2: Invalid object - missing required field (address)
    let result = builder
        .try_create_object(
            address.clone(),
            0,
            "user_profile",
            json!({
                "username": "johndoe",
                "email": "john@example.com"
            }),
        )
        .await;
    assert!(result.is_err(), "Expected error for missing required field");

    // Test 3: Invalid object - wrong type for nested object
    let result = builder
        .try_create_object(
            address.clone(),
            0,
            "user_profile",
            json!({
                "username": "johndoe",
                "email": "john@example.com",
                "address": "not an object"
            }),
        )
        .await;
    assert!(
        result.is_err(),
        "Expected error for invalid nested object type"
    );

    // Test 4: Invalid object - array with too many items
    let mut tags = Vec::new();
    for i in 0..15 {
        tags.push(format!("tag{}", i));
    }
    let result = builder
        .try_create_object(
            address.clone(),
            0,
            "user_profile",
            json!({
                "username": "johndoe",
                "email": "john@example.com",
                "address": {
                    "street": "123 Main St",
                    "city": "Springfield"
                },
                "tags": tags
            }),
        )
        .await;
    assert!(
        result.is_err(),
        "Expected error for too many items in array"
    );

    Ok(())
}

// ... rest of the existing tests ...
