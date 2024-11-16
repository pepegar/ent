use ent::graph_service_server::GraphService;
use ent::schema_service_server::SchemaService;
use ent::{
    CreateSchemaRequest, CreateSchemaResponse, GetEdgeRequest, GetEdgeResponse, GetEdgesRequest,
    GetEdgesResponse, GetObjectRequest, GetObjectResponse,
};
use tonic::{async_trait, Request, Response, Status};

pub mod ent {
    tonic::include_proto!("ent");
}

#[derive(Debug)]
pub struct EntServer {}

impl EntServer {
    pub(crate) fn new() -> EntServer {
        EntServer {}
    }
}

#[tonic::async_trait()]
impl GraphService for EntServer {
    async fn get_object(
        &self,
        _request: Request<GetObjectRequest>,
    ) -> Result<Response<GetObjectResponse>, Status> {
        todo!();
    }
    async fn get_edge(
        &self,
        _request: Request<GetEdgeRequest>,
    ) -> Result<Response<GetEdgeResponse>, Status> {
        todo!();
    }

    async fn get_edges(
        &self,
        _request: Request<GetEdgesRequest>,
    ) -> Result<Response<GetEdgesResponse>, Status> {
        todo!();
    }
}

#[async_trait()]
impl SchemaService for EntServer {
    async fn create_schema(
        &self,
        _request: Request<CreateSchemaRequest>,
    ) -> Result<Response<CreateSchemaResponse>, Status> {
        todo!();
    }
}
