use rusqlite::params;

use crate::db::sqlite::Database;
use crate::error::AppResult;

pub fn rewrite_message(
    db: &Database,
    session_id: &str,
    message_id: &str,
    content: &str,
) -> AppResult<()> {
    let connection = db.connection()?;
    connection.execute(
        "UPDATE messages SET content = ?2, tool_calls_json = '[]' WHERE id = ?1",
        params![message_id, content],
    )?;
    delete_after_message(db, session_id, message_id, false)
}

pub fn delete_after_message(
    db: &Database,
    session_id: &str,
    message_id: &str,
    inclusive: bool,
) -> AppResult<()> {
    let connection = db.connection()?;
    let operator = if inclusive { ">=" } else { ">" };
    let sql = format!(
        "DELETE FROM messages WHERE session_id = ?1 AND created_at {operator} (SELECT created_at FROM messages WHERE id = ?2 LIMIT 1)"
    );
    connection.execute(&sql, params![session_id, message_id])?;
    connection.execute(
        "UPDATE sessions SET updated_at = ?2 WHERE id = ?1",
        params![session_id, chrono::Utc::now().to_rfc3339()],
    )?;
    Ok(())
}
