use crate::auth::AuthenticatedRequest;
use crate::db::graph::{GraphRepository, Object, ObjectWithMetadata};
use crate::db::schema::SchemaRepository;
use crate::db::transaction::{ConsistencyMode, Revision};
use ent_proto::ent::consistency_requirement::Requirement;
use ent_proto::ent::graph_service_server::GraphService;
use ent_proto::ent::{
    CreateEdgeRequest, CreateEdgeResponse, CreateObjectRequest, CreateObjectResponse,
    GetEdgeRequest, GetEdgeResponse, GetEdgesRequest, GetEdgesResponse, GetObjectRequest,
    GetObjectResponse, Object as ProtoObject, UpdateObjectRequest, UpdateObjectResponse,
};
use prost_types::Struct;
use prost_types::Value as ProstValue;
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use tonic::{Request, Response, Status};

use super::json_value_to_prost_value;

#[derive(Debug)]
pub struct GraphServer {
    repository: GraphRepository,
    schema_repository: SchemaRepository,
}

impl GraphServer {
    pub fn new(pool: PgPool) -> Self {
        let repository = GraphRepository::new(pool.clone());
        let schema_repository = SchemaRepository::new(pool);
        Self {
            repository,
            schema_repository,
        }
    }

    // Helper function to convert our domain Object to protobuf Object
    fn to_proto_object(obj: ObjectWithMetadata) -> ProtoObject {
        let fields: std::collections::BTreeMap<String, ProstValue> = match obj.metadata {
            JsonValue::Object(map) => map
                .into_iter()
                .map(|(k, v)| (k, json_value_to_prost_value(v.clone())))
                .collect(),
            _ => std::collections::BTreeMap::new(),
        };

        let metadata = if fields.is_empty() {
            None
        } else {
            Some(Struct { fields })
        };

        ProtoObject {
            id: obj.id,
            r#type: obj.type_name,
            metadata,
        }
    }

    async fn validate_object_metadata(
        &self,
        type_name: &str,
        metadata: &JsonValue,
    ) -> Result<(), Status> {
        match self
            .schema_repository
            .validate_object(type_name, metadata)
            .await
        {
            Ok(true) => Ok(()),
            Ok(false) => Err(Status::invalid_argument("Object does not match schema")),
            Err(e) => {
                tracing::error!("Failed to validate object: {:?}", e);
                Err(Status::internal("Failed to validate object"))
            }
        }
    }

    fn parse_consistency_requirement(
        req: Option<ent_proto::ent::ConsistencyRequirement>,
    ) -> Result<ConsistencyMode, Status> {
        match req.and_then(|r| r.requirement) {
            Some(Requirement::FullConsistency(true)) => Ok(ConsistencyMode::Full),
            Some(Requirement::MinimizeLatency(true)) => Ok(ConsistencyMode::MinimizeLatency),
            Some(Requirement::AtLeastAsFresh(zookie)) => match Revision::from_zookie(zookie) {
                Ok(revision) => Ok(ConsistencyMode::AtLeastAsFresh(revision)),
                Err(_) => Err(Status::invalid_argument("Invalid zookie format")),
            },
            Some(Requirement::ExactlyAt(zookie)) => match Revision::from_zookie(zookie) {
                Ok(revision) => Ok(ConsistencyMode::ExactlyAt(revision)),
                Err(_) => Err(Status::invalid_argument("Invalid zookie format")),
            },
            _ => Ok(ConsistencyMode::MinimizeLatency), // Default to minimize latency
        }
    }

    async fn check_object_ownership(&self, object_id: i64, user_id: &str) -> Result<(), Status> {
        match self
            .repository
            .check_object_ownership(object_id, user_id)
            .await
        {
            Ok(true) => Ok(()),
            Ok(false) => Err(Status::permission_denied(
                "You do not have permission to access this object",
            )),
            Err(e) => {
                tracing::error!("Failed to check object ownership: {:?}", e);
                Err(Status::internal("Failed to check object ownership"))
            }
        }
    }
}

