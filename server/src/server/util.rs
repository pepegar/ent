use prost_types::{Struct, Value as ProstValue};
use serde_json::Value as JsonValue;

pub fn json_value_to_prost_value(json_value: JsonValue) -> ProstValue {
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
            let values: Vec<ProstValue> = arr.into_iter().map(json_value_to_prost_value).collect();
            ProstValue {
                kind: Some(prost_types::value::Kind::ListValue(
                    prost_types::ListValue { values },
                )),
            }
        }
        JsonValue::Object(map) => {
            let mut fields = std::collections::BTreeMap::new();
            for (k, v) in map {
                fields.insert(k, json_value_to_prost_value(v));
            }
            ProstValue {
                kind: Some(prost_types::value::Kind::StructValue(Struct { fields })),
            }
        }
    }
}
