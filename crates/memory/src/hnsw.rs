use std::collections::BinaryHeap;
use std::cmp::Ordering;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
struct Neighbor {
    id: usize,
    distance: f32,
}

impl PartialEq for Neighbor {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Neighbor {}

impl Ord for Neighbor {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse for min-heap
        other.distance.partial_cmp(&self.distance).unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for Neighbor {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

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

        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot / (norm_a.sqrt() * norm_b.sqrt())
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
