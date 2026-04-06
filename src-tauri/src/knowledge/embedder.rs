use crate::error::AppResult;

pub trait Embedder: Send + Sync {
    fn embed(&self, text: &str) -> AppResult<Vec<f32>>;
}

#[derive(Debug, Clone, Default)]
pub struct LocalHashEmbedder {
    dims: usize,
}

impl LocalHashEmbedder {
    pub fn new(dims: usize) -> Self {
        Self { dims }
    }
}

impl Embedder for LocalHashEmbedder {
    fn embed(&self, text: &str) -> AppResult<Vec<f32>> {
        let dims = self.dims.max(32);
        let mut vector = vec![0.0f32; dims];
        for token in text.split_whitespace() {
            let mut hash = 0xcbf29ce484222325u64;
            for byte in token.as_bytes() {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x100000001b3);
            }
            let index = (hash as usize) % dims;
            vector[index] += 1.0;
        }

        let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
        if norm > 0.0 {
            for value in &mut vector {
                *value /= norm;
            }
        }
        Ok(vector)
    }
}

pub struct ApiEmbedder {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl ApiEmbedder {
    pub fn from_env() -> Option<Self> {
        let base_url = std::env::var("EMBEDDING_API_BASE").ok()?;
        let api_key = std::env::var("EMBEDDING_API_KEY").ok()?;
        let model = std::env::var("EMBEDDING_MODEL").ok()?;
        Some(Self {
            client: reqwest::Client::new(),
            base_url,
            api_key,
            model,
        })
    }
}

impl Embedder for ApiEmbedder {
    fn embed(&self, text: &str) -> AppResult<Vec<f32>> {
        block_on_embed_request(self.client.clone(), self.base_url.clone(), self.api_key.clone(), self.model.clone(), text.to_string())
    }
}

fn block_on_embed_request(
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
    text: String,
) -> AppResult<Vec<f32>> {
    let future = async move {
        let url = format!("{}/embeddings", base_url.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": model,
            "input": text
        });
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| crate::error::AppError::new(e.to_string()))?;
        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| crate::error::AppError::new(e.to_string()))?;
        let embedding = json["data"][0]["embedding"]
            .as_array()
            .ok_or_else(|| crate::error::AppError::new("embedding response missing data[0].embedding"))?
            .iter()
            .filter_map(|v| v.as_f64().map(|f| f as f32))
            .collect();
        Ok(embedding)
    };

    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        tokio::task::block_in_place(|| handle.block_on(future))
    } else {
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|error| crate::error::AppError::new(error.to_string()))?;
        runtime.block_on(future)
    }
}

pub fn create_embedder() -> Box<dyn Embedder> {
    match ApiEmbedder::from_env() {
        Some(api) => Box::new(api),
        None => Box::new(LocalHashEmbedder::new(256)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_fixed_size_vector() {
        let embedder = LocalHashEmbedder::new(64);
        let vector = embedder
            .embed("agent loop prompt tool")
            .expect("embedding should succeed");
        assert_eq!(vector.len(), 64);
    }
}
