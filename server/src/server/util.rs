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

pub fn prost_value_to_json_value(prost_value: ProstValue) -> JsonValue {
    match prost_value.kind {
        Some(prost_types::value::Kind::NullValue(_)) => JsonValue::Null,

        Some(prost_types::value::Kind::BoolValue(b)) => JsonValue::Bool(b),

        Some(prost_types::value::Kind::NumberValue(n)) => {
            // Handle conversion of float to appropriate JSON number representation
            if n.fract() == 0.0 && n <= i64::MAX as f64 && n >= i64::MIN as f64 {
                // Convert to integer if it's a whole number within i64 range
                JsonValue::Number(serde_json::Number::from(n as i64))
            } else {
                // Otherwise keep as floating point
                match serde_json::Number::from_f64(n) {
                    Some(num) => JsonValue::Number(num),
                    None => JsonValue::Null, // Handle invalid numbers like infinity/NaN
                }
            }
        }

        Some(prost_types::value::Kind::StringValue(s)) => JsonValue::String(s),

        Some(prost_types::value::Kind::ListValue(list)) => JsonValue::Array(
            list.values
                .into_iter()
                .map(prost_value_to_json_value)
                .collect(),
        ),

        Some(prost_types::value::Kind::StructValue(obj)) => {
            let mut map = serde_json::Map::new();
            for (key, value) in obj.fields {
                map.insert(key, prost_value_to_json_value(value));
            }
            JsonValue::Object(map)
        }

        None => JsonValue::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_basic_values() {
        // Test null
        let prost_null = ProstValue {
            kind: Some(prost_types::value::Kind::NullValue(0)),
        };
        assert_eq!(prost_value_to_json_value(prost_null), JsonValue::Null);

        // Test boolean
        let prost_bool = ProstValue {
            kind: Some(prost_types::value::Kind::BoolValue(true)),
        };
        assert_eq!(prost_value_to_json_value(prost_bool), JsonValue::Bool(true));

        // Test string
        let prost_string = ProstValue {
            kind: Some(prost_types::value::Kind::StringValue("hello".to_string())),
        };
        assert_eq!(
            prost_value_to_json_value(prost_string),
            JsonValue::String("hello".to_string())
        );
    }

    #[test]
    fn test_numbers() {
        // Test integer
        let prost_int = ProstValue {
            kind: Some(prost_types::value::Kind::NumberValue(42.0)),
        };
        assert_eq!(prost_value_to_json_value(prost_int), json!(42));

        // Test float
        let prost_float = ProstValue {
            kind: Some(prost_types::value::Kind::NumberValue(3.14)),
        };
        assert_eq!(prost_value_to_json_value(prost_float), json!(3.14));

        // Test large integer
        let prost_large = ProstValue {
            kind: Some(prost_types::value::Kind::NumberValue(1e20)),
        };
        assert_eq!(prost_value_to_json_value(prost_large), json!(1e20));
    }

    #[test]
    fn test_arrays() {
        let prost_array = ProstValue {
            kind: Some(prost_types::value::Kind::ListValue(
                prost_types::ListValue {
                    values: vec![
                        ProstValue {
                            kind: Some(prost_types::value::Kind::NumberValue(1.0)),
                        },
                        ProstValue {
                            kind: Some(prost_types::value::Kind::StringValue("test".to_string())),
                        },
                    ],
                },
            )),
        };
        assert_eq!(prost_value_to_json_value(prost_array), json!([1, "test"]));
    }

    #[test]
    fn test_objects() {
        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "key".to_string(),
            ProstValue {
                kind: Some(prost_types::value::Kind::StringValue("value".to_string())),
            },
        );

        let prost_object = ProstValue {
            kind: Some(prost_types::value::Kind::StructValue(prost_types::Struct {
                fields,
            })),
        };

        assert_eq!(
            prost_value_to_json_value(prost_object),
            json!({"key": "value"})
        );
    }

    #[test]
    fn test_nested_structures() {
        let mut inner_fields = std::collections::BTreeMap::new();
        inner_fields.insert(
            "inner_key".to_string(),
            ProstValue {
                kind: Some(prost_types::value::Kind::NumberValue(42.0)),
            },
        );

        let mut outer_fields = std::collections::BTreeMap::new();
        outer_fields.insert(
            "outer_key".to_string(),
            ProstValue {
                kind: Some(prost_types::value::Kind::StructValue(prost_types::Struct {
                    fields: inner_fields,
                })),
            },
        );

        let nested = ProstValue {
            kind: Some(prost_types::value::Kind::StructValue(prost_types::Struct {
                fields: outer_fields,
            })),
        };

        assert_eq!(
            prost_value_to_json_value(nested),
            json!({"outer_key": {"inner_key": 42}})
        );
    }

    #[test]
    fn test_invalid_numbers() {
        let prost_infinity = ProstValue {
            kind: Some(prost_types::value::Kind::NumberValue(f64::INFINITY)),
        };
        assert_eq!(prost_value_to_json_value(prost_infinity), JsonValue::Null);

        let prost_nan = ProstValue {
            kind: Some(prost_types::value::Kind::NumberValue(f64::NAN)),
        };
        assert_eq!(prost_value_to_json_value(prost_nan), JsonValue::Null);
    }
}
