use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

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
        let out_list = self.out_edges.entry(triple.subject.clone()).or_default();
        let out_edge = (triple.predicate.clone(), triple.object.clone());
        if !out_list.contains(&out_edge) {
            out_list.push(out_edge);
        }

        let in_list = self.in_edges.entry(triple.object.clone()).or_default();
        let in_edge = (triple.predicate.clone(), triple.subject.clone());
        if !in_list.contains(&in_edge) {
            in_list.push(in_edge);
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str, p: &str, o: &str) -> RdfTriple {
        RdfTriple {
            subject: s.into(),
            predicate: p.into(),
            object: o.into(),
            confidence: 100,
        }
    }

    #[test]
    fn insert_and_len() {
        let mut g = TriplesGraph::new();
        assert!(g.is_empty());
        g.insert(t("alice", "knows", "bob"));
        assert_eq!(g.len(), 1);
        assert!(!g.is_empty());
    }

    #[test]
    fn insert_same_triple_twice_is_deduped() {
        let mut g = TriplesGraph::new();
        g.insert(t("a", "r", "b"));
        g.insert(t("a", "r", "b"));
        assert_eq!(g.len(), 1);
    }

    #[test]
    fn insert_same_edge_different_confidence_dedupes_edge_list() {
        let mut g = TriplesGraph::new();
        g.insert(t("a", "r", "b"));
        let mut t2 = t("a", "r", "b");
        t2.confidence = 50;
        g.insert(t2);
        // Triple set grows (different confidence) but edge lists dedupe.
        assert_eq!(g.len(), 2);
        let out = g.traverse("a", 1);
        assert_eq!(out.len(), 1); // one edge traversed, not two
    }

    #[test]
    fn traverse_respects_max_depth() {
        let mut g = TriplesGraph::new();
        g.insert(t("a", "to", "b"));
        g.insert(t("b", "to", "c"));
        g.insert(t("c", "to", "d"));
        assert_eq!(g.traverse("a", 1).len(), 1); // a→b only
        assert_eq!(g.traverse("a", 2).len(), 2); // a→b, b→c
        assert_eq!(g.traverse("a", 3).len(), 3);
    }

    #[test]
    fn traverse_depth_zero_returns_nothing() {
        let mut g = TriplesGraph::new();
        g.insert(t("a", "to", "b"));
        assert!(g.traverse("a", 0).is_empty());
    }

    #[test]
    fn traverse_no_cycles() {
        let mut g = TriplesGraph::new();
        g.insert(t("a", "to", "b"));
        g.insert(t("b", "to", "a"));
        g.insert(t("b", "to", "c"));
        let r = g.traverse("a", 10);
        // Each edge reported once; no infinite loop on the a↔b cycle.
        assert_eq!(r.len(), 3);
    }

    #[test]
    fn traverse_unknown_start_empty() {
        let g = TriplesGraph::new();
        assert!(g.traverse("ghost", 5).is_empty());
    }

    #[test]
    fn query_entity_case_insensitive_on_subject_and_object() {
        let mut g = TriplesGraph::new();
        g.insert(t("Alice", "knows", "Bob"));
        g.insert(t("Carol", "likes", "alice"));
        let hits = g.query_entity("ALICE");
        assert_eq!(hits.len(), 2);
        assert!(g.query_entity("dave").is_empty());
    }

    #[test]
    fn confidence_defaults_to_100_on_deserialize() {
        let t: RdfTriple =
            serde_json::from_str(r#"{"subject":"a","predicate":"p","object":"b"}"#).unwrap();
        assert_eq!(t.confidence, 100);
    }
}
