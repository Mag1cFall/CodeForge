use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use crate::db::sqlite::Database;
use crate::error::{AppError, AppResult};

use super::model::{ProviderConfigInput, ProviderExtra, ProviderRecord, ProviderSummary};

#[derive(Debug, Clone)]
pub struct ProviderStore {
    db: Database,
}

impl ProviderStore {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn list(&self) -> AppResult<Vec<ProviderSummary>> {
        let connection = self.db.connection()?;
        let mut statement = connection.prepare(
            r#"
            SELECT id, name, provider_type, endpoint, api_key, model, extra_json,
                   enabled, is_default, created_at, updated_at
            FROM providers
            ORDER BY is_default DESC, updated_at DESC
            "#,
        )?;

        let rows = statement.query_map([], |row| {
            let extra_json: String = row.get(6)?;
            let extra = serde_json::from_str::<ProviderExtra>(&extra_json).unwrap_or_default();

            Ok(ProviderRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                provider_type: serde_json::from_str(&format!("\"{}\"", row.get::<_, String>(2)?))
                    .map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?,
                endpoint: row.get(3)?,
                api_key: row.get(4)?,
                model: row.get(5)?,
                extra,
                enabled: row.get::<_, i64>(7)? != 0,
                is_default: row.get::<_, i64>(8)? != 0,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?;

        let mut providers = Vec::new();
        for row in rows {
            let record = row?;
            providers.push(ProviderSummary::from(&record));
        }
        Ok(providers)
    }

    pub fn get_default(&self) -> AppResult<Option<ProviderRecord>> {
        self.get_by_selector("is_default = 1 ORDER BY updated_at DESC LIMIT 1")
    }

    pub fn get_by_id(&self, id: &str) -> AppResult<Option<ProviderRecord>> {
        let connection = self.db.connection()?;
        let mut statement = connection.prepare(
            r#"
            SELECT id, name, provider_type, endpoint, api_key, model, extra_json,
                   enabled, is_default, created_at, updated_at
            FROM providers
            WHERE id = ?1
            LIMIT 1
            "#,
        )?;

        let record = statement
            .query_row(params![id], |row| {
                let extra_json: String = row.get(6)?;
                let extra = serde_json::from_str::<ProviderExtra>(&extra_json).unwrap_or_default();
                let provider_type: String = row.get(2)?;
                let provider_type = serde_json::from_str(&format!("\"{}\"", provider_type))
                    .map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            2,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;

                Ok(ProviderRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    provider_type,
                    endpoint: row.get(3)?,
                    api_key: row.get(4)?,
                    model: row.get(5)?,
                    extra,
                    enabled: row.get::<_, i64>(7)? != 0,
                    is_default: row.get::<_, i64>(8)? != 0,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })
            .optional()?;

        Ok(record)
    }

    pub fn create(&self, input: ProviderConfigInput) -> AppResult<ProviderSummary> {
        let name = input.name.trim();
        let endpoint = input.endpoint.trim();
        let model = input.model.trim();
        if name.is_empty() || endpoint.is_empty() || model.is_empty() {
            return Err(AppError::new("Provider 名称、端点与默认模型不能为空"));
        }

        let now = chrono::Utc::now().to_rfc3339();
        let id = Uuid::new_v4().to_string();
        let extra = ProviderExtra {
            models: input
                .models
                .into_iter()
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect(),
            headers: input.headers,
        };

        let mut connection = self.db.connection()?;
        let tx = connection.transaction()?;

        let existing_count: i64 =
            tx.query_row("SELECT COUNT(*) FROM providers", [], |row| row.get(0))?;
        let make_default = input.is_default || existing_count == 0;

        if make_default {
            tx.execute("UPDATE providers SET is_default = 0", [])?;
        }

        tx.execute(
            r#"
            INSERT INTO providers (
                id, name, provider_type, endpoint, api_key, model, extra_json,
                enabled, is_default, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
            params![
                id,
                name,
                serde_json::to_string(&input.provider_type)?
                    .trim_matches('"')
                    .to_string(),
                endpoint,
                input.api_key.filter(|value| !value.trim().is_empty()),
                model,
                serde_json::to_string(&extra)?,
                bool_to_int(input.enabled),
                bool_to_int(make_default),
                now,
                now,
            ],
        )?;
        tx.commit()?;

        let record = self
            .get_by_id(&id)?
            .ok_or_else(|| AppError::new("新建 Provider 后未能读取记录"))?;
        Ok(ProviderSummary::from(&record))
    }

    pub fn delete(&self, id: &str) -> AppResult<()> {
        let provider = self
            .get_by_id(id)?
            .ok_or_else(|| AppError::new("指定 Provider 不存在"))?;

        let mut connection = self.db.connection()?;
        let tx = connection.transaction()?;
        tx.execute("DELETE FROM providers WHERE id = ?1", params![id])?;

        if provider.is_default {
            let replacement = tx
                .query_row(
                    "SELECT id FROM providers ORDER BY updated_at DESC LIMIT 1",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;

            if let Some(replacement) = replacement {
                tx.execute(
                    "UPDATE providers SET is_default = 1, updated_at = ?2 WHERE id = ?1",
                    params![replacement, chrono::Utc::now().to_rfc3339()],
                )?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    fn get_by_selector(&self, selector_sql: &str) -> AppResult<Option<ProviderRecord>> {
        let connection = self.db.connection()?;
        let sql = format!(
            "SELECT id, name, provider_type, endpoint, api_key, model, extra_json, enabled, is_default, created_at, updated_at FROM providers WHERE {selector_sql}"
        );

        let mut statement = connection.prepare(&sql)?;
        let record = statement
            .query_row([], |row| {
                let extra_json: String = row.get(6)?;
                let extra = serde_json::from_str::<ProviderExtra>(&extra_json).unwrap_or_default();
                let provider_type: String = row.get(2)?;
                let provider_type = serde_json::from_str(&format!("\"{}\"", provider_type))
                    .map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            2,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;

                Ok(ProviderRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    provider_type,
                    endpoint: row.get(3)?,
                    api_key: row.get(4)?,
                    model: row.get(5)?,
                    extra,
                    enabled: row.get::<_, i64>(7)? != 0,
                    is_default: row.get::<_, i64>(8)? != 0,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })
            .optional()?;

        Ok(record)
    }
}

fn bool_to_int(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::db::sqlite::Database;

    use super::*;
    use crate::llm::model::{ProviderConfigInput, ProviderType};

    fn temp_db_path() -> PathBuf {
        std::env::temp_dir().join(format!("codeforge-provider-{}.db", Uuid::new_v4()))
    }

    #[test]
    fn creates_lists_and_deletes_provider() {
        let db_path = temp_db_path();
        let db = Database::new(&db_path).expect("db should initialize");
        let store = ProviderStore::new(db);

        let created = store
            .create(ProviderConfigInput {
                name: "OpenAI".into(),
                provider_type: ProviderType::OpenAiCompatible,
                endpoint: "https://example.com/v1/chat/completions".into(),
                api_key: Some("secret".into()),
                model: "gpt-5.4-mini".into(),
                models: vec!["gpt-5.4-mini".into()],
                enabled: true,
                is_default: true,
                headers: Default::default(),
            })
            .expect("provider should be created");

        let list = store.list().expect("provider list should load");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, created.id);
        assert!(list[0].key_set);
        assert!(list[0].is_default);

        store
            .delete(&created.id)
            .expect("provider should be deleted");
        assert!(store.list().expect("provider list should load").is_empty());

        let _ = std::fs::remove_file(db_path);
    }
}
