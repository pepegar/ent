use crate::test_helper::EntTestBuilder;
use anyhow::Result;

#[tokio::test]
async fn test_create_schema() -> Result<()> {
    let (address, _pool, _container) = crate::common::spawn_app().await?;

    let state = EntTestBuilder::new()
        .with_basic_schema()
        .with_user("test_user")
        .with_basic_object(0)
        .build(address)
        .await?;

    assert!(state.get_object(0).is_some());
    Ok(())
}

#[tokio::test]
async fn test_invalid_schema() -> Result<()> {
    let (address, _pool, _container) = crate::common::spawn_app().await?;

    let builder = EntTestBuilder::new().with_schema("{ invalid json }");

    let result = builder.try_create_schema(address).await;
    assert!(result.is_err());

    Ok(())
}
