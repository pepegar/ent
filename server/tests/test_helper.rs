use anyhow::Result;
use ent_proto::ent::{
    graph_service_client::GraphServiceClient, schema_service_client::SchemaServiceClient,
    CreateEdgeRequest, CreateObjectRequest, CreateSchemaRequest, Edge, Object,
};
use ent_server::server::json_value_to_prost_value;
use prost_types::Struct;
use serde_json::Value as JsonValue;
use tracing::info;

use crate::jwt::generate_test_token;

// Represents a user context for testing
#[derive(Debug, Clone)]
pub struct TestUser {
    id: String,
    token: String,
}

// Stores created objects for reference
#[derive(Debug)]
pub struct CreatedObject {
    user_index: usize,
    object: Object,
}

// Edge creation request with object indices
#[derive(Debug)]
struct EdgeCreationRequest {
    user_index: usize,
    from_object_index: usize,
    to_object_index: usize,
    relation: String,
    metadata: JsonValue,
}

// Main builder struct
#[derive(Default)]
pub struct EntTestBuilder {
    schema: Option<String>,
    users: Vec<TestUser>,
    objects_to_create: Vec<(usize, CreateObjectRequest)>,
    edges_to_create: Vec<EdgeCreationRequest>,
    created_objects: Vec<CreatedObject>,
    created_edges: Vec<Edge>,
}

fn json_to_protobuf_struct(value: JsonValue) -> Option<Struct> {
    match json_value_to_prost_value(value).kind {
        Some(prost_types::value::Kind::StructValue(s)) => Some(s),
        _ => None,
    }
}

impl EntTestBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_schema(mut self, schema: impl Into<String>) -> Self {
        self.schema = Some(schema.into());
        self
    }

    pub fn with_user(mut self, user_id: impl Into<String>) -> Self {
        let user_id = user_id.into();
        let token = generate_test_token(&user_id).unwrap();

        self.users.push(TestUser { id: user_id, token });
        self
    }

    pub fn with_object(
        mut self,
        user_index: usize,
        type_name: impl Into<String>,
        metadata: JsonValue,
    ) -> Self {
        let request = CreateObjectRequest {
            r#type: type_name.into(),
            metadata: json_to_protobuf_struct(metadata),
        };

        self.objects_to_create.push((user_index, request));
        self
    }

    pub fn with_edge(
        mut self,
        user_index: usize,
        from_index: usize,
        to_index: usize,
        relation: impl Into<String>,
        metadata: JsonValue,
    ) -> Self {
        self.edges_to_create.push(EdgeCreationRequest {
            user_index,
            from_object_index: from_index,
            to_object_index: to_index,
            relation: relation.into(),
            metadata,
        });
        self
    }

    pub async fn build(mut self, addr: String) -> Result<EntTestState> {
        let mut schema_client = SchemaServiceClient::connect(addr.clone()).await?;
        let mut graph_client = GraphServiceClient::connect(addr).await?;

        if let Some(schema) = self.schema {
            let request = CreateSchemaRequest {
                schema,
                description: "Test schema".to_string(),
            };
            info!(schema = &request.schema);
            let response = schema_client.create_schema(request).await?;
            info!(response = ?response);
        }

        // Create objects
        for (user_index, request) in self.objects_to_create {
            let user = &self.users[user_index];
            let mut request = tonic::Request::new(request);
            request
                .metadata_mut()
                .insert("authorization", format!("Bearer {}", user.token).parse()?);

            info!(request = ?request);

            let response = graph_client.create_object(request).await?;
            info!(response = ?response);
            if let Some(object) = response.into_inner().object {
                self.created_objects
                    .push(CreatedObject { user_index, object });
            }
        }

        // Create edges
        for edge_request in self.edges_to_create {
            let from_obj = &self.created_objects[edge_request.from_object_index].object;
            let to_obj = &self.created_objects[edge_request.to_object_index].object;

            let request = CreateEdgeRequest {
                from_id: from_obj.id,
                from_type: from_obj.r#type.clone(),
                to_id: to_obj.id,
                to_type: to_obj.r#type.clone(),
                relation: edge_request.relation,
                metadata: json_to_protobuf_struct(edge_request.metadata),
            };

            let user = &self.users[edge_request.user_index];
            let mut request = tonic::Request::new(request);
            request
                .metadata_mut()
                .insert("authorization", format!("Bearer {}", user.token).parse()?);

            info!(request = ?request);

            let response = graph_client.create_edge(request).await?;

            info!(response = ?response);
            if let Some(edge) = response.into_inner().edge {
                self.created_edges.push(edge);
            }
        }

        Ok(EntTestState {
            users: self.users,
            objects: self.created_objects,
            edges: self.created_edges,
        })
    }
}

#[derive(Debug)]
pub struct EntTestState {
    pub users: Vec<TestUser>,
    pub objects: Vec<CreatedObject>,
    pub edges: Vec<Edge>,
}

impl EntTestState {
    pub fn get_object(&self, index: usize) -> Option<&Object> {
        self.objects.get(index).map(|co| &co.object)
    }

    pub fn get_user_token(&self, index: usize) -> Option<&str> {
        self.users.get(index).map(|u| u.token.as_str())
    }
}
