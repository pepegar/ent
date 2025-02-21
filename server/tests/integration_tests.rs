use anyhow::Result;
use ent_proto::ent::graph_service_client::GraphServiceClient;
use ent_proto::ent::CreateObjectRequest;
use ent_proto::ent::{schema_service_client::SchemaServiceClient, CreateSchemaRequest};
use ent_server::config::Settings;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use test_helper::EntTestBuilder;
use testcontainers::{clients::Cli, Container, GenericImage};
use tokio::net::TcpListener;
use tonic::transport::Server;
use uuid::Uuid;

mod jwt;
mod test_helper;

/// Test utilities and fixtures
mod common {
    use super::*;
    use ent_proto::ent::{
        graph_service_server::GraphServiceServer, schema_service_server::SchemaServiceServer,
    };
    use ent_server::{GraphServer, SchemaServer};
    use once_cell::sync::Lazy;
    use sqlx::{Pool, Postgres as SqlxPostgres};
    use std::sync::Mutex;
    use tracing::info;
    use tracing_subscriber::fmt::format::FmtSpan;

    // Ensure migrations are run only once
    static MIGRATIONS_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
    // Create a single Docker client for all tests
    static DOCKER: Lazy<Cli> = Lazy::new(Cli::default);

    // Wrapper struct to ensure container cleanup
    pub struct PostgresContainer<'a> {
        container: Container<'a, GenericImage>,
        port: u16,
    }

    impl<'a> PostgresContainer<'a> {
        pub fn new() -> Self {
            let postgres_image = GenericImage::new("postgres", "15-alpine")
                .with_env_var("POSTGRES_USER", "postgres")
                .with_env_var("POSTGRES_PASSWORD", "postgres")
                .with_env_var("POSTGRES_DB", "postgres")
                .with_wait_for(testcontainers::core::WaitFor::message_on_stderr(
                    "database system is ready to accept connections",
                ));

            let container = DOCKER.run(postgres_image);
            let port = container.get_host_port_ipv4(5432);

            Self { container, port }
        }

        pub fn port(&self) -> u16 {
            self.port
        }
    }

    impl<'a> Drop for PostgresContainer<'a> {
        fn drop(&mut self) {
            info!("Cleaning up Postgres container {}", self.container.id());
            // The container will be automatically removed when dropped
        }
    }

    pub async fn setup_test_db() -> Result<(Pool<SqlxPostgres>, PostgresContainer<'static>)> {
        let _lock = MIGRATIONS_LOCK.lock().unwrap();

        // Start a Postgres container
        let container = PostgresContainer::new();
        let connection_string = format!(
            "postgres://postgres:postgres@localhost:{}/postgres",
            container.port()
        );

        // Create the test database
        let admin_pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&connection_string)
            .await?;

        // Create the ent user and grant privileges
        sqlx::query(
            r#"
            DO $$
            BEGIN
                IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'ent') THEN
                    CREATE USER ent WITH PASSWORD 'ent_password' SUPERUSER CREATEDB;
                END IF;
            END
            $$;
            "#,
        )
        .execute(&admin_pool)
        .await?;

        // Create a unique test database name
        let test_db_name = format!("ent_test_{}", Uuid::new_v4().simple());
        sqlx::query(&format!("CREATE DATABASE {}", test_db_name))
            .execute(&admin_pool)
            .await?;

        // Grant privileges to ent user
        sqlx::query(&format!(
            "GRANT ALL PRIVILEGES ON DATABASE {} TO ent",
            test_db_name
        ))
        .execute(&admin_pool)
        .await?;

        // Connect to the new test database as ent user
        let test_db_url = format!(
            "postgres://ent:ent_password@localhost:{}/{}",
            container.port(),
            test_db_name
        );
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&test_db_url)
            .await?;

        // Run migrations
        sqlx::migrate!("../migrations").run(&pool).await?;

        Ok((pool, container))
    }

    pub async fn get_test_server_address() -> Result<SocketAddr> {
        // Find a random available port
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        // Close the listener - the actual server will bind to this address
        drop(listener);
        Ok(addr)
    }

    pub async fn spawn_app() -> Result<(String, Pool<SqlxPostgres>, PostgresContainer<'static>)> {
        let _subscriber = tracing_subscriber::fmt()
            .with_span_events(FmtSpan::FULL)
            .with_test_writer()
            .try_init();

        let (pool, container) = setup_test_db().await?;
        let addr = get_test_server_address().await?;

        // Create test settings
        let mut settings = Settings::new_from_folder("..".to_string())?;
        settings.server.host = addr.ip().to_string();
        settings.server.port = addr.port();

        // Initialize JWT validator with test keys
        let public_key = std::fs::read_to_string("../test/data/public.pem")?;
        ent_server::auth::JwtValidator::init(&public_key, "ent".to_string())?;

        // Clone pool for the server
        let schema_pool = pool.clone();
        let graph_pool = pool.clone();

        // Spawn the server in the background
        tokio::spawn(async move {
            let schema_server = SchemaServer::new(schema_pool);
            let graph_server = GraphServer::new(graph_pool);

            Server::builder()
                .add_service(SchemaServiceServer::new(schema_server))
                .add_service(GraphServiceServer::new(graph_server))
                .serve(addr)
                .await
                .expect("Failed to start test server");
        });

        // Allow the server some time to start
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        info!("Test server ready");

        Ok((format!("http://{}", addr), pool, container))
    }
}