#[tonic::async_trait]
impl GraphService for GraphServer {
    #[tracing::instrument(skip(self))]
    async fn get_object(
        &self,
        request: Request<GetObjectRequest>,
    ) -> Result<Response<GetObjectResponse>, Status> {
        let user_id = request.user_id()?;
        let req = request.into_inner();
        let consistency = Self::parse_consistency_requirement(req.consistency)?;

        // Check object ownership
        self.check_object_ownership(req.object_id, &user_id).await?;

        match self.repository.get_object(req.object_id, consistency).await {
            Ok(Some(obj)) => Ok(Response::new(GetObjectResponse {
                object: Some(Self::to_proto_object(obj)),
            })),
            Ok(None) => Err(Status::not_found("Object not found")),
            Err(e) => {
                tracing::error!("Failed to get object: {:?}", e);
                Err(Status::internal("Failed to get object"))
            }
        }
    }

    #[tracing::instrument(skip(self))]
    async fn get_edge(
        &self,
        request: Request<GetEdgeRequest>,
    ) -> Result<Response<GetEdgeResponse>, Status> {
        let req = request.into_inner();
        let consistency = Self::parse_consistency_requirement(req.consistency)?;

        match self
            .repository
            .get_edge(req.object_id, &req.edge_type, consistency.clone())
            .await
        {
            Ok(Some(edge)) => {
                // Get the target object with the same consistency requirement
                match self.repository.get_object(edge.to_id, consistency).await {
                    Ok(Some(obj)) => Ok(Response::new(GetEdgeResponse {
                        object: Some(Self::to_proto_object(obj)),
                    })),
                    Ok(None) => Err(Status::not_found("Target object not found")),
                    Err(e) => {
                        tracing::error!("Failed to get target object: {:?}", e);
                        Err(Status::internal("Failed to get target object"))
                    }
                }
            }
            Ok(None) => Err(Status::not_found("Edge not found")),
            Err(e) => {
                tracing::error!("Failed to get edge: {:?}", e);
                Err(Status::internal("Failed to get edge"))
            }
        }
    }

    #[tracing::instrument(skip(self))]
    async fn get_edges(
        &self,
        request: Request<GetEdgesRequest>,
    ) -> Result<Response<GetEdgesResponse>, Status> {
        let req = request.into_inner();
        let consistency = Self::parse_consistency_requirement(req.consistency)?;

        match self
            .repository
            .get_edges(req.object_id, &req.edge_type, consistency.clone())
            .await
        {
            Ok(edges) => {
                let mut objects = Vec::new();
                for edge in edges {
                    match self
                        .repository
                        .get_object(edge.to_id, consistency.clone())
                        .await
                    {
                        Ok(Some(obj)) => {
                            objects.push(Self::to_proto_object(obj));
                        }
                        Ok(None) => {
                            tracing::warn!("Target object not found for edge: {:?}", edge);
                            continue;
                        }
                        Err(e) => {
                            tracing::error!("Failed to get target object: {:?}", e);
                            return Err(Status::internal("Failed to get target objects"));
                        }
                    }
                }
                Ok(Response::new(GetEdgesResponse { objects }))
            }
            Err(e) => {
                tracing::error!("Failed to get edges: {:?}", e);
                Err(Status::internal("Failed to get edges"))
            }
        }
    }

    async fn create_object(
        &self,
        request: Request<CreateObjectRequest>,
    ) -> Result<Response<CreateObjectResponse>, Status> {
        // Extract user ID from JWT
        let user_id = request.user_id()?;
        let req = request.into_inner();

        // Convert metadata to JSON for validation
        let metadata = match &req.metadata {
            Some(metadata) => {
                let mut map = serde_json::Map::new();
                for (k, v) in &metadata.fields {
                    map.insert(k.clone(), super::prost_value_to_json_value(v.clone()));
                }
                JsonValue::Object(map)
            }
            None => JsonValue::Object(serde_json::Map::new()),
        };

        // Validate against schema if one exists
        self.validate_object_metadata(&req.r#type, &metadata)
            .await?;

        // Use the user_id when creating the object
        let (object, revision) = self
            .repository
            .create_object(user_id, req)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(CreateObjectResponse {
            object: Some(Self::to_proto_object(object)),
            revision: revision.to_zookie().ok(),
        }))
    }

