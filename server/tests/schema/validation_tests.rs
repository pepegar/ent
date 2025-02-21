use anyhow::Result;
use ent_proto::ent::{
    graph_service_client::GraphServiceClient, schema_service_client::SchemaServiceClient,
    CreateObjectRequest, CreateSchemaRequest,
};
use prost_types::{Struct, Value};
use std::collections::BTreeMap;

use crate::jwt;

#[tokio::test]
async fn test_schema_validation_comprehensive() -> Result<()> {
    let (address, _pool, _container) = crate::common::spawn_app().await?;

    // Create schema client
    let mut schema_client = SchemaServiceClient::connect(address.clone()).await?;
    let mut graph_client = GraphServiceClient::connect(address).await?;

    // Create a test user token
    let user_token = jwt::generate_test_token("test_user")?;

    // Create a complex schema with nested objects and arrays
    let schema_request = tonic::Request::new(CreateSchemaRequest {
        type_name: "user_profile".to_string(),
        description: "User profile schema with nested objects and arrays".to_string(),
        schema: r#"{
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
        }"#
        .to_string(),
    });
    schema_client.create_schema(schema_request).await?;

    // Test 1: Valid object with all required fields
    let valid_metadata = {
        let mut fields = BTreeMap::new();
        fields.insert(
            "username".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::StringValue("johndoe".to_string())),
            },
        );
        fields.insert(
            "email".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "john@example.com".to_string(),
                )),
            },
        );

        // Nested address object
        let mut address_fields = BTreeMap::new();
        address_fields.insert(
            "street".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "123 Main St".to_string(),
                )),
            },
        );
        address_fields.insert(
            "city".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "Springfield".to_string(),
                )),
            },
        );
        fields.insert(
            "address".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::StructValue(Struct {
                    fields: address_fields,
                })),
            },
        );

        // Optional array of tags
        let tags = vec![
            Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "developer".to_string(),
                )),
            },
            Value {
                kind: Some(prost_types::value::Kind::StringValue("rust".to_string())),
            },
        ];
        fields.insert(
            "tags".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::ListValue(
                    prost_types::ListValue { values: tags },
                )),
            },
        );

        Struct { fields }
    };

    let mut valid_request = tonic::Request::new(CreateObjectRequest {
        r#type: "user_profile".to_string(),
        metadata: Some(valid_metadata),
    });
    valid_request
        .metadata_mut()
        .insert("authorization", format!("Bearer {}", user_token).parse()?);

    // This should succeed
    let response = graph_client.create_object(valid_request).await;
    assert!(
        response.is_ok(),
        "Valid object creation failed: {:?}",
        response
    );

    // Test 2: Invalid object - missing required field (address)
    let invalid_metadata = {
        let mut fields = BTreeMap::new();
        fields.insert(
            "username".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::StringValue("johndoe".to_string())),
            },
        );
        fields.insert(
            "email".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "john@example.com".to_string(),
                )),
            },
        );
        Struct { fields }
    };

    let mut invalid_request = tonic::Request::new(CreateObjectRequest {
        r#type: "user_profile".to_string(),
        metadata: Some(invalid_metadata),
    });
    invalid_request
        .metadata_mut()
        .insert("authorization", format!("Bearer {}", user_token).parse()?);

    // This should fail - missing required field
    let response = graph_client.create_object(invalid_request).await;
    assert!(
        response.is_err(),
        "Expected error for missing required field"
    );

    // Test 3: Invalid object - wrong type for nested object
    let invalid_nested_metadata = {
        let mut fields = BTreeMap::new();
        fields.insert(
            "username".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::StringValue("johndoe".to_string())),
            },
        );
        fields.insert(
            "email".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "john@example.com".to_string(),
                )),
            },
        );
        fields.insert(
            "address".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "not an object".to_string(),
                )),
            },
        );
        Struct { fields }
    };

    let mut invalid_nested_request = tonic::Request::new(CreateObjectRequest {
        r#type: "user_profile".to_string(),
        metadata: Some(invalid_nested_metadata),
    });
    invalid_nested_request
        .metadata_mut()
        .insert("authorization", format!("Bearer {}", user_token).parse()?);

    // This should fail - wrong type for nested object
    let response = graph_client.create_object(invalid_nested_request).await;
    assert!(
        response.is_err(),
        "Expected error for invalid nested object type"
    );

    // Test 4: Invalid object - array with too many items
    let invalid_array_metadata = {
        let mut fields = BTreeMap::new();
        fields.insert(
            "username".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::StringValue("johndoe".to_string())),
            },
        );
        fields.insert(
            "email".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "john@example.com".to_string(),
                )),
            },
        );

        // Valid address
        let mut address_fields = BTreeMap::new();
        address_fields.insert(
            "street".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "123 Main St".to_string(),
                )),
            },
        );
        address_fields.insert(
            "city".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "Springfield".to_string(),
                )),
            },
        );
        fields.insert(
            "address".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::StructValue(Struct {
                    fields: address_fields,
                })),
            },
        );

        // Too many tags
        let tags = (0..15)
            .map(|i| Value {
                kind: Some(prost_types::value::Kind::StringValue(format!("tag{}", i))),
            })
            .collect();

        fields.insert(
            "tags".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::ListValue(
                    prost_types::ListValue { values: tags },
                )),
            },
        );

        Struct { fields }
    };

    let mut invalid_array_request = tonic::Request::new(CreateObjectRequest {
        r#type: "user_profile".to_string(),
        metadata: Some(invalid_array_metadata),
    });
    invalid_array_request
        .metadata_mut()
        .insert("authorization", format!("Bearer {}", user_token).parse()?);

    // This should fail - too many items in array
    let response = graph_client.create_object(invalid_array_request).await;
    assert!(
        response.is_err(),
        "Expected error for too many items in array"
    );

    Ok(())
}

