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
