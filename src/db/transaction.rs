use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::postgres::{PgPool, Postgres};
use sqlx::Transaction;
use std::collections::HashMap;
use time::OffsetDateTime;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PgSnapshot {
    pub xmin: u64,
    pub xmax: u64,
    pub xip_list: Vec<u64>,
}

#[derive(Clone, Debug)]
pub struct Revision {
    pub xid: u64,
    pub snapshot: PgSnapshot,
    pub timestamp: OffsetDateTime,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug)]
pub struct TransactionManager {
    pool: PgPool,
}

impl TransactionManager {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn begin_tx(&self) -> Result<Transaction<'static, Postgres>> {
        let tx = self.pool.begin().await?;

        // Create transaction log entry
        sqlx::query!(
            r#"
            INSERT INTO transaction_log DEFAULT VALUES
            RETURNING xid, snapshot, timestamp, metadata
            "#
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(tx)
    }

    pub async fn head_revision(&self) -> Result<Revision> {
        let record = sqlx::query!(
            r#"
            SELECT xid, snapshot, timestamp, metadata 
            FROM transaction_log 
            ORDER BY xid DESC 
            LIMIT 1
            "#
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(Revision {
            xid: record.xid as u64,
            snapshot: serde_json::from_value(record.snapshot)?,
            timestamp: record.timestamp,
            metadata: record
                .metadata
                .as_object()
                .map(|o| o.clone())
                .unwrap_or_default()
                .into_iter()
                .collect(),
        })
    }

    pub async fn revision_at(&self, timestamp: OffsetDateTime) -> Result<Revision> {
        let record = sqlx::query!(
            r#"
            SELECT xid, snapshot, timestamp, metadata 
            FROM transaction_log 
            WHERE timestamp <= $1
            ORDER BY timestamp DESC 
            LIMIT 1
            "#,
            timestamp,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(Revision {
            xid: record.xid as u64,
            snapshot: serde_json::from_value(record.snapshot)?,
            timestamp: record.timestamp,
            metadata: record
                .metadata
                .as_object()
                .map(|o| o.clone())
                .unwrap_or_default()
                .into_iter()
                .collect(),
        })
    }
}
