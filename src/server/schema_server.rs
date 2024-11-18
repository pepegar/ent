use super::ent::schema_service_server::SchemaService;
use super::ent::{CreateSchemaRequest, CreateSchemaResponse};
use tonic::{async_trait, Request, Response, Status};

#[derive(Debug)]
pub struct SchemaServer {}

impl SchemaServer {
    pub(crate) fn new() -> SchemaServer {
        SchemaServer {}
    }
}

#[async_trait()]
impl SchemaService for SchemaServer {
    async fn create_schema(
        &self,
        _request: Request<CreateSchemaRequest>,
    ) -> Result<Response<CreateSchemaResponse>, Status> {
        todo!();
    }
}
