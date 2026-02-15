mod api;
mod db;
mod domain;
mod infra;

use std::net::SocketAddr;

use axum::Router;
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://xphoto.db".to_string());
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    db::init_schema(&pool).await?;

    let app: Router = api::router(pool);
    let addr: SocketAddr = bind_addr.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;

    println!("xphoto service listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}