#[tokio::test]
async fn test_schema_validation_additional_cases() -> Result<()> {
    let (address, _pool, _container) = crate::common::spawn_app().await?;

    let mut schema_client = SchemaServiceClient::connect(address.clone()).await?;
    let mut graph_client = GraphServiceClient::connect(address).await?;

    // Create a test user token
    let user_token = jwt::generate_test_token("test_user")?;

    // Create a schema with various validation rules
    let schema_request = tonic::Request::new(CreateSchemaRequest {
        type_name: "product".to_string(),
        description: "Product schema with various validation rules".to_string(),
        schema: r#"{
            "type": "object",
            "required": ["name", "price", "category"],
            "additionalProperties": false,
            "properties": {
                "name": { 
                    "type": "string",
                    "pattern": "^[A-Za-z0-9\\s-]+$"
                },
                "price": {
                    "type": "number",
                    "minimum": 0,
                    "maximum": 1000000
                },
                "category": {
                    "type": "string",
                    "enum": ["electronics", "books", "clothing"]
                },
                "discount": {
                    "type": "number",
                    "minimum": 0,
                    "maximum": 100
                },
                "salePrice": {
                    "type": "number"
                },
                "description": {
                    "type": "string"
                }
            },
            "dependencies": {
                "discount": ["salePrice"]
            }
        }"#
        .to_string(),
    });
    schema_client.create_schema(schema_request).await?;

    // Test 1: Valid product
    let valid_metadata = {
        let mut fields = BTreeMap::new();
        fields.insert(
            "name".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "Laptop Pro-2024".to_string(),
                )),
            },
        );
        fields.insert(
            "price".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::NumberValue(999.99)),
            },
        );
        fields.insert(
            "category".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "electronics".to_string(),
                )),
            },
        );
        Struct { fields }
    };

    let mut request = tonic::Request::new(CreateObjectRequest {
        r#type: "product".to_string(),
        metadata: Some(valid_metadata),
    });
    request
        .metadata_mut()
        .insert("authorization", format!("Bearer {}", user_token).parse()?);

    let response = graph_client.create_object(request).await;
    assert!(response.is_ok(), "Valid product creation failed");

    // Test 2: Invalid product - price out of range
    let invalid_price_metadata = {
        let mut fields = BTreeMap::new();
        fields.insert(
            "name".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::StringValue("Laptop".to_string())),
            },
        );
        fields.insert(
            "price".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::NumberValue(-10.0)),
            },
        );
        fields.insert(
            "category".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "electronics".to_string(),
                )),
            },
        );
        Struct { fields }
    };

    let mut request = tonic::Request::new(CreateObjectRequest {
        r#type: "product".to_string(),
        metadata: Some(invalid_price_metadata),
    });
    request
        .metadata_mut()
        .insert("authorization", format!("Bearer {}", user_token).parse()?);

    let response = graph_client.create_object(request).await;
    assert!(response.is_err(), "Expected error for invalid price");

    // Test 3: Invalid product - invalid category enum value
    let invalid_category_metadata = {
        let mut fields = BTreeMap::new();
        fields.insert(
            "name".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::StringValue("Book".to_string())),
            },
        );
        fields.insert(
            "price".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::NumberValue(29.99)),
            },
        );
        fields.insert(
            "category".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::StringValue("food".to_string())),
            },
        );
        Struct { fields }
    };

    let mut request = tonic::Request::new(CreateObjectRequest {
        r#type: "product".to_string(),
        metadata: Some(invalid_category_metadata),
    });
    request
        .metadata_mut()
        .insert("authorization", format!("Bearer {}", user_token).parse()?);

    let response = graph_client.create_object(request).await;
    assert!(response.is_err(), "Expected error for invalid category");

    // Test 4: Invalid product - additional property not allowed
    let additional_prop_metadata = {
        let mut fields = BTreeMap::new();
        fields.insert(
            "name".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::StringValue("T-Shirt".to_string())),
            },
        );
        fields.insert(
            "price".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::NumberValue(19.99)),
            },
        );
        fields.insert(
            "category".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "clothing".to_string(),
                )),
            },
        );
        fields.insert(
            "color".to_string(), // Not in schema
            Value {
                kind: Some(prost_types::value::Kind::StringValue("blue".to_string())),
            },
        );
        Struct { fields }
    };

    let mut request = tonic::Request::new(CreateObjectRequest {
        r#type: "product".to_string(),
        metadata: Some(additional_prop_metadata),
    });
    request
        .metadata_mut()
        .insert("authorization", format!("Bearer {}", user_token).parse()?);

    let response = graph_client.create_object(request).await;
    assert!(response.is_err(), "Expected error for additional property");

    // Test 5: Invalid product - missing dependent field
    let missing_dependent_metadata = {
        let mut fields = BTreeMap::new();
        fields.insert(
            "name".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::StringValue("Book".to_string())),
            },
        );
        fields.insert(
            "price".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::NumberValue(29.99)),
            },
        );
        fields.insert(
            "category".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::StringValue("books".to_string())),
            },
        );
        fields.insert(
            "discount".to_string(),
            Value {
                kind: Some(prost_types::value::Kind::NumberValue(20.0)),
            },
        );
        // Missing salePrice which is required when discount is present
        Struct { fields }
    };

    let mut request = tonic::Request::new(CreateObjectRequest {
        r#type: "product".to_string(),
        metadata: Some(missing_dependent_metadata),
    });
    request
        .metadata_mut()
        .insert("authorization", format!("Bearer {}", user_token).parse()?);

    let response = graph_client.create_object(request).await;
    assert!(
        response.is_err(),
        "Expected error for missing dependent field"
    );

    Ok(())
}
