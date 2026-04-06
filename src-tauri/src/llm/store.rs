use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use crate::db::sqlite::Database;
use crate::error::{AppError, AppResult};

use super::model::{
    model_context_window, ProviderConfigInput, ProviderExtra, ProviderRecord, ProviderSummary,
    ProviderType,
};

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

    pub fn resolve_context_window_for_default(&self, model: &str) -> AppResult<usize> {
        let provider = self.get_default()?;
        self.resolve_context_window(provider.as_ref(), model)
    }

    pub fn resolve_context_window(
        &self,
        provider: Option<&ProviderRecord>,
        model: &str,
    ) -> AppResult<usize> {
        let normalized = normalize_model_key(model);
        if let Some(provider) = provider {
            if let Some(value) = provider.extra.context_windows.get(&normalized).copied() {
                return Ok(value);
            }
        }

        Ok(model_context_window(model))
    }

    pub async fn resolve_context_window_with_refresh(
        &self,
        provider: Option<&ProviderRecord>,
        model: &str,
    ) -> AppResult<usize> {
        let normalized = normalize_model_key(model);
        if let Some(provider) = provider {
            if let Some(value) = provider.extra.context_windows.get(&normalized).copied() {
                return Ok(value);
            }

            if let Some(discovered) = fetch_context_window_from_provider_async(provider, model).await?
            {
                self.store_context_window(provider, &normalized, discovered)?;
                return Ok(discovered);
            }
        }

        Ok(model_context_window(model))
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
            context_windows: Default::default(),
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

    pub fn ensure_default_from_env(&self) -> AppResult<Option<ProviderSummary>> {
        if !self.list()?.is_empty() {
            return Ok(None);
        }

        let Some(endpoint) = std::env::var("OPENAI_API_BASE")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            return Ok(None);
        };
        let Some(api_key) = std::env::var("OPENAI_API_KEY")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            return Ok(None);
        };
        let model = std::env::var("OPENAI_API_MODEL")
            .ok()
            .or_else(|| std::env::var("OPENAI_MODEL").ok())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "gpt-5.4-mini".to_string());

        let created = self.create(ProviderConfigInput {
            name: "OpenAI Default".into(),
            provider_type: ProviderType::OpenAiCompatible,
            endpoint,
            api_key: Some(api_key),
            model: model.clone(),
            models: vec![model],
            enabled: true,
            is_default: true,
            headers: Default::default(),
        })?;

        Ok(Some(created))
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

    fn store_context_window(
        &self,
        provider: &ProviderRecord,
        model: &str,
        context_window: usize,
    ) -> AppResult<()> {
        let mut next_extra = provider.extra.clone();
        next_extra
            .context_windows
            .insert(model.to_string(), context_window);

        let connection = self.db.connection()?;
        connection.execute(
            "UPDATE providers SET extra_json = ?2, updated_at = ?3 WHERE id = ?1",
            params![
                provider.id,
                serde_json::to_string(&next_extra)?,
                chrono::Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }
}

fn normalize_model_key(model: &str) -> String {
    model.trim().to_ascii_lowercase()
}

async fn fetch_context_window_from_provider_async(
    provider: &ProviderRecord,
    model: &str,
) -> AppResult<Option<usize>> {
    if provider.provider_type != ProviderType::OpenAiCompatible {
        return Ok(None);
    }

    let endpoint = normalize_models_endpoint(&provider.endpoint);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|error| AppError::new(error.to_string()))?;

    let mut request = client.get(endpoint);
    if let Some(api_key) = provider
        .api_key
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        request = request.bearer_auth(api_key.trim());
    }
    for (key, value) in &provider.extra.headers {
        request = request.header(key, value);
    }

    let response = match request.send().await {
        Ok(response) => response,
        Err(_) => return Ok(None),
    };
    let payload: serde_json::Value = match response.error_for_status() {
        Ok(response) => match response.json().await {
            Ok(json) => json,
            Err(_) => return Ok(None),
        },
        Err(_) => return Ok(None),
    };

    Ok(extract_context_window_from_models_payload(&payload, model))
}

fn normalize_models_endpoint(endpoint: &str) -> String {
    let trimmed = endpoint.trim_end_matches('/');
    if trimmed.ends_with("/chat/completions") {
        return format!("{}/models", trimmed.trim_end_matches("/chat/completions"));
    }
    if trimmed.ends_with("/v1") {
        return format!("{trimmed}/models");
    }
    if trimmed.ends_with("/models") {
        return trimmed.to_string();
    }
    format!("{trimmed}/models")
}

fn extract_context_window_from_models_payload(
    payload: &serde_json::Value,
    model: &str,
) -> Option<usize> {
    let target = normalize_model_key(model);
    let candidates = payload
        .get("data")
        .and_then(|value| value.as_array())
        .cloned()
        .or_else(|| payload.as_array().cloned())
        .unwrap_or_default();

    for item in candidates {
        let item_id = item
            .get("id")
            .and_then(|value| value.as_str())
            .map(normalize_model_key)
            .or_else(|| {
                item.get("name")
                    .and_then(|value| value.as_str())
                    .map(normalize_model_key)
            });
        if item_id.as_deref() != Some(target.as_str()) {
            continue;
        }

        if let Some(tokens) = read_context_window_value(&item) {
            return Some(tokens);
        }
    }

    None
}

fn read_context_window_value(item: &serde_json::Value) -> Option<usize> {
    [
        item.get("context_window"),
        item.get("contextWindow"),
        item.get("max_input_tokens"),
        item.get("maxInputTokens"),
        item.get("input_token_limit"),
        item.get("inputTokenLimit"),
        item.get("context_length"),
        item.get("contextLength"),
        item.get("limits")
            .and_then(|value| value.get("context_window")),
        item.get("limits")
            .and_then(|value| value.get("contextWindow")),
    ]
    .into_iter()
    .flatten()
    .find_map(|value| value.as_u64().map(|tokens| tokens as usize))
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

    #[test]
    fn extracts_context_window_from_models_payload() {
        let payload = serde_json::json!({
            "data": [
                {
                    "id": "gpt-5.4-mini",
                    "context_window": 400000
                }
            ]
        });

        assert_eq!(
            extract_context_window_from_models_payload(&payload, "gpt-5.4-mini"),
            Some(400_000)
        );
    }

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
