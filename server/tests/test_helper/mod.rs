mod assertions;
mod fixtures;

use anyhow::Result;
use ent_proto::ent::{
    graph_service_client::GraphServiceClient, schema_service_client::SchemaServiceClient,
    CreateEdgeRequest, CreateObjectRequest, CreateSchemaRequest, Edge, Object,
};
use ent_server::server::json_value_to_prost_value;
use prost_types::Struct;
use serde_json::Value as JsonValue;
use tracing::info;
use uuid::Uuid;

pub use fixtures::{TestObjects, TestSchemas};

use crate::jwt::generate_test_token;

// Represents a user context for testing
#[derive(Debug, Clone)]
pub struct TestUser {
    #[allow(dead_code)]
    id: String,
    token: String,
}

// Stores created objects for reference
#[derive(Debug, Clone)]
pub struct CreatedObject {
    #[allow(dead_code)]
    user_index: usize,
    object: Object,
}

// Edge creation request with object indices
#[derive(Debug, Default, Clone)]
struct EdgeCreationRequest {
    user_index: usize,
    from_object_index: usize,
    to_object_index: usize,
    relation: String,
    metadata: JsonValue,
}

// Main builder struct
#[derive(Default, Clone)]
pub struct EntTestBuilder {
    schema: Option<String>,
    type_name: Option<String>,
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

    pub fn with_schema_and_type(
        mut self,
        schema: impl Into<String>,
        type_name: impl Into<String>,
    ) -> Self {
        self.schema = Some(schema.into());
        self.type_name = Some(type_name.into());
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

    // New method to test schema creation with expected error
    pub async fn try_create_schema(&self, addr: String) -> Result<(), tonic::Status> {
        let mut schema_client = match SchemaServiceClient::connect(addr).await {
            Ok(client) => client,
            Err(e) => return Err(tonic::Status::internal(e.to_string())),
        };

        let schema = self
            .schema
            .as_ref()
            .expect("Schema must be set for try_create_schema");
        let type_name_str = format!("test_type_{}", Uuid::new_v4());
        let type_name = self.type_name.as_ref().unwrap_or(&type_name_str);

        let request = CreateSchemaRequest {
            schema: schema.to_string(),
            type_name: type_name.to_string(),
            description: "Test schema".to_string(),
        };

        schema_client.create_schema(request).await.map(|_| ())
    }

    // New method to test object creation with expected error
    pub async fn try_create_object(
        &self,
        address: String,
        object_index: usize,
        type_name: &str,
        metadata: serde_json::Value,
    ) -> anyhow::Result<()> {
        let mut client = GraphServiceClient::connect(address)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to connect to graph service: {}", e))?;

        let metadata_struct = match json_to_protobuf_struct(metadata) {
            Some(s) => s,
            None => {
                return Err(anyhow::anyhow!(
                    "Failed to convert metadata to protobuf struct"
                ))
            }
        };

        let mut request = tonic::Request::new(CreateObjectRequest {
            r#type: type_name.to_string(),
            metadata: Some(metadata_struct),
        });

        let user = &self.users[object_index];
        let auth_header = format!("Bearer {}", &user.token);
        let auth_value: tonic::metadata::MetadataValue<tonic::metadata::Ascii> = auth_header
            .parse()
            .map_err(|e| anyhow::anyhow!("Failed to parse auth header: {}", e))?;

        request.metadata_mut().insert("authorization", auth_value);

        client
            .create_object(request)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create object: {}", e))?;
        Ok(())
    }

    pub async fn build(mut self, addr: String) -> Result<EntTestState> {
        let mut schema_client = SchemaServiceClient::connect(addr.clone()).await?;
        let mut graph_client = GraphServiceClient::connect(addr).await?;

        let type_name = if let Some(schema) = self.schema {
            let type_name = self
                .type_name
                .unwrap_or_else(|| format!("test_type_{}", Uuid::new_v4()));
            let request = CreateSchemaRequest {
                schema: schema.to_string(),
                type_name: type_name.clone(),
                description: "Test schema".to_string(),
            };
            info!(schema = &request.schema);
            let response = schema_client.create_schema(request).await?;
            info!(response = ?response);
            Some(type_name)
        } else {
            None
        };

        // Update all object requests to use the new type name if schema was created
        if let Some(type_name) = type_name {
            for (_, request) in self.objects_to_create.iter_mut() {
                request.r#type = type_name.clone();
            }
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

    // Add common schema patterns
    pub fn with_basic_schema(self) -> Self {
        self.with_schema(TestSchemas::new().basic_schema)
    }

    pub fn with_user_schema(self) -> Self {
        self.with_schema(TestSchemas::new().user_schema)
    }

    pub fn with_complex_schema(self) -> Self {
        self.with_schema(TestSchemas::new().complex_schema)
    }

    // Add common object patterns
    pub fn with_basic_object(self, user_index: usize) -> Self {
        self.with_object(user_index, "basic_type", TestObjects::new().basic_object)
    }

    pub fn with_user_object(self, user_index: usize) -> Self {
        self.with_object(user_index, "user_type", TestObjects::new().user_object)
    }

    // Add common test scenarios
    pub fn create_two_connected_objects(&mut self, user_index: usize) -> Result<(usize, usize)> {
        let obj1_index = self.objects_to_create.len();
        let obj2_index = obj1_index + 1;

        *self = self
            .clone()
            .with_basic_object(user_index)
            .with_basic_object(user_index)
            .with_edge(
                user_index,
                obj1_index,
                obj2_index,
                "connected_to",
                serde_json::json!({"weight": 1}),
            );

        Ok((obj1_index, obj2_index))
    }

    // Add debugging helpers
    pub fn debug_state(&self) -> String {
        format!(
            "Test State:\n\
             - Users: {}\n\
             - Objects to create: {}\n\
             - Edges to create: {}\n\
             - Created objects: {}\n\
             - Created edges: {}\n",
            self.users.len(),
            self.objects_to_create.len(),
            self.edges_to_create.len(),
            self.created_objects.len(),
            self.created_edges.len()
        )
    }
}

#[derive(Debug)]
pub struct EntTestState {
    pub users: Vec<TestUser>,
    pub objects: Vec<CreatedObject>,
    #[allow(dead_code)]
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
