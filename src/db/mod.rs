use anyhow::Result;
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, instrument, warn};

// Export the schema module
pub mod schema;

pub struct Database {
    pool: PgPool,
}

impl Database {
    #[instrument(skip(database_url))]
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = Self::create_pool_with_retry(database_url).await?;
        Ok(Self { pool })
    }

    async fn create_pool_with_retry(database_url: &str) -> Result<PgPool> {
        let mut retry_count = 0;
        let max_retries = 5;
        let retry_delay = Duration::from_secs(5);

        loop {
            match PgPoolOptions::new()
                .max_connections(5)
                .acquire_timeout(Duration::from_secs(3))
                .connect(database_url)
                .await
            {
                Ok(pool) => {
                    info!("Successfully connected to database");
                    return Ok(pool);
                }
                Err(e) => {
                    retry_count += 1;
                    if retry_count >= max_retries {
                        return Err(e.into());
                    }
                    warn!(
                        "Failed to connect to database, retrying in {} seconds (attempt {}/{})",
                        retry_delay.as_secs(),
                        retry_count,
                        max_retries
                    );
                    sleep(retry_delay).await;
                }
            }
        }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}
