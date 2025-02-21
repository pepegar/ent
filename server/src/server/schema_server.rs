use crate::db::schema::SchemaRepository;
use ent_proto::ent::schema_service_server::SchemaService;
use ent_proto::ent::{CreateSchemaRequest, CreateSchemaResponse};
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
        let req = request.into_inner();
        let type_name = req.type_name.clone();

        if type_name.is_empty() {
            return Err(Status::invalid_argument("type_name is required"));
        }

        match self.repository.create_schema(&type_name, &req.schema).await {
            Ok(schema) => Ok(Response::new(CreateSchemaResponse {
                schema_id: schema.id,
            })),
            Err(e) => {
                tracing::error!("Failed to create schema: {:?}", e);
                Err(Status::internal("Failed to create schema"))
            }
        }
    }
}
