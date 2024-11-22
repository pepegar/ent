use anyhow::{anyhow, Result};
use base64::{self, engine::general_purpose::URL_SAFE as base64_url, Engine};
use serde::{Deserialize, Serialize};
use sqlx::postgres::types::Xid8;
use sqlx::types::Json;

/// PostgreSQL snapshot information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgSnapshot {
    xmin: u64,
    xmax: u64,
    xip_list: Vec<u64>,
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
    pub fn to_zookie(&self) -> Result<String> {
        let bytes = serde_json::to_vec(self)?;
        Ok(base64_url.encode(bytes))
    }

    pub fn from_zookie(zookie: &str) -> Result<Self> {
        let bytes = base64_url
            .decode(zookie)
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
}
