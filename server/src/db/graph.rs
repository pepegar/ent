use anyhow::{anyhow, Result};
use ent_proto::ent::{
    CreateEdgeRequest, CreateObjectRequest, Edge as ProtoEdge, Object as ProtoObject,
};
use prost_types::Value as ProstValue;
use serde_json::Value;
use sqlx::PgPool;
use time::OffsetDateTime;
use tracing::{info, instrument};

use crate::{
    db::xid::Xid8,
    server::{json_value_to_prost_value, prost_value_to_json_value},
};

use super::transaction::{ConsistencyMode, Revision, Transaction};

#[derive(Debug, sqlx::FromRow)]
pub struct Object {
    pub id: i64,
    pub type_name: String,
    pub metadata: Value,
    pub created_at: Option<OffsetDateTime>,
    pub updated_at: Option<OffsetDateTime>,
}

impl Object {
    pub fn to_pb(&self) -> ProtoObject {
        let json_value = self.metadata.clone();
        ProtoObject {
            id: self.id,
            r#type: self.type_name.clone(),
            metadata: match json_value_to_prost_value(json_value).kind {
                Some(prost_types::value::Kind::StructValue(v)) => Some(v),
                _ => todo!(),
            },
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct Edge {
    pub id: i64,
    pub from_type: String,
    pub from_id: i64,
    pub relation: String,
    pub to_type: String,
    pub to_id: i64,
    pub metadata: Value,
    pub created_at: Option<OffsetDateTime>,
    pub updated_at: Option<OffsetDateTime>,
}

impl Edge {
    pub fn to_pb(&self) -> ProtoEdge {
        let json_value = self.metadata.clone();
        ProtoEdge {
            id: self.id,
            relation: self.relation.clone(),
            from_id: self.from_id,
            from_type: self.from_type.clone(),
            to_id: self.to_id,
            to_type: self.to_type.clone(),
            metadata: match json_value_to_prost_value(json_value).kind {
                Some(prost_types::value::Kind::StructValue(v)) => Some(v),
                _ => None,
            },
        }
    }
}

#[derive(Debug)]
pub struct GraphRepository {
    pool: PgPool,
}

impl GraphRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_object(
        &self,
        user_id: String,
        request: CreateObjectRequest,
    ) -> Result<(Object, Revision)> {
        let metadata: ProstValue = match request.metadata {
            Some(v) => ProstValue {
                kind: Some(prost_types::value::Kind::StructValue(v)),
            },
            None => ProstValue::default(),
        };

        let mut tx = self.pool.begin().await?;
        let transaction = Transaction::create(&mut tx).await?;

        let revision = transaction.revision();

        // Create the object with transaction tracking
        let object = sqlx::query_as!(
            Object,
            r#"
                INSERT INTO objects (
                    type, 
                    metadata, 
                    user_id,
                    created_xid,
                    deleted_xid
                )
                VALUES ($1, $2, $3, $4, $5)
                RETURNING 
                    id, 
                    type as type_name, 
                    metadata as "metadata: Value",
                    created_at as "created_at?: OffsetDateTime",
                    updated_at as "updated_at?: OffsetDateTime"
            "#,
            request.r#type,
            prost_value_to_json_value(metadata),
            user_id,
            transaction.xid as _, // The current transaction's XID
            Xid8::max() as _,     // Max XID value for "not deleted"
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| anyhow!("Failed to create object: {}", e))?;

        info!("Created object: {:?}", object);

        // Commit the transaction
        tx.commit().await?;

        Ok((object, revision))
    }

    pub async fn create_edge(
        &self,
        user_id: String,
        request: CreateEdgeRequest,
    ) -> Result<(Edge, Revision)> {
        let metadata: ProstValue = match request.metadata {
            Some(v) => ProstValue {
                kind: Some(prost_types::value::Kind::StructValue(v)),
            },
            None => ProstValue::default(),
        };

        let mut tx = self.pool.begin().await?;
        let transaction = Transaction::create(&mut tx).await?;

        let revision = transaction.revision();

        // Create the object with transaction tracking
        let object = sqlx::query_as!(
            Edge,
            r#"
                INSERT INTO triples (
                    relation, 
                    metadata, 
                    user_id,
                    from_id,
                    from_type,
                    to_id,
                    to_type,
                    created_xid,
                    deleted_xid
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                RETURNING 
                    id, 
                    from_type,
                    from_id,
                    relation, 
                    to_type,
                    to_id,
                    metadata as "metadata: Value",
                    created_at as "created_at?: OffsetDateTime",
                    updated_at as "updated_at?: OffsetDateTime"
            "#,
            request.relation,
            prost_value_to_json_value(metadata),
            user_id,
            request.from_id,
            request.from_type,
            request.to_id,
            request.to_type,
            transaction.xid as _, // The current transaction's XID
            Xid8::max() as _,     // Max XID value for "not deleted"
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| anyhow!("Failed to create object: {}", e))?;

        info!("Created object: {:?}", object);

        // Commit the transaction
        tx.commit().await?;

        Ok((object, revision))
    }
    #[instrument(skip(self))]
    pub async fn get_object(
        &self,
        id: i64,
        consistency: ConsistencyMode,
    ) -> Result<Option<Object>> {
        match &consistency {
            ConsistencyMode::Full => sqlx::query_as!(
                Object,
                r#"
                    SELECT 
                        id,
                        type as type_name,
                        metadata as "metadata: Value",
                        created_at as "created_at?: OffsetDateTime",
                        updated_at as "updated_at?: OffsetDateTime"
                    FROM objects
                    WHERE id = $1
                    AND created_xid <= pg_current_xact_id()
                    AND deleted_xid > pg_current_xact_id()
                    "#,
                id
            )
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow!("Failed to fetch object: {}", e)),
            ConsistencyMode::MinimizeLatency => sqlx::query_as!(
                Object,
                r#"
                    SELECT 
                        id,
                        type as type_name,
                        metadata as "metadata: Value",
                        created_at as "created_at?: OffsetDateTime",
                        updated_at as "updated_at?: OffsetDateTime"
                    FROM objects
                    WHERE id = $1
                    "#,
                id
            )
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow!("Failed to fetch object: {}", e)),
            ConsistencyMode::AtLeastAsFresh(_revision) | ConsistencyMode::ExactlyAt(_revision) => {
                sqlx::query_as!(
                    Object,
                    r#"
                    WITH snapshot AS (
                        SELECT $2::text::pg_snapshot as snapshot
                    )
                    SELECT 
                        o.id,
                        o.type as type_name,
                        o.metadata as "metadata: Value",
                        o.created_at as "created_at?: OffsetDateTime",
                        o.updated_at as "updated_at?: OffsetDateTime"
                    FROM objects o, snapshot s
                    WHERE o.id = $1
                    AND o.created_xid <= pg_snapshot_xmax(s.snapshot)
                    AND o.deleted_xid > pg_snapshot_xmax(s.snapshot)
                    "#,
                    id,
                    _revision.snapshot_string()
                )
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| anyhow!("Failed to fetch object: {}", e))
            }
        }
    }

