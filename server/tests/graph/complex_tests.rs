use crate::test_helper::{EntTestBuilder, TestObjects};
use anyhow::Result;

#[tokio::test]
async fn test_complex_scenario() -> Result<()> {
    let (address, _pool, _container) = crate::common::spawn_app().await?;

    let mut builder = EntTestBuilder::new()
        .with_basic_schema()
        .with_user("user1")
        .with_user("user2");

    // Create two connected objects and verify the connection
    let (obj1, obj2) = builder.create_two_connected_objects(0)?;
    let state = builder.build(address).await?;

    state.assert_object_exists(obj1)?;
    state.assert_object_exists(obj2)?;
    state.assert_edge_exists(obj1, obj2)?;
    state.assert_user_authenticated(0)?;

    Ok(())
}

#[tokio::test]
async fn test_user_interactions() -> Result<()> {
    let (address, _pool, _container) = crate::common::spawn_app().await?;

    let state = EntTestBuilder::new()
        .with_user_schema()
        .with_user("alice")
        .with_user("bob")
        .with_user_object(0) // Alice's profile
        .with_user_object(1) // Bob's profile
        .build(address)
        .await?;

    state.assert_object_exists(0)?;
    state.assert_object_exists(1)?;
    state.assert_user_authenticated(0)?;
    state.assert_user_authenticated(1)?;

    Ok(())
}

#[tokio::test]
async fn test_complex_schema_and_debugging() -> Result<()> {
    let (address, _pool, _container) = crate::common::spawn_app().await?;

    let builder = EntTestBuilder::new()
        .with_complex_schema()
        .with_user("debug_user");

    // Log the initial state
    println!("Initial state:\n{}", builder.debug_state());

    let state = builder
        .with_object(0, "complex_type", TestObjects::new().complex_object)
        .build(address)
        .await?;

    state.assert_object_exists(0)?;
    state.assert_user_authenticated(0)?;

    Ok(())
}
