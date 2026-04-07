use rusqlite::{params, OptionalExtension};

use crate::db::sqlite::Database;
use crate::error::{AppError, AppResult};
use crate::session::logging::record_structured_log;

pub fn rewrite_message(
    db: &Database,
    session_id: &str,
    message_id: &str,
    content: &str,
) -> AppResult<()> {
    let mut connection = db.connection()?;
    let transaction = connection.transaction()?;

    let anchor = load_anchor_message(&transaction, session_id, message_id)?
        .ok_or_else(|| AppError::new("目标消息不存在，或不属于该会话"))?;
    if anchor.role != "user" {
        return Err(AppError::new("仅支持重写用户消息"));
    }

    let rewritten = transaction.execute(
        "UPDATE messages SET content = ?3, tool_calls_json = '[]' WHERE id = ?1 AND session_id = ?2",
        params![message_id, session_id, content],
    )?;
    if rewritten == 0 {
        return Err(AppError::new("消息重写失败"));
    }

    let deleted_count = delete_after_anchor(&transaction, session_id, anchor.rowid, false)?;
    touch_session_updated_at(&transaction, session_id)?;

    transaction.commit()?;

    record_structured_log(
        db,
        "session_rewrite_message",
        serde_json::json!({
            "sessionId": session_id,
            "messageId": message_id,
            "contentLength": content.chars().count(),
            "deletedAfterCount": deleted_count,
        }),
    );

    Ok(())
}

pub fn delete_after_message(
    db: &Database,
    session_id: &str,
    message_id: &str,
    inclusive: bool,
) -> AppResult<()> {
    let mut connection = db.connection()?;
    let transaction = connection.transaction()?;

    let anchor = load_anchor_message(&transaction, session_id, message_id)?
        .ok_or_else(|| AppError::new("目标消息不存在，或不属于该会话"))?;

    let deleted_count = delete_after_anchor(&transaction, session_id, anchor.rowid, inclusive)?;
    touch_session_updated_at(&transaction, session_id)?;

    transaction.commit()?;

    record_structured_log(
        db,
        "session_delete_after_message",
        serde_json::json!({
            "sessionId": session_id,
            "anchorMessageId": message_id,
            "inclusive": inclusive,
            "deletedCount": deleted_count,
        }),
    );

    Ok(())
}

#[derive(Debug)]
struct AnchorMessage {
    rowid: i64,
    role: String,
}

fn load_anchor_message(
    transaction: &rusqlite::Transaction<'_>,
    session_id: &str,
    message_id: &str,
) -> AppResult<Option<AnchorMessage>> {
    transaction
        .query_row(
            "SELECT rowid, role FROM messages WHERE id = ?1 AND session_id = ?2 LIMIT 1",
            params![message_id, session_id],
            |row| {
                Ok(AnchorMessage {
                    rowid: row.get(0)?,
                    role: row.get(1)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
}

fn delete_after_anchor(
    transaction: &rusqlite::Transaction<'_>,
    session_id: &str,
    anchor_rowid: i64,
    inclusive: bool,
) -> AppResult<usize> {
    if inclusive {
        return transaction
            .execute(
                "DELETE FROM messages WHERE session_id = ?1 AND rowid >= ?2",
                params![session_id, anchor_rowid],
            )
            .map_err(Into::into);
    }

    transaction
        .execute(
            "DELETE FROM messages WHERE session_id = ?1 AND rowid > ?2",
            params![session_id, anchor_rowid],
        )
        .map_err(Into::into)
}

fn touch_session_updated_at(
    transaction: &rusqlite::Transaction<'_>,
    session_id: &str,
) -> AppResult<()> {
    let updated = transaction.execute(
        "UPDATE sessions SET updated_at = ?2 WHERE id = ?1",
        params![session_id, chrono::Utc::now().to_rfc3339()],
    )?;
    if updated == 0 {
        return Err(AppError::new(format!("会话不存在: {session_id}")));
    }
    Ok(())
}
