use anyhow::{Context, Result};
use pretty_assertions::assert_eq;

use crate::helpers::spawn_app;
use challenges::http::passwords::{UserState, NUM_PASSWORDS};

#[tokio::test]
async fn post_returns_a_200_for_valid_username() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let user = "test_user";

    // Act
    let response = app.post_user(user.into()).await;

    // Assert
    assert_eq!(200, response.status().as_u16());

    Ok(())
}

#[tokio::test]
async fn create_returns_a_200_for_valid_username() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let user = "test_user";

    // Act
    let response = app.post_user(user.into()).await;

    // Assert
    assert_eq!(200, response.status().as_u16());

    Ok(())
}

#[tokio::test]
async fn create_persists_the_new_user() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let user = "test_user";

    // Act
    app.post_user(user.into()).await;

    // Assert
    let saved = sqlx::query!("SELECT username FROM user;")
        .fetch_one(&app.db_pool)
        .await
        .context("Failed to fetch saved user.")?;

    assert_eq!(saved.username, user);

    Ok(())
}

#[tokio::test]
async fn create_fails_if_there_is_a_fatal_database_error() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let user = "test_user";

    // Sabotage the database
    sqlx::query!("ALTER TABLE user DROP COLUMN solved;")
        .execute(&app.db_pool)
        .await
        .context("Failed to drop column.")?;

    // Act
    let response = app.post_user(user.into()).await;

    // Assert
    assert_eq!(response.status().as_u16(), 500);

    Ok(())
}

#[tokio::test]
async fn create_returns_a_400_for_duplicate_username() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let user = "test_user";

    // Act
    app.post_user(user.into()).await;
    let response = app.post_user(user.into()).await;

    // Assert
    assert_eq!(400, response.status().as_u16());

    Ok(())
}

#[tokio::test]
async fn delete_returns_a_200_for_valid_username() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let user = "test_user";

    // Act
    let _ = app.post_user(user.into()).await;
    let response = app.delete_user(user.into()).await;

    // Assert
    assert_eq!(200, response.status().as_u16());

    Ok(())
}

#[tokio::test]
async fn delete_returns_a_404_for_invalid_username() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let user = "test_user";

    // Act
    let response = app.delete_user(user.into()).await;

    // Assert
    assert_eq!(404, response.status().as_u16());

    Ok(())
}

#[tokio::test]
async fn delete_unpersists_the_user() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let user = "test_user";

    // Act
    app.post_user(user.into()).await;
    app.delete_user(user.into()).await;

    // Assert
    let saved = sqlx::query!("SELECT username FROM user;")
        .fetch_all(&app.db_pool)
        .await
        .context("Failed to fetch users.")?;

    assert!(saved.is_empty());

    Ok(())
}

#[tokio::test]
async fn passwords_returns_a_200_for_valid_username() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let user = "test_user";

    // Act
    app.post_user(user.into()).await;
    let response = app.get_passwords(user.into()).await;

    // Assert
    assert_eq!(200, response.status().as_u16());

    Ok(())
}

#[tokio::test]
async fn passwords_returns_a_the_right_number_of_entries() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let user = "test_user";

    // Act
    app.post_user(user.into()).await;
    let response = app.get_passwords(user.into()).await;

    // Assert
    let text = response.text().await?;
    assert_eq!(text.lines().count(), NUM_PASSWORDS);

    Ok(())
}

#[tokio::test]
async fn check_returns_a_200_false_for_bad_pass() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let user = "test_user";

    // Act
    app.post_user(user.into()).await;
    let response = app.check_password(user.into(), "fake".into()).await;

    // Assert
    assert_eq!(200, response.status().as_u16());
    assert_eq!("False", response.text().await?);

    Ok(())
}

#[tokio::test]
async fn check_returns_a_200_true_for_good_pass() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let user = "test_user";

    // Act
    app.post_user(user.into()).await;
    let saved: String = sqlx::query_scalar!("SELECT secret FROM user;")
        .fetch_one(&app.db_pool)
        .await
        .context("Failed to fetch saved user.")?;
    let response = app.check_password(user.into(), saved).await;

    // Assert
    assert_eq!(200, response.status().as_u16());
    assert_eq!("True", response.text().await?);

    Ok(())
}

#[tokio::test]
async fn check_updates_user_for_good_pass() -> Result<()> {
    // Arrange
    let app = spawn_app().await?;
    let user = "test_user";

    // Act
    app.post_user(user.into()).await;
    let saved: String = sqlx::query_scalar!("SELECT secret FROM user;")
        .fetch_one(&app.db_pool)
        .await
        .context("Failed to fetch saved user.")?;
    app.check_password(user.into(), saved).await;
    app.check_password(user.into(), "bad".into()).await;

    let user = sqlx::query_as!(UserState, "SELECT * FROM user;")
        .fetch_one(&app.db_pool)
        .await
        .context("Failed to fetch saved user.")?;

    // Assert
    assert_eq!(1, user.solved);
    assert!(user.solved_at.is_some());
    assert_eq!(2, user.total_hits);
    assert_eq!(1, user.hits_before_solved);

    Ok(())
}
