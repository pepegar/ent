use anyhow::Result;
use ent_proto::ent::{schema_service_client::SchemaServiceClient, CreateSchemaRequest};
use ent_server::config::Settings;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use test_helper::EntTestBuilder;
use tokio::net::TcpListener;
use tonic::transport::Server;
use uuid::Uuid;

mod test_helper;

/// Test utilities and fixtures
mod common {
    use super::*;
    use ent_proto::ent::{
        graph_service_server::GraphServiceServer, schema_service_server::SchemaServiceServer,
    };
    use ent_server::{GraphServer, SchemaServer};
    use once_cell::sync::Lazy;
    use sqlx::{Pool, Postgres};
    use std::sync::Mutex;
    use tracing::info;

    // Ensure migrations are run only once
    static MIGRATIONS_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    pub async fn setup_test_db() -> Result<Pool<Postgres>> {
        let _lock = MIGRATIONS_LOCK.lock().unwrap();

        // Create a unique test database name
        let test_db_name = format!("ent_test_{}", Uuid::new_v4().simple());
        let admin_db_url = "postgres://ent:ent_password@localhost:5432/postgres";

        // Connect to the default postgres database to create our test database
        let admin_pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(admin_db_url)
            .await?;

        // Create the test database
        sqlx::query(&format!("CREATE DATABASE {}", test_db_name))
            .execute(&admin_pool)
            .await?;

        // Connect to the new test database
        let test_db_url = format!(
            "postgres://ent:ent_password@localhost:5432/{}",
            test_db_name
        );
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&test_db_url)
            .await?;

        // Run migrations
        sqlx::migrate!("../migrations").run(&pool).await?;

        Ok(pool)
    }

    pub async fn get_test_server_address() -> Result<SocketAddr> {
        // Find a random available port
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        // Close the listener - the actual server will bind to this address
        drop(listener);
        Ok(addr)
    }

    pub async fn spawn_app() -> Result<(String, Pool<Postgres>)> {
        let pool = setup_test_db().await?;
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

        Ok((format!("http://{}", addr), pool))
    }
}

#[tokio::test]
async fn test_create_schema() -> Result<()> {
    // Spawn a new instance of the application
    let (address, _pool) = common::spawn_app().await?;

    // Create a gRPC client
    let mut client = SchemaServiceClient::connect(address).await?;

    // Create a test schema
    let request = tonic::Request::new(CreateSchemaRequest {
        description: "Test schema".to_string(),
        schema: r#"{
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "age": { "type": "number" }
            }
        }"#
        .to_string(),
    });

    let response = client.create_schema(request).await;

    // Assert on the response
    assert!(response.is_ok());

    Ok(())
}

#[tokio::test]
async fn test_invalid_schema() -> Result<()> {
    let (address, _pool) = common::spawn_app().await?;
    let mut client = SchemaServiceClient::connect(address).await?;

    // Try to create an invalid schema
    let request = tonic::Request::new(CreateSchemaRequest {
        description: "Invalid schema".to_string(),
        schema: r#"{ invalid json }"#.to_string(),
    });

    // Should return an error
    let response = client.create_schema(request).await;
    assert!(response.is_err());

    Ok(())
}

#[tokio::test]
async fn test_complex_scenario() -> Result<()> {
    let (address, _pool) = common::spawn_app().await?;

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
            "document",
            json!({
                "name": "Doc 1"
            }),
        )
        .with_object(
            1,
            "document",
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
