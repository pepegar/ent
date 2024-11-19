use super::ent::schema_service_server::SchemaService;
use super::ent::{CreateSchemaRequest, CreateSchemaResponse};
use crate::db::schema::SchemaRepository;
use sqlx::PgPool;
use tonic::{async_trait, Request, Response, Status};

#[derive(Debug)]
pub struct SchemaServer {
    repository: SchemaRepository,
}

impl SchemaServer {
    pub fn new(pool: PgPool) -> Self {
        let repository = SchemaRepository::new(pool);
        SchemaServer { repository }
    }
}

#[async_trait]
impl SchemaService for SchemaServer {
    #[tracing::instrument(skip(self))]
    async fn create_schema(
        &self,
        request: Request<CreateSchemaRequest>,
    ) -> Result<Response<CreateSchemaResponse>, Status> {
        let schema = request.into_inner().schema;

        match self.repository.create_schema(&schema).await {
            Ok(_) => Ok(Response::new(CreateSchemaResponse {})),
            Err(e) => {
                tracing::error!("Failed to create schema: {:?}", e);
                Err(Status::internal("Failed to create schema"))
            }
        }
    }
}
