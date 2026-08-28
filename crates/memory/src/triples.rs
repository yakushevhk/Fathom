use std::collections::{HashMap, HashSet, VecDeque};
use serde::{Deserialize, Serialize};

/// An RDF-style Subject-Predicate-Object knowledge graph triple.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct RdfTriple {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    #[serde(default = "default_confidence")]
    pub confidence: u32, // 1 - 100
}

fn default_confidence() -> u32 {
    100
}

/// In-Memory RDF Knowledge Graph & Multi-Hop GraphRAG Traversal Engine.
#[derive(Default)]
pub struct TriplesGraph {
    /// subject -> [(predicate, object)]
    out_edges: HashMap<String, Vec<(String, String)>>,
    /// object -> [(predicate, subject)]
    in_edges: HashMap<String, Vec<(String, String)>>,
    triples: HashSet<RdfTriple>,
}

impl TriplesGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an RDF triple to the graph.
    pub fn insert(&mut self, triple: RdfTriple) {
        self.out_edges
            .entry(triple.subject.clone())
            .or_default()
            .push((triple.predicate.clone(), triple.object.clone()));

        self.in_edges
            .entry(triple.object.clone())
            .or_default()
            .push((triple.predicate.clone(), triple.subject.clone()));

        self.triples.insert(triple);
    }

    /// Multi-hop BFS path traversal from starting entity.
    pub fn traverse(&self, start: &str, max_depth: usize) -> Vec<RdfTriple> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut result = Vec::new();

        queue.push_back((start.to_string(), 0));
        visited.insert(start.to_string());

        while let Some((node, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }

            if let Some(edges) = self.out_edges.get(&node) {
                for (pred, obj) in edges {
                    result.push(RdfTriple {
                        subject: node.clone(),
                        predicate: pred.clone(),
                        object: obj.clone(),
                        confidence: 100,
                    });

                    if !visited.contains(obj) {
                        visited.insert(obj.clone());
                        queue.push_back((obj.clone(), depth + 1));
                    }
                }
            }
        }

        result
    }

    /// Query triples connected to a given subject or object.
    pub fn query_entity(&self, entity: &str) -> Vec<RdfTriple> {
        let mut matches = Vec::new();
        for t in &self.triples {
            if t.subject.eq_ignore_ascii_case(entity) || t.object.eq_ignore_ascii_case(entity) {
                matches.push(t.clone());
            }
        }
        matches
    }

    pub fn len(&self) -> usize {
        self.triples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.triples.is_empty()
    }
}
