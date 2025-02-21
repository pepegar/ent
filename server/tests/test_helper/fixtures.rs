use serde_json::json;

pub struct TestSchemas {
    pub basic_schema: &'static str,
    pub user_schema: &'static str,
    pub complex_schema: &'static str,
}

impl TestSchemas {
    pub fn new() -> Self {
        Self {
            basic_schema: r#"{
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "age": { "type": "number" }
                }
            }"#,
            user_schema: r#"{
                "type": "object",
                "required": ["username", "email"],
                "properties": {
                    "username": { "type": "string" },
                    "email": { "type": "string", "format": "email" }
                }
            }"#,
            complex_schema: r#"{
                "type": "object",
                "required": ["id", "metadata"],
                "properties": {
                    "id": { "type": "string" },
                    "metadata": {
                        "type": "object",
                        "properties": {
                            "tags": { "type": "array", "items": { "type": "string" } }
                        }
                    }
                }
            }"#,
        }
    }
}

pub struct TestObjects {
    pub basic_object: serde_json::Value,
    pub user_object: serde_json::Value,
    pub complex_object: serde_json::Value,
}

impl TestObjects {
    pub fn new() -> Self {
        Self {
            basic_object: json!({
                "name": "Test Object",
                "age": 25
            }),
            user_object: json!({
                "username": "testuser",
                "email": "test@example.com"
            }),
            complex_object: json!({
                "id": "complex-123",
                "metadata": {
                    "tags": ["sample", "test", "fixture"]
                }
            }),
        }
    }
}
