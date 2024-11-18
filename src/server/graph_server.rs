use super::ent::graph_service_server::GraphService;
use super::ent::{
    GetEdgeRequest, GetEdgeResponse, GetEdgesRequest, GetEdgesResponse, GetObjectRequest,
    GetObjectResponse,
};
use tonic::{Request, Response, Status};

#[derive(Debug)]
pub struct GraphServer {}

impl GraphServer {
    pub(crate) fn new() -> GraphServer {
        GraphServer {}
    }
}

#[tonic::async_trait()]
impl GraphService for GraphServer {
    #[tracing::instrument]
    async fn get_object(
        &self,
        _request: Request<GetObjectRequest>,
    ) -> Result<Response<GetObjectResponse>, Status> {
        todo!();
    }

    #[tracing::instrument]
    async fn get_edge(
        &self,
        _request: Request<GetEdgeRequest>,
    ) -> Result<Response<GetEdgeResponse>, Status> {
        todo!();
    }

    #[tracing::instrument]
    async fn get_edges(
        &self,
        _request: Request<GetEdgesRequest>,
    ) -> Result<Response<GetEdgesResponse>, Status> {
        todo!();
    }
}
