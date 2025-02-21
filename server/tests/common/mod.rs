use anyhow::Result;
use ent_proto::ent::{
    graph_service_server::GraphServiceServer, schema_service_server::SchemaServiceServer,
};
use ent_server::{config::Settings, GraphServer, SchemaServer};
use once_cell::sync::Lazy;
use sqlx::{postgres::PgPoolOptions, Pool, Postgres as SqlxPostgres};
use std::{net::SocketAddr, sync::Mutex};
use testcontainers::{clients::Cli, Container, GenericImage};
use tokio::net::TcpListener;
use tonic::transport::Server;
use tracing::info;
use tracing_subscriber::fmt::format::FmtSpan;
use uuid::Uuid;

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
