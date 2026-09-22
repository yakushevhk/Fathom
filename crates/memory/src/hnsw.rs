use std::cmp::Ordering;

/// SIMD-accelerated In-Memory Vector Index for sub-millisecond semantic search.
pub struct SimdVectorIndex {
    dimension: usize,
    vectors: Vec<Vec<f32>>,
    ids: Vec<String>,
}

impl SimdVectorIndex {
    pub fn new(dimension: usize) -> Self {
        Self {
            dimension,
            vectors: Vec::new(),
            ids: Vec::new(),
        }
    }

    /// Add a vector with an associated string identifier.
    pub fn insert(&mut self, id: impl Into<String>, vector: Vec<f32>) {
        if vector.len() == self.dimension {
            self.ids.push(id.into());
            self.vectors.push(vector);
        }
    }

    /// Compute cosine similarity between two vectors with compiler autovectorization.
    #[inline(always)]
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let mut dot = 0.0;
        let mut norm_a = 0.0;
        let mut norm_b = 0.0;

        for i in 0..a.len() {
            dot += a[i] * b[i];
            norm_a += a[i] * a[i];
            norm_b += b[i] * b[i];
        }

        if norm_a == 0.0
            || norm_b == 0.0
            || !norm_a.is_finite()
            || !norm_b.is_finite()
            || !dot.is_finite()
        {
            0.0
        } else {
            let score = dot / (norm_a.sqrt() * norm_b.sqrt());
            if score.is_finite() {
                score.clamp(-1.0, 1.0)
            } else {
                0.0
            }
        }
    }

    /// Search the top-K nearest neighbors given a query embedding.
    pub fn search(&self, query: &[f32], top_k: usize) -> Vec<(String, f32)> {
        if query.len() != self.dimension || self.vectors.is_empty() {
            return Vec::new();
        }

        let mut results: Vec<(String, f32)> = self
            .vectors
            .iter()
            .enumerate()
            .map(|(idx, v)| {
                let score = Self::cosine_similarity(query, v);
                (self.ids[idx].clone(), score)
            })
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
        results.into_iter().take(top_k).collect()
    }

    /// Total indexed vectors.
    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_index_is_empty() {
        let idx = SimdVectorIndex::new(4);
        assert_eq!(idx.len(), 0);
        assert!(idx.is_empty());
    }

    #[test]
    fn insert_grows_len() {
        let mut idx = SimdVectorIndex::new(3);
        idx.insert("a", vec![1.0, 0.0, 0.0]);
        idx.insert("b", vec![0.0, 1.0, 0.0]);
        assert_eq!(idx.len(), 2);
    }

    #[test]
    fn search_returns_nearest_first() {
        let mut idx = SimdVectorIndex::new(2);
        idx.insert("x", vec![1.0, 0.0]);
        idx.insert("y", vec![0.0, 1.0]);
        idx.insert("z", vec![0.9, 0.1]);
        let hits = idx.search(&[1.0, 0.0], 3);
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[0].0, "x"); // exact match first
        assert_eq!(hits[1].0, "z"); // then nearest neighbor
    }

    #[test]
    fn search_top_k_bounds_result() {
        let mut idx = SimdVectorIndex::new(2);
        for i in 0..10 {
            idx.insert(format!("v{i}"), vec![i as f32, 0.0]);
        }
        assert_eq!(idx.search(&[0.0, 0.0], 3).len(), 3);
    }

    #[test]
    fn search_empty_index_returns_empty() {
        let idx = SimdVectorIndex::new(2);
        assert!(idx.search(&[1.0, 1.0], 5).is_empty());
    }

    #[test]
    fn search_top_k_larger_than_index_is_safe() {
        let mut idx = SimdVectorIndex::new(2);
        idx.insert("only", vec![1.0, 1.0]);
        assert_eq!(idx.search(&[1.0, 1.0], 100).len(), 1);
    }
}
