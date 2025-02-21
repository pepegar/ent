use crate::test_helper::EntTestBuilder;
use anyhow::Result;
use serde_json::json;

#[tokio::test]
async fn test_complex_scenario() -> Result<()> {
    let (address, _pool, _container) = crate::common::spawn_app().await?;

    let type_name = format!("test_type_{}", uuid::Uuid::new_v4());

    let state = EntTestBuilder::new()
        .with_schema(
            r#"{
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            }
        }"#,
        )
        .with_user("user1")
        .with_user("user2")
        .with_object(
            0,
            &type_name,
            json!({
                "name": "Doc 1"
            }),
        )
        .with_object(
            1,
            &type_name,
            json!({
                "name": "Doc 2"
            }),
        )
        .with_edge(
            0,
            0,
            1,
            "references",
            json!({
                "note": "Important reference"
            }),
        )
        .build(address)
        .await?;

    assert!(state.get_object(0).is_some());
    assert!(state.get_user_token(0).is_some());

    Ok(())
}
