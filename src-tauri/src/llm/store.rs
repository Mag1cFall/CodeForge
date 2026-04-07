use std::collections::BTreeSet;
use std::time::Instant;

use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use crate::db::sqlite::Database;
use crate::error::{AppError, AppResult};

use super::model::{
    model_context_window, ProviderConfigInput, ProviderExtra, ProviderRecord, ProviderSummary,
    ProviderType,
};
use super::telemetry::log_event;

const MODELS_FETCH_RETRY_DELAYS_MS: [u64; 3] = [0, 300, 1000];

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
                log_event(
                    "provider_store",
                    "context_window_hit_cache",
                    serde_json::json!({
                        "providerId": provider.id.as_str(),
                        "providerType": provider.provider_type.as_str(),
                        "model": normalized.as_str(),
                        "contextWindow": value,
                    }),
                );
                return Ok(value);
            }

            if let Some(discovered) =
                fetch_context_window_from_provider_async(provider, model).await?
            {
                self.store_context_window(provider, &normalized, discovered)?;
                log_event(
                    "provider_store",
                    "context_window_discovered",
                    serde_json::json!({
                        "providerId": provider.id.as_str(),
                        "providerType": provider.provider_type.as_str(),
                        "model": normalized.as_str(),
                        "contextWindow": discovered,
                    }),
                );
                return Ok(discovered);
            }

            let fallback = model_context_window(model);
            log_event(
                "provider_store",
                "context_window_fallback",
                serde_json::json!({
                    "providerId": provider.id.as_str(),
                    "providerType": provider.provider_type.as_str(),
                    "model": normalized.as_str(),
                    "contextWindow": fallback,
                    "reason": "provider_models_endpoint_unavailable",
                }),
            );
            return Ok(fallback);
        }

        let fallback = model_context_window(model);
        log_event(
            "provider_store",
            "context_window_fallback",
            serde_json::json!({
                "providerId": null,
                "model": normalized.as_str(),
                "contextWindow": fallback,
                "reason": "no_provider",
            }),
        );
        Ok(fallback)
    }

    pub fn create(&self, input: ProviderConfigInput) -> AppResult<ProviderSummary> {
        let name = input.name.trim();
        let endpoint = input.endpoint.trim();
        if name.is_empty() || endpoint.is_empty() {
            return Err(AppError::new("Provider 名称与端点不能为空"));
        }

        let (default_model, normalized_models) = normalize_provider_models(&input.model, &input.models)?;

        let now = chrono::Utc::now().to_rfc3339();
        let id = Uuid::new_v4().to_string();
        let extra = ProviderExtra {
            models: normalized_models,
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
                default_model,
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
        log_event(
            "provider_store",
            "provider_created",
            serde_json::json!({
                "providerId": record.id.as_str(),
                "providerType": record.provider_type.as_str(),
                "name": record.name.as_str(),
                "endpoint": record.endpoint.as_str(),
                "default": record.is_default,
                "enabled": record.enabled,
                "model": record.model.as_str(),
                "models": record.extra.models.clone(),
            }),
        );
        Ok(ProviderSummary::from(&record))
    }

    pub fn update(&self, id: &str, input: ProviderConfigInput) -> AppResult<ProviderSummary> {
        let existing = self
            .get_by_id(id)?
            .ok_or_else(|| AppError::new("指定 Provider 不存在"))?;

        let name = input.name.trim();
        let endpoint = input.endpoint.trim();
        if name.is_empty() || endpoint.is_empty() {
            return Err(AppError::new("Provider 名称与端点不能为空"));
        }

        let (default_model, normalized_models) =
            normalize_provider_models(&input.model, &input.models)?;
        let now = chrono::Utc::now().to_rfc3339();

        let mut context_windows = existing.extra.context_windows.clone();
        if !existing.endpoint.eq_ignore_ascii_case(endpoint) {
            context_windows.clear();
        }
        let extra = ProviderExtra {
            models: normalized_models,
            headers: input.headers,
            context_windows,
        };

        let make_default = existing.is_default || input.is_default;

        let mut connection = self.db.connection()?;
        let tx = connection.transaction()?;

        if make_default {
            tx.execute("UPDATE providers SET is_default = 0", [])?;
        }

        tx.execute(
            r#"
            UPDATE providers
            SET name = ?2,
                provider_type = ?3,
                endpoint = ?4,
                api_key = ?5,
                model = ?6,
                extra_json = ?7,
                enabled = ?8,
                is_default = ?9,
                updated_at = ?10
            WHERE id = ?1
            "#,
            params![
                id,
                name,
                serde_json::to_string(&input.provider_type)?
                    .trim_matches('"')
                    .to_string(),
                endpoint,
                input.api_key.filter(|value| !value.trim().is_empty()),
                default_model,
                serde_json::to_string(&extra)?,
                bool_to_int(input.enabled),
                bool_to_int(make_default),
                now,
            ],
        )?;

        tx.commit()?;

        let record = self
            .get_by_id(id)?
            .ok_or_else(|| AppError::new("更新 Provider 后未能读取记录"))?;
        log_event(
            "provider_store",
            "provider_updated",
            serde_json::json!({
                "providerId": record.id.as_str(),
                "providerType": record.provider_type.as_str(),
                "name": record.name.as_str(),
                "endpoint": record.endpoint.as_str(),
                "default": record.is_default,
                "enabled": record.enabled,
                "model": record.model.as_str(),
                "models": record.extra.models.clone(),
            }),
        );
        Ok(ProviderSummary::from(&record))
    }

    pub async fn fetch_models_preview(
        &self,
        provider_type: ProviderType,
        endpoint: &str,
        api_key: Option<&str>,
        headers: &std::collections::BTreeMap<String, String>,
    ) -> AppResult<Vec<String>> {
        if provider_type != ProviderType::OpenAiCompatible {
            return Ok(Vec::new());
        }

        let endpoint = normalize_models_endpoint(endpoint);
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|error| AppError::new(error.to_string()))?;

        let mut request = client
            .get(endpoint.as_str())
            .header(reqwest::header::ACCEPT, "application/json");
        if let Some(api_key) = api_key.filter(|value| !value.trim().is_empty()) {
            request = request.bearer_auth(api_key.trim());
        }
        for (key, value) in headers {
            request = request.header(key, value);
        }

        let response = request.send().await?;
        let status = response.status();
        if !status.is_success() {
            let body_preview = truncate_for_log(&response.text().await.unwrap_or_default());
            return Err(AppError::new(format!(
                "拉取模型列表失败: status={} body={}",
                status.as_u16(),
                body_preview
            )));
        }

        let payload = response.json::<serde_json::Value>().await?;
        let candidates = payload
            .get("data")
            .and_then(|value| value.as_array())
            .cloned()
            .or_else(|| payload.as_array().cloned())
            .unwrap_or_default();

        let mut models = Vec::new();
        let mut dedupe = BTreeSet::new();
        for item in candidates {
            let Some(model) = item
                .get("id")
                .and_then(|value| value.as_str())
                .or_else(|| item.get("name").and_then(|value| value.as_str()))
                .or_else(|| item.get("model").and_then(|value| value.as_str()))
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
            else {
                continue;
            };

            if dedupe.insert(model.clone()) {
                models.push(model);
            }
        }

        Ok(models)
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

        log_event(
            "provider_store",
            "provider_created_from_env",
            serde_json::json!({
                "providerId": created.id.as_str(),
                "providerType": created.provider_type.as_str(),
                "model": created.model.as_str(),
                "endpoint": created.endpoint.as_str(),
            }),
        );

        Ok(Some(created))
    }

    pub fn delete(&self, id: &str) -> AppResult<()> {
        let provider = self
            .get_by_id(id)?
            .ok_or_else(|| AppError::new("指定 Provider 不存在"))?;

        let mut connection = self.db.connection()?;
        let tx = connection.transaction()?;
        tx.execute("DELETE FROM providers WHERE id = ?1", params![id])?;
        let mut replacement_assigned = false;

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
                replacement_assigned = true;
            }
        }

        tx.commit()?;
        log_event(
            "provider_store",
            "provider_deleted",
            serde_json::json!({
                "providerId": provider.id.as_str(),
                "wasDefault": provider.is_default,
                "replacementAssigned": replacement_assigned,
            }),
        );
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
        log_event(
            "provider_store",
            "context_window_persisted",
            serde_json::json!({
                "providerId": provider.id.as_str(),
                "model": model,
                "contextWindow": context_window,
            }),
        );
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
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|error| AppError::new(error.to_string()))?;

    for (attempt, delay_ms) in MODELS_FETCH_RETRY_DELAYS_MS.iter().enumerate() {
        if *delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(*delay_ms)).await;
        }

        let started_at = Instant::now();
        log_event(
            "provider_store",
            "context_window_probe_attempt",
            serde_json::json!({
                "providerId": provider.id.as_str(),
                "attempt": attempt + 1,
                "maxAttempts": MODELS_FETCH_RETRY_DELAYS_MS.len(),
                "endpoint": endpoint.as_str(),
                "model": model,
            }),
        );

        let mut request = client
            .get(endpoint.as_str())
            .header(reqwest::header::ACCEPT, "application/json");
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
            Err(error) => {
                log_event(
                    "provider_store",
                    "context_window_probe_network_error",
                    serde_json::json!({
                        "providerId": provider.id.as_str(),
                        "attempt": attempt + 1,
                        "elapsedMs": started_at.elapsed().as_millis() as u64,
                        "error": error.to_string(),
                    }),
                );
                continue;
            }
        };

        let status = response.status();
        if !status.is_success() {
            let should_retry = status.is_server_error() || status.as_u16() == 429;
            let body_preview = truncate_for_log(&response.text().await.unwrap_or_default());
            log_event(
                "provider_store",
                "context_window_probe_http_error",
                serde_json::json!({
                    "providerId": provider.id.as_str(),
                    "attempt": attempt + 1,
                    "elapsedMs": started_at.elapsed().as_millis() as u64,
                    "status": status.as_u16(),
                    "retry": should_retry,
                    "bodyPreview": body_preview,
                }),
            );
            if should_retry {
                continue;
            }
            return Ok(None);
        }

        let payload = match response.json::<serde_json::Value>().await {
            Ok(payload) => payload,
            Err(error) => {
                log_event(
                    "provider_store",
                    "context_window_probe_parse_error",
                    serde_json::json!({
                        "providerId": provider.id.as_str(),
                        "attempt": attempt + 1,
                        "elapsedMs": started_at.elapsed().as_millis() as u64,
                        "error": error.to_string(),
                    }),
                );
                continue;
            }
        };

        let resolved = extract_context_window_from_models_payload(&payload, model);
        log_event(
            "provider_store",
            "context_window_probe_finished",
            serde_json::json!({
                "providerId": provider.id.as_str(),
                "attempt": attempt + 1,
                "elapsedMs": started_at.elapsed().as_millis() as u64,
                "resolved": resolved,
            }),
        );

        if resolved.is_some() {
            return Ok(resolved);
        }
    }

    Ok(None)
}

