use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    str::FromStr,
};

use super::xid::Xid8;
use anyhow::{anyhow, Result};
use base64::{self, engine::general_purpose::URL_SAFE as base64_url, Engine};
use ent_proto::ent::Zookie;
use serde::{Deserialize, Serialize};
use sqlx::{
    encode::IsNull,
    error::BoxDynError,
    postgres::{PgArgumentBuffer, PgTypeInfo, PgValueRef},
    types::Json,
    Decode, Encode, Type,
};

#[derive(Debug)]
pub struct SnapshotError(String);

impl Display for SnapshotError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Snapshot error: {}", self.0)
    }
}

impl std::error::Error for SnapshotError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgSnapshot {
    xmin: u64,
    xmax: u64,
    xip_list: Vec<u64>,
}

impl FromStr for PgSnapshot {
    type Err = SnapshotError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 3 {
            return Err(SnapshotError(format!("Invalid snapshot format {:?}", s)));
        }

        let xmin = parts[0]
            .parse::<u64>()
            .map_err(|e| SnapshotError(format!("Invalid xmin: {}", e)))?;
        let xmax = parts[1]
            .parse::<u64>()
            .map_err(|e| SnapshotError(format!("Invalid xmax: {}", e)))?;

        let xip_list = if parts[2].is_empty() {
            Vec::new()
        } else {
            parts[2]
                .split(',')
                .map(|s| {
                    s.parse::<u64>()
                        .map_err(|e| SnapshotError(format!("Invalid xip value: {}", e)))
                })
                .collect::<Result<Vec<_>, _>>()?
        };

        Ok(PgSnapshot {
            xmin,
            xmax,
            xip_list,
        })
    }
}

impl ToString for PgSnapshot {
    fn to_string(&self) -> String {
        if self.xip_list.is_empty() {
            format!("{}:{}:", self.xmin, self.xmax)
        } else {
            format!(
                "{}:{}:{}",
                self.xmin,
                self.xmax,
                self.xip_list
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
    }
}

impl Type<sqlx::Postgres> for PgSnapshot {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::with_name("pg_snapshot")
    }
}

impl<'r> Decode<'r, sqlx::Postgres> for PgSnapshot {
    fn decode(value: PgValueRef<'r>) -> Result<Self, BoxDynError> {
        let snapshot_str = <&str as Decode<sqlx::Postgres>>::decode(value)?;
        PgSnapshot::from_str(snapshot_str).map_err(|e| Box::new(e) as BoxDynError)
    }
}

impl Encode<'_, sqlx::Postgres> for PgSnapshot {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> IsNull {
        let snapshot_str = self.to_string();
        Encode::<sqlx::Postgres>::encode_by_ref(&snapshot_str, buf)
    }
}

impl PgSnapshot {
    pub fn is_visible(&self, xid: u64) -> bool {
        if xid < self.xmin {
            return true;
        }
        if xid >= self.xmax {
            return false;
        }
        !self.xip_list.binary_search(&xid).is_ok()
    }

    pub fn mark_complete(mut self, xid: u64) -> Self {
        if xid >= self.xmax {
            self.xmax = xid + 1;
        }
        if let Ok(pos) = self.xip_list.binary_search(&xid) {
            self.xip_list.remove(pos);
        }
        if self.xip_list.is_empty() {
            self.xmin = self.xmax;
        }
        self
    }
}

/// Internal revision representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Revision {
    snapshot: PgSnapshot,
    optional_xid: Option<u64>,
}

impl Revision {
    pub fn to_zookie(&self) -> Result<Zookie> {
        let bytes = serde_json::to_vec(self)?;
        Ok(Zookie {
            value: base64_url.encode(bytes),
        })
    }

    pub fn from_zookie(zookie: Zookie) -> Result<Self> {
        let bytes = base64_url
            .decode(zookie.value.as_bytes())
            .map_err(|_| anyhow!("Invalid zookie encoding"))?;

        serde_json::from_slice(&bytes).map_err(|_| anyhow!("Invalid zookie format"))
    }

    pub fn greater_than(&self, other: &Self) -> bool {
        // A revision is greater if it can see transactions the other can't
        self.snapshot.xmax > other.snapshot.xmax
    }
}

/// Consistency mode for queries
#[derive(Debug, Clone)]
pub enum ConsistencyMode {
    Full,
    AtLeastAsFresh(Revision),
    ExactlyAt(Revision),
    MinimizeLatency,
}

#[derive(Debug)]
pub struct Transaction {
    pub xid: Xid8,
    pub snapshot: PgSnapshot,
    pub metadata: Option<Json<serde_json::Value>>,
}

impl Transaction {
    pub fn revision(&self) -> Revision {
        Revision {
            snapshot: self.snapshot.clone(),
            optional_xid: Some(self.xid.value()),
        }
    }

    pub async fn create(
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<Transaction> {
        let row = sqlx::query!(
            r#"
            INSERT INTO relation_tuple_transaction DEFAULT VALUES 
            RETURNING
                    xid as "xid!: Xid8",
                    snapshot::text as "snapshot!: PgSnapshot",
                    metadata as "metadata: Json<serde_json::Value>"
            "#
        )
        .fetch_one(&mut **transaction)
        .await?;

        Ok(Transaction {
            xid: row.xid,
            snapshot: row.snapshot,
            metadata: Some(row.metadata),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_parsing() {
        // Test empty transaction list
        let snapshot = PgSnapshot::from_str("100:100:").unwrap();
        assert_eq!(snapshot.xmin, 100);
        assert_eq!(snapshot.xmax, 100);
        assert!(snapshot.xip_list.is_empty());

        // Test with in-progress transactions
        let snapshot = PgSnapshot::from_str("100:105:101,102,103").unwrap();
        assert_eq!(snapshot.xmin, 100);
        assert_eq!(snapshot.xmax, 105);
        assert_eq!(snapshot.xip_list, vec![101, 102, 103]);

        // Test error cases
        let err = PgSnapshot::from_str("invalid").unwrap_err();
        assert!(err.to_string().contains("Invalid snapshot format"));

        let err = PgSnapshot::from_str("a:b:c").unwrap_err();
        assert!(err.to_string().contains("Invalid xmin"));
    }

    #[test]
    fn test_snapshot_to_string() {
        let snapshot = PgSnapshot {
            xmin: 100,
            xmax: 105,
            xip_list: vec![101, 102, 103],
        };
        assert_eq!(snapshot.to_string(), "100:105:101,102,103");

        let snapshot = PgSnapshot {
            xmin: 100,
            xmax: 100,
            xip_list: vec![],
        };
        assert_eq!(snapshot.to_string(), "100:100:");
    }
}
