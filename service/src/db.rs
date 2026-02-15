use sqlx::SqlitePool;

const INIT_SQL: &str = include_str!("../migrations/20260215000100_init.sql");

pub async fn init_schema(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    for statement in INIT_SQL.split(';') {
        let sql = statement.trim();
        if sql.is_empty() {
            continue;
        }
        sqlx::query(sql).execute(pool).await?;
    }
    Ok(())
}
