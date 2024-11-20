use anyhow::Result;
use serde_json::Value;
use sqlx::PgPool;
use time::OffsetDateTime;
use tracing::instrument;

#[derive(Debug)]
pub struct Object {
    pub id: i32,
    pub type_name: String,
    pub metadata: Value,
    pub created_at: Option<OffsetDateTime>,
    pub updated_at: Option<OffsetDateTime>,
}

#[derive(Debug)]
pub struct Edge {
    pub id: i32,
    pub from_type: String,
    pub from_id: i32,
    pub relation: String,
    pub to_type: String,
    pub to_id: i32,
    pub metadata: Value,
    pub created_at: Option<OffsetDateTime>,
}

#[derive(Debug)]
pub struct GraphRepository {
    pool: PgPool,
}

impl GraphRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    #[instrument(skip(self))]
    pub async fn get_object(&self, id: i32) -> Result<Option<Object>> {
        let object = sqlx::query_as!(
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
        .await?;

        Ok(object)
    }

    #[instrument(skip(self))]
    pub async fn get_edge(&self, from_id: i32, relation: &str) -> Result<Option<Edge>> {
        let edge = sqlx::query_as!(
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
                created_at as "created_at?: OffsetDateTime"
            FROM triples
            WHERE from_id = $1 AND relation = $2
            LIMIT 1
            "#,
            from_id,
            relation
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(edge)
    }

    #[instrument(skip(self))]
    pub async fn get_edges(&self, from_id: i32, relation: &str) -> Result<Vec<Edge>> {
        let edges = sqlx::query_as!(
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
                created_at as "created_at?: OffsetDateTime"
            FROM triples
            WHERE from_id = $1 AND relation = $2
            "#,
            from_id,
            relation
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(edges)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
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

        // Insert test object
        let test_object = sqlx::query!(
            r#"
            INSERT INTO objects (type, metadata)
            VALUES ($1, $2)
            RETURNING id
            "#,
            "test_type",
            json!({"name": "test object"})
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        let repo = GraphRepository::new(pool.clone());

        // Test retrieving object
        let retrieved = repo.get_object(test_object.id).await.unwrap().unwrap();
        assert_eq!(retrieved.type_name, "test_type");
        assert_eq!(retrieved.metadata["name"], "test object");
    }

    #[tokio::test]
    async fn test_edge_operations() {
        let pool = setup().await;

        // Insert test objects
        let obj1 = sqlx::query!(
            r#"
            INSERT INTO objects (type, metadata)
            VALUES ($1, $2)
            RETURNING id
            "#,
            "test_type",
            json!({"name": "object 1"})
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        let obj2 = sqlx::query!(
            r#"
            INSERT INTO objects (type, metadata)
            VALUES ($1, $2)
            RETURNING id
            "#,
            "test_type",
            json!({"name": "object 2"})
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        // Create test edge
        sqlx::query!(
            r#"
            INSERT INTO triples (from_type, from_id, relation, to_type, to_id, metadata)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            "test_type",
            obj1.id,
            "test_relation",
            "test_type",
            obj2.id,
            json!({})
        )
        .execute(&pool)
        .await
        .unwrap();

        let repo = GraphRepository::new(pool);

        // Test getting single edge
        let edge = repo
            .get_edge(obj1.id, "test_relation")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(edge.from_id, obj1.id);
        assert_eq!(edge.to_id, obj2.id);
        assert_eq!(edge.relation, "test_relation");

        // Test getting multiple edges
        let edges = repo.get_edges(obj1.id, "test_relation").await.unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].from_id, obj1.id);
    }
}
