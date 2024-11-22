use super::ent::graph_service_server::GraphService;
use super::ent::{
    CreateObjectRequest, CreateObjectResponse, GetEdgeRequest, GetEdgeResponse, GetEdgesRequest,
    GetEdgesResponse, GetObjectRequest, GetObjectResponse, Object as ProtoObject,
};
use crate::db::graph::{GraphRepository, Object};
use prost_types::Struct;
use prost_types::Value as ProstValue;
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use tonic::{Request, Response, Status};

#[derive(Debug)]
pub struct GraphServer {
    repository: GraphRepository,
}

impl GraphServer {
    pub fn new(pool: PgPool) -> Self {
        let repository = GraphRepository::new(pool);
        Self { repository }
    }

    // Helper function to convert serde_json::Value to prost_types::Value
    fn json_value_to_prost_value(json_value: JsonValue) -> ProstValue {
        match json_value {
            JsonValue::Null => ProstValue {
                kind: Some(prost_types::value::Kind::NullValue(0)),
            },
            JsonValue::Bool(b) => ProstValue {
                kind: Some(prost_types::value::Kind::BoolValue(b)),
            },
            JsonValue::Number(n) => {
                if let Some(f) = n.as_f64() {
                    ProstValue {
                        kind: Some(prost_types::value::Kind::NumberValue(f)),
                    }
                } else {
                    // Handle integers that don't fit in f64
                    ProstValue {
                        kind: Some(prost_types::value::Kind::StringValue(n.to_string())),
                    }
                }
            }
            JsonValue::String(s) => ProstValue {
                kind: Some(prost_types::value::Kind::StringValue(s)),
            },
            JsonValue::Array(arr) => {
                let values: Vec<ProstValue> = arr
                    .into_iter()
                    .map(Self::json_value_to_prost_value)
                    .collect();
                ProstValue {
                    kind: Some(prost_types::value::Kind::ListValue(
                        prost_types::ListValue { values },
                    )),
                }
            }
            JsonValue::Object(map) => {
                let mut fields = std::collections::BTreeMap::new();
                for (k, v) in map {
                    fields.insert(k, Self::json_value_to_prost_value(v));
                }
                ProstValue {
                    kind: Some(prost_types::value::Kind::StructValue(Struct { fields })),
                }
            }
        }
    }

    // Helper function to convert our domain Object to protobuf Object
    fn to_proto_object(obj: Object) -> ProtoObject {
        let fields: std::collections::BTreeMap<String, ProstValue> = match obj.metadata {
            JsonValue::Object(map) => map
                .into_iter()
                .map(|(k, v)| (k, Self::json_value_to_prost_value(v)))
                .collect(),
            _ => std::collections::BTreeMap::new(), // Handle non-object values
        };

        let metadata = Struct { fields };

        ProtoObject {
            object_id: obj.id.to_string(),
            r#type: obj.type_name,
            metadata: Some(metadata),
        }
    }

    // Helper function to convert object ID from string to i32
    fn parse_object_id(id: &str) -> Result<i32, Status> {
        id.parse::<i32>()
            .map_err(|_| Status::invalid_argument("Invalid object ID format"))
    }
}

#[tonic::async_trait]
impl GraphService for GraphServer {
    #[tracing::instrument(skip(self))]
    async fn get_object(
        &self,
        request: Request<GetObjectRequest>,
    ) -> Result<Response<GetObjectResponse>, Status> {
        let req = request.into_inner();
        let object_id = Self::parse_object_id(&req.object_id)?;

        match self.repository.get_object(object_id).await {
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
        let object_id = Self::parse_object_id(&req.object_id)?;

        match self.repository.get_edge(object_id, &req.edge).await {
            Ok(Some(edge)) => {
                // Get the target object
                match self.repository.get_object(edge.to_id).await {
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
        let object_id = Self::parse_object_id(&req.object_id)?;

        match self.repository.get_edges(object_id, &req.edge).await {
            Ok(edges) => {
                let mut objects = Vec::new();
                for edge in edges {
                    match self.repository.get_object(edge.to_id).await {
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
                Ok(Response::new(GetEdgesResponse { object: objects }))
            }
            Err(e) => {
                tracing::error!("Failed to get edges: {:?}", e);
                Err(Status::internal("Failed to get edges"))
            }
        }
    }

    async fn create_object(
        &self,
        _request: Request<CreateObjectRequest>,
    ) -> Result<Response<CreateObjectResponse>, Status> {
        todo!()
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
        let prost_value = GraphServer::json_value_to_prost_value(original.clone());
        // Convert Protobuf -> JSON
        prost_value_to_json(&prost_value)
    }

    proptest! {
        // Test that any valid JSON value can be converted to protobuf
        #[test]
        fn test_json_conversion_doesnt_panic(value in json_value_strategy()) {
            let _ = GraphServer::json_value_to_prost_value(value);
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