fn normalize_models_endpoint(endpoint: &str) -> String {
    let trimmed = endpoint.trim_end_matches('/');
    if trimmed.ends_with("/chat/completions") {
        return format!("{}/models", trimmed.trim_end_matches("/chat/completions"));
    }
    if trimmed.ends_with("/responses") {
        return format!("{}/models", trimmed.trim_end_matches("/responses"));
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

    let mut fallback_candidate = None;

    for item in candidates {
        let item_id = item
            .get("id")
            .and_then(|value| value.as_str())
            .map(normalize_model_key)
            .or_else(|| {
                item.get("name")
                    .and_then(|value| value.as_str())
                    .map(normalize_model_key)
            })
            .or_else(|| {
                item.get("model")
                    .and_then(|value| value.as_str())
                    .map(normalize_model_key)
            });

        let Some(item_id) = item_id else {
            continue;
        };

        if item_id == target {
            if let Some(tokens) = read_context_window_value(&item) {
                return Some(tokens);
            }
        } else if fallback_candidate.is_none()
            && (item_id.contains(target.as_str()) || target.contains(item_id.as_str()))
        {
            fallback_candidate = read_context_window_value(&item);
        }
    }

    fallback_candidate
}

fn read_context_window_value(item: &serde_json::Value) -> Option<usize> {
    [
        item.get("context_window"),
        item.get("contextWindow"),
        item.get("max_context_length"),
        item.get("maxContextLength"),
        item.get("max_input_tokens"),
        item.get("maxInputTokens"),
        item.get("input_token_limit"),
        item.get("inputTokenLimit"),
        item.get("context_length"),
        item.get("contextLength"),
        item.get("token_limit"),
        item.get("tokenLimit"),
        item.get("model_spec")
            .and_then(|value| value.get("availableContextTokens")),
        item.get("limits")
            .and_then(|value| value.get("max_context_tokens")),
        item.get("limits")
            .and_then(|value| value.get("maxContextTokens")),
        item.get("limits")
            .and_then(|value| value.get("context_window")),
        item.get("limits")
            .and_then(|value| value.get("contextWindow")),
    ]
    .into_iter()
    .flatten()
    .find_map(read_usize_value)
}

fn read_usize_value(value: &serde_json::Value) -> Option<usize> {
    if let Some(value) = value.as_u64() {
        return Some(value as usize);
    }

    value
        .as_str()
        .and_then(|item| item.trim().parse::<usize>().ok())
}

fn truncate_for_log(value: &str) -> String {
    const LIMIT: usize = 512;
    if value.len() <= LIMIT {
        return value.to_string();
    }
    format!("{}...(truncated)", &value[..LIMIT])
}

fn normalize_provider_models(model: &str, models: &[String]) -> AppResult<(String, Vec<String>)> {
    let normalized_default = model.trim().to_string();

    let mut dedupe = BTreeSet::new();
    let mut normalized_models = Vec::new();

    for item in models {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }
        if dedupe.insert(trimmed.to_string()) {
            normalized_models.push(trimmed.to_string());
        }
    }

    if !normalized_default.is_empty() && dedupe.insert(normalized_default.clone()) {
        normalized_models.insert(0, normalized_default.clone());
    }

    let default_model = if normalized_default.is_empty() {
        normalized_models
            .first()
            .cloned()
            .ok_or_else(|| AppError::new("请至少配置一个模型"))?
    } else {
        normalized_default
    };

    if normalized_models.is_empty() {
        return Err(AppError::new("请至少配置一个模型"));
    }

    Ok((default_model, normalized_models))
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