#[tokio::test]
async fn test_create_schema() -> Result<()> {
    // Spawn a new instance of the application
    let (address, _pool, _container) = common::spawn_app().await?;

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
    let (address, _pool, _container) = common::spawn_app().await?;
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

#[tokio::test]
async fn test_complex_scenario() -> Result<()> {
    let (address, _pool, _container) = common::spawn_app().await?;

    let type_name = format!("test_type_{}", Uuid::new_v4());

    let state = EntTestBuilder::new()
        .with_schema(
            r#"{
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            }
        }"#,
        )
        .with_user("user1")
        .with_user("user2")
        .with_object(
            0,
            &type_name,
            json!({
                "name": "Doc 1"
            }),
        )
        .with_object(
            1,
            &type_name,
            json!({
                "name": "Doc 2"
            }),
        )
        .with_edge(
            0,
            0,
            1,
            "references",
            json!({
                "note": "Important reference"
            }),
        )
        .build(address)
        .await?;

    assert!(state.get_object(0).is_some());
    assert!(state.get_user_token(0).is_some());

    Ok(())
}

#[tokio::test]
async fn test_schema_validation_comprehensive() -> Result<()> {
    let (address, _pool, _container) = common::spawn_app().await?;

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
        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "username".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue("johndoe".to_string())),
            },
        );
        fields.insert(
            "email".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "john@example.com".to_string(),
                )),
            },
        );

        // Nested address object
        let mut address_fields = std::collections::BTreeMap::new();
        address_fields.insert(
            "street".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "123 Main St".to_string(),
                )),
            },
        );
        address_fields.insert(
            "city".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "Springfield".to_string(),
                )),
            },
        );
        fields.insert(
            "address".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StructValue(prost_types::Struct {
                    fields: address_fields,
                })),
            },
        );

        // Optional array of tags
        let tags = vec![
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "developer".to_string(),
                )),
            },
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue("rust".to_string())),
            },
        ];
        fields.insert(
            "tags".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::ListValue(
                    prost_types::ListValue { values: tags },
                )),
            },
        );

        prost_types::Struct { fields }
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
        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "username".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue("johndoe".to_string())),
            },
        );
        fields.insert(
            "email".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "john@example.com".to_string(),
                )),
            },
        );
        prost_types::Struct { fields }
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
        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "username".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue("johndoe".to_string())),
            },
        );
        fields.insert(
            "email".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "john@example.com".to_string(),
                )),
            },
        );
        fields.insert(
            "address".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "not an object".to_string(),
                )),
            },
        );
        prost_types::Struct { fields }
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
        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "username".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue("johndoe".to_string())),
            },
        );
        fields.insert(
            "email".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "john@example.com".to_string(),
                )),
            },
        );

        // Valid address
        let mut address_fields = std::collections::BTreeMap::new();
        address_fields.insert(
            "street".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "123 Main St".to_string(),
                )),
            },
        );
        address_fields.insert(
            "city".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "Springfield".to_string(),
                )),
            },
        );
        fields.insert(
            "address".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StructValue(prost_types::Struct {
                    fields: address_fields,
                })),
            },
        );

        // Too many tags
        let tags = (0..15)
            .map(|i| prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue(format!("tag{}", i))),
            })
            .collect();

        fields.insert(
            "tags".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::ListValue(
                    prost_types::ListValue { values: tags },
                )),
            },
        );

        prost_types::Struct { fields }
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
    let (address, _pool, _container) = common::spawn_app().await?;

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
        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "name".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "Laptop Pro-2024".to_string(),
                )),
            },
        );
        fields.insert(
            "price".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::NumberValue(999.99)),
            },
        );
        fields.insert(
            "category".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "electronics".to_string(),
                )),
            },
        );
        prost_types::Struct { fields }
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
        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "name".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue("Laptop".to_string())),
            },
        );
        fields.insert(
            "price".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::NumberValue(-10.0)),
            },
        );
        fields.insert(
            "category".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "electronics".to_string(),
                )),
            },
        );
        prost_types::Struct { fields }
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
        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "name".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue("Book".to_string())),
            },
        );
        fields.insert(
            "price".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::NumberValue(29.99)),
            },
        );
        fields.insert(
            "category".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue("food".to_string())),
            },
        );
        prost_types::Struct { fields }
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
        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "name".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue("T-Shirt".to_string())),
            },
        );
        fields.insert(
            "price".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::NumberValue(19.99)),
            },
        );
        fields.insert(
            "category".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue(
                    "clothing".to_string(),
                )),
            },
        );
        fields.insert(
            "color".to_string(), // Not in schema
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue("blue".to_string())),
            },
        );
        prost_types::Struct { fields }
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
        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "name".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue("Book".to_string())),
            },
        );
        fields.insert(
            "price".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::NumberValue(29.99)),
            },
        );
        fields.insert(
            "category".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue("books".to_string())),
            },
        );
        fields.insert(
            "discount".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::NumberValue(20.0)),
            },
        );
        // Missing salePrice which is required when discount is present
        prost_types::Struct { fields }
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