    #[instrument(skip(self))]
    pub async fn get_edge(
        &self,
        from_id: i64,
        relation: &str,
        consistency: ConsistencyMode,
    ) -> Result<Option<Edge>> {
        match &consistency {
            ConsistencyMode::Full => sqlx::query_as!(
                Edge,
                r#"
                    SELECT 
                        id,
                        from_type,
                        from_id,
                        relation,
                        to_type,
                        to_id,
                        metadata as "metadata: Value",
                        created_at as "created_at?: OffsetDateTime",
                        updated_at as "updated_at?: OffsetDateTime"
                    FROM triples
                    WHERE from_id = $1 AND relation = $2
                    AND created_xid <= pg_current_xact_id()
                    AND deleted_xid > pg_current_xact_id()
                    LIMIT 1
                    "#,
                from_id,
                relation
            )
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow!("Failed to fetch edge: {}", e)),
            ConsistencyMode::MinimizeLatency => sqlx::query_as!(
                Edge,
                r#"
                    SELECT 
                        id,
                        from_type,
                        from_id,
                        relation,
                        to_type,
                        to_id,
                        metadata as "metadata: Value",
                        created_at as "created_at?: OffsetDateTime",
                        updated_at as "updated_at?: OffsetDateTime"
                    FROM triples
                    WHERE from_id = $1 AND relation = $2
                    LIMIT 1
                    "#,
                from_id,
                relation
            )
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow!("Failed to fetch edge: {}", e)),
            ConsistencyMode::AtLeastAsFresh(_revision) | ConsistencyMode::ExactlyAt(_revision) => {
                sqlx::query_as!(
                    Edge,
                    r#"
                    WITH snapshot AS (
                        SELECT $3::text::pg_snapshot as snapshot
                    )
                    SELECT 
                        t.id,
                        t.from_type,
                        t.from_id,
                        t.relation,
                        t.to_type,
                        t.to_id,
                        t.metadata as "metadata: Value",
                        t.created_at as "created_at?: OffsetDateTime",
                        t.updated_at as "updated_at?: OffsetDateTime"
                    FROM triples t, snapshot s
                    WHERE t.from_id = $1 AND t.relation = $2
                    AND t.created_xid <= pg_snapshot_xmax(s.snapshot)
                    AND t.deleted_xid > pg_snapshot_xmax(s.snapshot)
                    LIMIT 1
                    "#,
                    from_id,
                    relation,
                    _revision.snapshot_string()
                )
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| anyhow!("Failed to fetch edge: {}", e))
            }
        }
    }

    #[instrument(skip(self))]
    pub async fn get_edges(
        &self,
        from_id: i64,
        relation: &str,
        consistency: ConsistencyMode,
    ) -> Result<Vec<Edge>> {
        match &consistency {
            ConsistencyMode::Full => sqlx::query_as!(
                Edge,
                r#"
                    SELECT 
                        id,
                        from_type,
                        from_id,
                        relation,
                        to_type,
                        to_id,
                        metadata as "metadata: Value",
                        created_at as "created_at?: OffsetDateTime",
                        updated_at as "updated_at?: OffsetDateTime"
                    FROM triples
                    WHERE from_id = $1 AND relation = $2
                    AND created_xid <= pg_current_xact_id()
                    AND deleted_xid > pg_current_xact_id()
                    "#,
                from_id,
                relation
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow!("Failed to fetch edges: {}", e)),
            ConsistencyMode::MinimizeLatency => sqlx::query_as!(
                Edge,
                r#"
                    SELECT 
                        id,
                        from_type,
                        from_id,
                        relation,
                        to_type,
                        to_id,
                        metadata as "metadata: Value",
                        created_at as "created_at?: OffsetDateTime",
                        updated_at as "updated_at?: OffsetDateTime"
                    FROM triples
                    WHERE from_id = $1 AND relation = $2
                    "#,
                from_id,
                relation
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow!("Failed to fetch edges: {}", e)),
            ConsistencyMode::AtLeastAsFresh(_revision) | ConsistencyMode::ExactlyAt(_revision) => {
                sqlx::query_as!(
                    Edge,
                    r#"
                    WITH snapshot AS (
                        SELECT $3::text::pg_snapshot as snapshot
                    )
                    SELECT 
                        t.id,
                        t.from_type,
                        t.from_id,
                        t.relation,
                        t.to_type,
                        t.to_id,
                        t.metadata as "metadata: Value",
                        t.created_at as "created_at?: OffsetDateTime",
                        t.updated_at as "updated_at?: OffsetDateTime"
                    FROM triples t, snapshot s
                    WHERE t.from_id = $1 AND t.relation = $2
                    AND t.created_xid <= pg_snapshot_xmax(s.snapshot)
                    AND t.deleted_xid > pg_snapshot_xmax(s.snapshot)
                    "#,
                    from_id,
                    relation,
                    _revision.snapshot_string()
                )
                .fetch_all(&self.pool)
                .await
                .map_err(|e| anyhow!("Failed to fetch edges: {}", e))
            }
        }
    }

    #[instrument(skip(self))]
    pub async fn get_related_objects(
        &self,
        from_id: i64,
        relation: &str,
    ) -> Result<Vec<ProtoObject>> {
        let query_result = sqlx::query!(
            r#"
            SELECT 
                o.id,
                o.type as "type_name",
                o.metadata as "metadata: Value",
                o.created_at as "created_at?: OffsetDateTime",
                o.updated_at as "updated_at?: OffsetDateTime"
            FROM triples t
            JOIN objects o ON t.to_id = o.id
            WHERE t.from_id = $1 AND t.relation = $2
            "#,
            from_id,
            relation
        )
        .fetch_all(&self.pool)
        .await;

        match query_result {
            Ok(rows) => {
                let objects = rows
                    .into_iter()
                    .map(|row| ProtoObject {
                        id: row.id,
                        r#type: row.type_name,
                        metadata: match json_value_to_prost_value(row.metadata).kind {
                            Some(prost_types::value::Kind::StructValue(v)) => Some(v),
                            _ => todo!(),
                        },
                    })
                    .collect();

                Ok(objects)
            }
            Err(e) => {
                tracing::error!("Failed to get edges: {:?}", e);
                Err(anyhow!("Failed to get edges"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost_types::Struct;
    use sqlx::postgres::PgPoolOptions;

    async fn setup() -> PgPool {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://ent:ent_password@localhost:5432/ent".to_string());

        PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
            .expect("Failed to create connection pool")
    }

    #[tokio::test]
    async fn test_object_operations() {
        let pool = setup().await;
        let repo = GraphRepository::new(pool.clone());

        let (object, _) =
            insert_object(&repo, "user_id".to_string(), "test object".to_string()).await;

        let retrieved = repo
            .get_object(object.id, ConsistencyMode::Full)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retrieved.type_name, "test_type");
        assert_eq!(retrieved.metadata["name"], "test object");
    }

    #[tokio::test]
    async fn test_edge_operations() {
        let pool = setup().await;
        let repo = GraphRepository::new(pool.clone());

        let (from_obj, _) =
            insert_object(&repo, "user_id".to_string(), "from object".to_string()).await;
        let (to_obj, _) =
            insert_object(&repo, "user_id".to_string(), "to object".to_string()).await;

        let (_edge, _) = insert_edge(
            &repo,
            "user_id".to_string(),
            "test_relation".to_string(),
            &from_obj,
            &to_obj,
        )
        .await;

        // Add assertions here if needed
    }

    async fn insert_object(
        repo: &GraphRepository,
        user_id: String,
        object_name: String,
    ) -> (Object, Revision) {
        return repo
            .create_object(
                user_id,
                CreateObjectRequest {
                    r#type: "test_type".to_string(),
                    metadata: Some(Struct {
                        fields: std::collections::BTreeMap::from([(
                            "name".to_string(),
                            ProstValue {
                                kind: Some(prost_types::value::Kind::StringValue(object_name)),
                            },
                        )]),
                    }),
                },
            )
            .await
            .unwrap();
    }

    async fn insert_edge(
        repo: &GraphRepository,
        user_id: String,
        relation: String,
        from: &Object,
        to: &Object,
    ) -> (Edge, Revision) {
        return repo
            .create_edge(
                user_id,
                CreateEdgeRequest {
                    relation: relation.clone(),
                    from_id: from.id,
                    from_type: from.type_name.clone(),
                    to_id: to.id,
                    to_type: to.type_name.clone(),
                    metadata: Some(Struct {
                        fields: std::collections::BTreeMap::from([(
                            "name".to_string(),
                            ProstValue {
                                kind: Some(prost_types::value::Kind::StringValue(relation.clone())),
                            },
                        )]),
                    }),
                },
            )
            .await
            .unwrap();
    }
}
