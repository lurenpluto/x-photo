use sqlx::SqlitePool;
use tracing::{debug, info};

const INIT_SQL: &str = include_str!("../migrations/20260215000100_init.sql");

pub async fn init_schema(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    info!("starting schema initialization");
    let mut statement_count: usize = 0;

    for statement in INIT_SQL.split(';') {
        let sql = statement.trim();
        if sql.is_empty() {
            continue;
        }
        statement_count += 1;
        debug!(statement_index = statement_count, sql = %sql, "executing schema statement");
        sqlx::query(sql).execute(pool).await?;
    }

    info!(statement_count, "schema initialization finished");
    Ok(())
}
