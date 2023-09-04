#![allow(unreachable_code)]
#![allow(unused_imports)]

use anyhow::bail;
use futures::TryStreamExt;
use sqlx::{postgres::PgPoolOptions, types::Uuid, Pool, Postgres};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Hello, world!");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect("postgres://postgres:password@localhost/example")
        .await?;

    insert_users_transaction(pool.clone()).await?;

    let mut query = sqlx::query!("SELECT * FROM USERS").fetch(&pool);
    while let Some(row) = query.try_next().await? {
        println!(
            "{row:?} {id} {name} {age} {favourite_food}",
            id = row.user_id,
            name = row.name,
            age = row.age,
            favourite_food = row.favourite_food.as_deref().unwrap_or("no food")
        );
    }

    #[derive(Debug)]
    struct User {
        user_id: Uuid,
        name: String,
        age: i64,
        favourite_food: Option<String>,
        favourite_programming_language: Option<String>,
    }

    let query = sqlx::query_as!(User, "SELECT * FROM USERS");
    let mut result = query.fetch(&pool);
    while let Some(row) = result.try_next().await? {
        println!(
            "{row:?} {id} {name} {age} {favourite_food}",
            id = row.user_id,
            name = row.name,
            age = row.age,
            favourite_food = row.favourite_food.as_deref().unwrap_or("no food")
        );
    }

    Ok(())
}

async fn insert_users_transaction(pool: Pool<Postgres>) -> anyhow::Result<()> {
    let mut transaction = pool.begin().await?;

    sqlx::query("DELETE FROM users")
        .execute(transaction.as_mut())
        .await?;

    sqlx::query!("INSERT INTO users (name, age) VALUES ('joseph', 25)")
        .execute(transaction.as_mut())
        .await?;

    sqlx::query("INSERT INTO users (name, age) VALUES ($1, $2)")
        .bind("jason")
        .bind(35)
        .execute(transaction.as_mut())
        .await?;

    // bail!("dont commit records please!");

    sqlx::query!(
        "INSERT INTO users (name, age, favourite_food) VALUES ($1, $2, $3)",
        "david",
        5,
        "pizza"
    )
    .execute(transaction.as_mut())
    .await?;

    transaction.commit().await?;

    Ok(())
}
