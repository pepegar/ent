use anyhow::Result;
use sqlx::PgPool;
use time::OffsetDateTime;
use tracing::instrument;

#[derive(Debug)]
pub struct Schema {
    pub id: i32,
    pub schema: serde_json::Value,
    pub created_at: Option<OffsetDateTime>,
    pub updated_at: Option<OffsetDateTime>,
}

#[derive(Debug)]
pub struct SchemaRepository {
    pool: PgPool,
}

impl SchemaRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    #[instrument(skip(self, schema))]
    pub async fn create_schema(&self, schema: &str) -> Result<Schema> {
        // First validate that the schema string is valid JSON
        let schema_json: serde_json::Value = serde_json::from_str(schema)?;

        // Insert the schema into the database
        let schema = sqlx::query_as!(
            Schema,
            r#"
            INSERT INTO schemata (schema, created_at, updated_at)
            VALUES ($1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            RETURNING 
                id, 
                schema as "schema: serde_json::Value",
                created_at as "created_at?: OffsetDateTime",
                updated_at as "updated_at?: OffsetDateTime"
            "#,
            schema_json
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(schema)
    }

    #[instrument(skip(self))]
    pub async fn get_schema(&self, id: i32) -> Result<Option<Schema>> {
        let schema = sqlx::query_as!(
            Schema,
            r#"
            SELECT 
                id, 
                schema as "schema: serde_json::Value",
                created_at as "created_at?: OffsetDateTime",
                updated_at as "updated_at?: OffsetDateTime"
            FROM schemata
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(schema)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    async fn test_create_and_get_schema() {
        let pool = setup().await;
        let repo = SchemaRepository::new(pool);

        let test_schema = r#"{
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "age": { "type": "number" }
            }
        }"#;

        // Test creating schema
        let created = repo.create_schema(test_schema).await.unwrap();
        assert!(created.id > 0);

        // Test retrieving schema
        let retrieved = repo.get_schema(created.id).await.unwrap().unwrap();
        assert_eq!(created.id, retrieved.id);
        assert_eq!(created.schema, retrieved.schema);
    }
}