    async fn create_edge(
        &self,
        request: Request<CreateEdgeRequest>,
    ) -> Result<Response<CreateEdgeResponse>, Status> {
        let user_id = request.user_id()?;

        let req = request.into_inner();

        // Use the user_id when creating the object
        // This would be stored in your database along with the object
        let (edge, revision) = self
            .repository
            .create_edge(user_id, req)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(CreateEdgeResponse {
            edge: Some(edge.to_pb()),
            revision: revision.to_zookie().ok(), // Fill this in based on your revision tracking
        }))
    }

    async fn update_object(
        &self,
        request: Request<UpdateObjectRequest>,
    ) -> Result<Response<UpdateObjectResponse>, Status> {
        // Extract user ID from JWT
        let user_id = request.user_id()?;
        let req = request.into_inner();

        // Check object ownership
        self.check_object_ownership(req.object_id, &user_id).await?;

        // Convert metadata to JSON for validation
        let metadata = match &req.metadata {
            Some(metadata) => {
                let mut map = serde_json::Map::new();
                for (k, v) in &metadata.fields {
                    map.insert(k.clone(), super::prost_value_to_json_value(v.clone()));
                }
                JsonValue::Object(map)
            }
            None => JsonValue::Object(serde_json::Map::new()),
        };

        // Get the object to validate its type
        let existing_object = match self
            .repository
            .get_object(req.object_id, ConsistencyMode::Full)
            .await
        {
            Ok(Some(obj)) => obj,
            Ok(None) => return Err(Status::not_found("Object not found")),
            Err(e) => {
                tracing::error!("Failed to get object: {:?}", e);
                return Err(Status::internal("Failed to get object"));
            }
        };

        // Validate against schema if one exists
        self.validate_object_metadata(&existing_object.type_name, &metadata)
            .await?;

        // Use the user_id when updating the object
        let (object, revision) = self
            .repository
            .update_object(user_id, req.object_id, metadata)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(UpdateObjectResponse {
            object: Some(Self::to_proto_object(object)),
            revision: revision.to_zookie().ok(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use serde_json::json;
    use serde_json::Value as JsonValue;
    use std::collections::HashMap;

    /// Custom strategy to generate JSON values
    fn json_value_strategy() -> impl Strategy<Value = JsonValue> {
        let leaf = prop_oneof![
            Just(JsonValue::Null),
            any::<bool>().prop_map(JsonValue::Bool),
            // Generate numbers within safe ranges
            (-1000.0..1000.0f64).prop_map(|f| JsonValue::Number(
                serde_json::Number::from_f64(f).unwrap_or_else(|| serde_json::Number::from(0))
            )),
            ".*".prop_map(JsonValue::String)
        ];

        leaf.prop_recursive(
            8,   // Max recursion depth
            256, // Max size
            10,  // Number of items per collection
            |inner| {
                prop_oneof![
                    // Generate arrays
                    prop::collection::vec(inner.clone(), 0..10).prop_map(JsonValue::Array),
                    // Generate objects with proper conversion to serde_json::Map
                    prop::collection::hash_map(".*", inner, 0..10).prop_map(
                        |map: HashMap<String, JsonValue>| {
                            let converted: serde_json::Map<String, JsonValue> =
                                map.into_iter().collect();
                            JsonValue::Object(converted)
                        }
                    )
                ]
            },
        )
    }

    /// Helper to convert from ProstValue to JsonValue
    fn prost_value_to_json(value: &prost_types::Value) -> JsonValue {
        match &value.kind {
            Some(prost_types::value::Kind::NullValue(_)) => JsonValue::Null,
            Some(prost_types::value::Kind::NumberValue(n)) => JsonValue::Number(
                serde_json::Number::from_f64(*n).unwrap_or_else(|| serde_json::Number::from(0)),
            ),
            Some(prost_types::value::Kind::StringValue(s)) => JsonValue::String(s.clone()),
            Some(prost_types::value::Kind::BoolValue(b)) => JsonValue::Bool(*b),
            Some(prost_types::value::Kind::StructValue(s)) => {
                let mut map = serde_json::Map::new();
                for (k, v) in &s.fields {
                    map.insert(k.clone(), prost_value_to_json(v));
                }
                JsonValue::Object(map)
            }
            Some(prost_types::value::Kind::ListValue(l)) => {
                JsonValue::Array(l.values.iter().map(prost_value_to_json).collect())
            }
            None => JsonValue::Null,
        }
    }

    /// Test that converting to protobuf and back preserves the semantic meaning
    fn round_trip_json(original: &JsonValue) -> JsonValue {
        // Convert JSON -> Protobuf
        let prost_value = json_value_to_prost_value(original.clone());
        // Convert Protobuf -> JSON
        prost_value_to_json(&prost_value)
    }

    proptest! {
        // Test that any valid JSON value can be converted to protobuf
        #[test]
        fn test_json_conversion_doesnt_panic(value in json_value_strategy()) {
            let _ = json_value_to_prost_value(value);
        }

        // Test that conversion preserves values (within reasonable bounds)
        #[test]
        fn test_json_round_trip(
            original in json_value_strategy()
                .prop_filter("Filter out non-finite numbers", |v| {
                    match v {
                        JsonValue::Number(n) => n.as_f64()
                            .map(|f| f.is_finite())
                            .unwrap_or(true),
                        _ => true,
                    }
                })
        ) {
            let original_clone = original.clone();
            let result = round_trip_json(&original);

            // For numbers, we need to compare with some tolerance due to floating point precision
            match (original_clone, &result) {
                (JsonValue::Number(n1), JsonValue::Number(n2)) => {
                    if let (Some(f1), Some(f2)) = (n1.as_f64(), n2.as_f64()) {
                        assert!((f1 - f2).abs() < 1e-10,
                            "Numbers differ too much: {} vs {}", f1, f2);
                    } else {
                        assert_eq!(original, result);
                    }
                }
                _ => assert_eq!(original, result)
            }
        }

        // Test nested structures specifically
        #[test]
        fn test_nested_structures(
            keys in prop::collection::vec(".*", 1..5),
            values in prop::collection::vec(json_value_strategy(), 1..5)
        ) {
            let mut map = serde_json::Map::new();
            for (k, v) in keys.into_iter().zip(values) {
                map.insert(k, v);
            }
            let original = JsonValue::Object(map);
            let result = round_trip_json(&original);
            assert_eq!(original, result);
        }
    }

    fn normalize_numbers(value: &JsonValue) -> JsonValue {
        match value {
            JsonValue::Number(n) => {
                if let Some(f) = n.as_f64() {
                    // Check if it's a whole number
                    if f.fract() == 0.0 {
                        JsonValue::Number(serde_json::Number::from(f as i64))
                    } else {
                        JsonValue::Number(serde_json::Number::from_f64(f).unwrap())
                    }
                } else {
                    value.clone()
                }
            }
            JsonValue::Array(arr) => JsonValue::Array(arr.iter().map(normalize_numbers).collect()),
            JsonValue::Object(obj) => {
                let mut new_obj = serde_json::Map::new();
                for (k, v) in obj {
                    new_obj.insert(k.clone(), normalize_numbers(v));
                }
                JsonValue::Object(new_obj)
            }
            _ => value.clone(),
        }
    }

    #[test]
    fn test_edge_cases() {
        let test_cases = vec![
            json!(1000.0),
            json!(-1000.0),
            json!({ "": null }), // Empty key
            json!([]),           // Empty array
            json!({}),           // Empty object
            // Deeply nested structure
            json!({
                "a": {
                    "b": {
                        "c": [1, 2, 3]
                    }
                }
            }),
        ];

        for case in test_cases {
            let result = round_trip_json(&case);
            // Normalize both the input and output before comparison
            let normalized_case = normalize_numbers(&case);
            let normalized_result = normalize_numbers(&result);
            assert_eq!(
                normalized_case, normalized_result,
                "Failed for case: {}\nGot result: {}",
                case, result
            );
        }
    }

    // Add test to specifically verify number handling
    #[test]
    fn test_number_handling() {
        let test_cases = vec![
            (json!(1), json!(1.0)),                       // Integer to float
            (json!(0), json!(0.0)),                       // Zero
            (json!(-1), json!(-1.0)),                     // Negative
            (json!(1.5), json!(1.5)),                     // Float stays float
            (json!([1, 2.5, 3]), json!([1.0, 2.5, 3.0])), // Mixed array
        ];

        for (input, expected) in test_cases {
            let result = round_trip_json(&input);
            assert_eq!(
                normalize_numbers(&expected),
                normalize_numbers(&result),
                "Failed for input: {}\nExpected: {}\nGot: {}",
                input,
                expected,
                result
            );
        }
    }
}
