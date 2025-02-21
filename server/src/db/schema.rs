use anyhow::{anyhow, Result};
use jsonschema::Validator;
use sqlx::PgPool;
use time::OffsetDateTime;
use tracing::instrument;

#[derive(Debug)]
pub struct Schema {
    pub id: i64,
    pub type_name: String,
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
    pub async fn create_schema(&self, type_name: &str, schema: &str) -> Result<Schema> {
        // First validate that the schema string is valid JSON
        let schema_json: serde_json::Value = serde_json::from_str(schema)?;

        // Validate that it's a valid JSON Schema
        Validator::new(&schema_json).map_err(|e| anyhow!("Invalid JSON Schema: {}", e))?;

        // Insert the schema into the database
        let schema = sqlx::query_as!(
            Schema,
            r#"
            INSERT INTO schemata (type_name, schema, created_at, updated_at)
            VALUES ($1, $2, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            RETURNING 
                id, 
                type_name,
                schema as "schema: serde_json::Value",
                created_at as "created_at?: OffsetDateTime",
                updated_at as "updated_at?: OffsetDateTime"
            "#,
            type_name,
            schema_json
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(schema)
    }

    #[instrument(skip(self))]
    pub async fn get_schema(&self, id: i64) -> Result<Option<Schema>> {
        let schema = sqlx::query_as!(
            Schema,
            r#"
            SELECT 
                id, 
                type_name,
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

    #[instrument(skip(self))]
    pub async fn get_schema_by_type(&self, type_name: &str) -> Result<Option<Schema>> {
        let schema = sqlx::query_as!(
            Schema,
            r#"
            SELECT 
                id, 
                type_name,
                schema as "schema: serde_json::Value",
                created_at as "created_at?: OffsetDateTime",
                updated_at as "updated_at?: OffsetDateTime"
            FROM schemata
            WHERE type_name = $1
            "#,
            type_name
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(schema)
    }

    #[instrument(skip(self))]
    pub async fn validate_object(
        &self,
        type_name: &str,
        object: &serde_json::Value,
    ) -> Result<bool> {
        if let Some(schema) = self.get_schema_by_type(type_name).await? {
            let validator = Validator::new(&schema.schema)
                .map_err(|e| anyhow!("Invalid JSON Schema: {}", e))?;

            Ok(validator.validate(object).is_ok())
        } else {
            // If no schema exists, we consider it valid
            Ok(true)
        }
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
        let created = repo.create_schema("test_type", test_schema).await.unwrap();
        assert!(created.id > 0);
        assert_eq!(created.type_name, "test_type");

        // Test retrieving schema by ID
        let retrieved = repo.get_schema(created.id).await.unwrap().unwrap();
        assert_eq!(created.id, retrieved.id);
        assert_eq!(created.schema, retrieved.schema);

        // Test retrieving schema by type
        let retrieved = repo.get_schema_by_type("test_type").await.unwrap().unwrap();
        assert_eq!(created.id, retrieved.id);
        assert_eq!(created.schema, retrieved.schema);
    }

    #[tokio::test]
    async fn test_validate_object() {
        let pool = setup().await;
        let repo = SchemaRepository::new(pool);

        let test_schema = r#"{
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "age": { "type": "number" }
            },
            "required": ["name", "age"]
        }"#;

        // Create schema
        repo.create_schema("person", test_schema).await.unwrap();

        // Test valid object
        let valid_object = serde_json::json!({
            "name": "John",
            "age": 30
        });
        assert!(repo.validate_object("person", &valid_object).await.unwrap());

        // Test invalid object
        let invalid_object = serde_json::json!({
            "name": "John",
            "age": "30" // age should be a number
        });
        assert!(!repo
            .validate_object("person", &invalid_object)
            .await
            .unwrap());
    }
}
