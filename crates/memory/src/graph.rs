//! Entity graph (mem0 GRAPH-MEMORY model, SQLite-native).
//!
//! A graph of real-world entities (people, companies, ...) connected by
//! typed relations (`works_at`, `leads`, `owns`, ...). Complements the
//! memory-to-memory edges in `memory_edges`: this one answers multi-hop
//! OSINT questions like "who runs companies in sector X".
//!
//! Nodes deduplicate by `(name, entity_type)` — storing "Ivan Petrov" twice
//! reuses the same node (mem0 GRAPH-MEMORY §dedup).

use crate::db::MemoryDb;
use anyhow::Context;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

/// Known entity types (open set — unknown types are stored as-is).
pub const ENTITY_TYPES: &[&str] = &[
    "person",
    "company",
    "project",
    "technology",
    "role",
    "location",
    "event",
    "product",
    "other",
];

/// One graph node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityNode {
    pub id: String,
    pub name: String,
    pub entity_type: String,
    pub metadata: serde_json::Value,
    pub created_at: String,
}

/// One typed relation between two nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityEdge {
    pub id: String,
    pub from_id: String,
    pub to_id: String,
    pub relation: String,
    /// Provenance memory (if the triple came from an absorbed fact).
    pub memory_id: Option<String>,
    pub source: String,
    pub confidence: f64,
    pub created_at: String,
}

/// A node plus its resolved relation (used in query results).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeighborHit {
    pub edge: EntityEdge,
    pub node: EntityNode,
    /// true when the edge points FROM the query node TO `node`.
    pub outgoing: bool,
}

/// Normalize an entity name for dedup: trim and collapse whitespace.
/// Case-insensitivity is handled by the DB collation.
pub fn normalize_name(name: &str) -> String {
    name.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl MemoryDb {
    /// Create the entity tables (idempotent; doubles as the migration for
    /// databases created before the graph existed).
    pub fn init_graph_schema(&self) -> anyhow::Result<()> {
        let conn = self.conn_lock();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS entity_nodes (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                entity_type TEXT NOT NULL,
                metadata    TEXT NOT NULL DEFAULT '{}',
                created_at  TEXT NOT NULL,
                UNIQUE(name COLLATE NOCASE, entity_type)
            );
            CREATE TABLE IF NOT EXISTS entity_edges (
                id         TEXT PRIMARY KEY,
                from_node  TEXT NOT NULL,
                to_node    TEXT NOT NULL,
                relation   TEXT NOT NULL,
                memory_id  TEXT,
                source     TEXT NOT NULL DEFAULT '',
                confidence REAL NOT NULL DEFAULT 0.8,
                created_at TEXT NOT NULL,
                UNIQUE(from_node, to_node, relation)
            );
            CREATE INDEX IF NOT EXISTS idx_entity_edges_from ON entity_edges (from_node);
            CREATE INDEX IF NOT EXISTS idx_entity_edges_to ON entity_edges (to_node);",
        )?;
        Ok(())
    }

    /// Insert or reuse an entity node; returns its id.
    /// Dedup key: (name case-insensitive, entity_type) — mem0 rule.
    pub fn upsert_entity(
        &self,
        name: &str,
        entity_type: &str,
        metadata: Option<&serde_json::Value>,
    ) -> anyhow::Result<String> {
        let name = normalize_name(name);
        anyhow::ensure!(!name.is_empty(), "entity name is empty");
        let etype = entity_type.trim().to_lowercase();
        let etype = if etype.is_empty() { "other".to_string() } else { etype };
        let now = chrono::Utc::now().to_rfc3339();
        let id = uuid::Uuid::now_v7().to_string();
        let meta = metadata
            .map(|m| m.to_string())
            .unwrap_or_else(|| "{}".to_string());
        let conn = self.conn_lock();
        conn.execute(
            "INSERT OR IGNORE INTO entity_nodes (id, name, entity_type, metadata, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, name, etype, meta, now],
        )?;
        // Fetch the surviving row (ours if inserted, existing one otherwise).
        let existing: String = conn
            .prepare("SELECT id FROM entity_nodes WHERE name = ?1 COLLATE NOCASE AND entity_type = ?2")?
            .query_row(params![name, etype], |r| r.get(0))?;
        Ok(existing)
    }

    /// Add a typed relation between two nodes (both must exist).
    pub fn add_entity_edge(
        &self,
        from_id: &str,
        to_id: &str,
        relation: &str,
        memory_id: Option<&str>,
        source: &str,
        confidence: f64,
    ) -> anyhow::Result<String> {
        anyhow::ensure!(from_id != to_id, "self-referencing entity edge");
        let now = chrono::Utc::now().to_rfc3339();
        let id = uuid::Uuid::now_v7().to_string();
        let conn = self.conn_lock();
        conn.execute(
            "INSERT OR REPLACE INTO entity_edges
             (id, from_node, to_node, relation, memory_id, source, confidence, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id,
                from_id,
                to_id,
                relation.trim().to_lowercase(),
                memory_id,
                source,
                confidence,
                now
            ],
        )?;
        Ok(id)
    }

    pub fn entity(&self, id: &str) -> anyhow::Result<Option<EntityNode>> {
        let conn = self.conn_lock();
        let row = conn
            .prepare("SELECT id, name, entity_type, metadata, created_at FROM entity_nodes WHERE id = ?1")?
            .query_row(params![id], map_node)
            .optional()?;
        Ok(row)
    }

    /// Look up a node by (case-insensitive) name and type; `entity_type`
    /// empty matches any type but requires a unique name.
    pub fn entity_by_name(&self, name: &str, entity_type: &str) -> anyhow::Result<Option<EntityNode>> {
        let name = normalize_name(name);
        let conn = self.conn_lock();
        let etype = entity_type.trim().to_lowercase();
        let row = if etype.is_empty() {
            let mut stmt = conn.prepare(
                "SELECT id, name, entity_type, metadata, created_at FROM entity_nodes
                 WHERE name = ?1 COLLATE NOCASE",
            )?;
            let mut rows = stmt.query_map(params![name], map_node)?;
            match rows.next().transpose()? {
                Some(node) => {
                    if rows.next().is_some() {
                        anyhow::bail!("ambiguous entity name '{name}' (multiple types)");
                    }
                    Some(node)
                }
                None => None,
            }
        } else {
            conn.prepare(
                "SELECT id, name, entity_type, metadata, created_at FROM entity_nodes
                 WHERE name = ?1 COLLATE NOCASE AND entity_type = ?2",
            )?
            .query_row(params![name, etype], map_node)
            .optional()?
        };
        Ok(row)
    }

    /// Direct neighbours of a node with the connecting edge and direction.
    pub fn entity_neighbors(&self, id: &str) -> anyhow::Result<Vec<NeighborHit>> {
        let conn = self.conn_lock();
        let mut out = Vec::new();
        // Outgoing edges.
        let mut stmt = conn.prepare(
            "SELECT e.id, e.from_node, e.to_node, e.relation, e.memory_id, e.source, e.confidence, e.created_at,
                    n.id, n.name, n.entity_type, n.metadata, n.created_at
             FROM entity_edges e JOIN entity_nodes n ON n.id = e.to_node
             WHERE e.from_node = ?1",
        )?;
        let rows = stmt.query_map(params![id], |r| {
            Ok((map_edge(r)?, map_node_offset(r)?))
        })?;
        for pair in rows {
            let (edge, node) = pair?;
            out.push(NeighborHit { edge, node, outgoing: true });
        }
        // Incoming edges.
        let mut stmt = conn.prepare(
            "SELECT e.id, e.from_node, e.to_node, e.relation, e.memory_id, e.source, e.confidence, e.created_at,
                    n.id, n.name, n.entity_type, n.metadata, n.created_at
             FROM entity_edges e JOIN entity_nodes n ON n.id = e.from_node
             WHERE e.to_node = ?1",
        )?;
        let rows = stmt.query_map(params![id], |r| {
            Ok((map_edge(r)?, map_node_offset(r)?))
        })?;
        for pair in rows {
            let (edge, node) = pair?;
            out.push(NeighborHit { edge, node, outgoing: false });
        }
        Ok(out)
    }

    /// All nodes, optionally filtered by type (for listing/stats).
    pub fn list_entities(&self, entity_type: Option<&str>, limit: usize) -> anyhow::Result<Vec<EntityNode>> {
        let conn = self.conn_lock();
        let etype = entity_type.map(|t| t.trim().to_lowercase()).unwrap_or_default();
        if !etype.is_empty() {
            let mut stmt = conn.prepare(
                "SELECT id, name, entity_type, metadata, created_at FROM entity_nodes
                 WHERE entity_type = ?1 ORDER BY name LIMIT ?2",
            )?;
            let rows = stmt
                .query_map(params![etype, limit as i64], map_node)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, name, entity_type, metadata, created_at FROM entity_nodes
                 ORDER BY entity_type, name LIMIT ?1",
            )?;
            let rows = stmt
                .query_map(params![limit as i64], map_node)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        }
    }

    pub fn count_entities(&self) -> anyhow::Result<(i64, i64)> {
        let conn = self.conn_lock();
        let nodes: i64 = conn.query_row("SELECT COUNT(*) FROM entity_nodes", [], |r| r.get(0))?;
        let edges: i64 = conn.query_row("SELECT COUNT(*) FROM entity_edges", [], |r| r.get(0))?;
        Ok((nodes, edges))
    }

    /// Remove an entity node and all touching edges (GDPR/poison data).
    pub fn delete_entity(&self, id: &str) -> anyhow::Result<bool> {
        let conn = self.conn_lock();
        let n = conn.execute("DELETE FROM entity_nodes WHERE id = ?1", params![id])?;
        conn.execute(
            "DELETE FROM entity_edges WHERE from_node = ?1 OR to_node = ?1",
            params![id],
        )?;
        Ok(n > 0)
    }
}

/// BFS over the entity graph from `start`, up to `max_depth` hops
/// (mem0 multi-hop retrieval, 1-2 hops recommended). Returns all simple
/// paths found, each as an alternating node/relation chain.
pub fn multi_hop(db: &MemoryDb, start: &EntityNode, max_depth: usize) -> anyhow::Result<Vec<GraphPath>> {
    let mut paths: Vec<GraphPath> = Vec::new();
    let mut frontier: Vec<(EntityNode, GraphPath)> = vec![(
        start.clone(),
        GraphPath {
            nodes: vec![start.clone()],
            relations: Vec::new(),
        },
    )];
    let depth = max_depth.clamp(1, 4);
    for _ in 0..depth {
        let mut next_frontier = Vec::new();
        for (node, path) in frontier {
            for hit in db.entity_neighbors(&node.id)? {
                // Simple paths only — never revisit a node within one path.
                if path.nodes.iter().any(|n| n.id == hit.node.id) {
                    continue;
                }
                let relation = if hit.outgoing {
                    hit.edge.relation.clone()
                } else {
                    format!("{}⁻¹", hit.edge.relation) // inverse traversal marker
                };
                let mut new_path = path.clone();
                new_path.nodes.push(hit.node.clone());
                new_path.relations.push(relation);
                paths.push(new_path.clone());
                next_frontier.push((hit.node, new_path));
            }
        }
        frontier = next_frontier;
        if frontier.is_empty() {
            break;
        }
    }
    Ok(paths)
}

/// One traversal chain: nodes[0] — start, relations[i] connects
/// nodes[i] → nodes[i+1] (a trailing ⁻¹ marks an inverse edge).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphPath {
    pub nodes: Vec<EntityNode>,
    pub relations: Vec<String>,
}

impl GraphPath {
    /// Human-readable chain: `Ivan Petrov --works_at--> Acme LLC`.
    pub fn render(&self) -> String {
        let mut out = String::new();
        for (i, node) in self.nodes.iter().enumerate() {
            if i > 0 {
                out.push_str(&format!(" --{}--> ", self.relations[i - 1]));
            }
            out.push_str(&format!("{} ({})", node.name, node.entity_type));
        }
        out
    }
}

/// Register entities/relations attached to an absorbed fact's metadata:
///
/// ```json
/// { "entities":  [{"name": "Acme LLC", "type": "company"}],
///   "relations": [{"from": "Ivan Petrov", "to": "Acme LLC", "relation": "works_at"}] }
/// ```
///
/// Unknown relation endpoints are created as `person`/`company`-less
/// `other` nodes — explicit typing wins when present.
pub fn ingest_entities(
    db: &MemoryDb,
    metadata: &serde_json::Value,
    memory_id: &str,
    source: &str,
) -> anyhow::Result<usize> {
    let mut count = 0usize;

    let entity_id = |name: &str, etype: &str| -> anyhow::Result<String> {
        db.upsert_entity(name, etype, None)
    };
    let type_of = |entities: &serde_json::Value, name: &str| -> String {
        entities
            .as_array()
            .and_then(|arr| {
                arr.iter().find(|e| {
                    e.get("name")
                        .and_then(|n| n.as_str())
                        .map(|n| normalize_name(n).eq_ignore_ascii_case(&normalize_name(name)))
                        .unwrap_or(false)
                })
            })
            .and_then(|e| e.get("type").and_then(|t| t.as_str()))
            .unwrap_or("other")
            .to_string()
    };

    // Ensure all declared entities exist.
    let entities = metadata.get("entities").cloned().unwrap_or(serde_json::json!([]));
    if let Some(arr) = entities.as_array() {
        for e in arr {
            let Some(name) = e.get("name").and_then(|n| n.as_str()) else { continue };
            let etype = e.get("type").and_then(|t| t.as_str()).unwrap_or("other");
            let meta = e.get("metadata");
            db.upsert_entity(name, etype, meta)?;
            count += 1;
        }
    }

    // Relations: resolve endpoints (creating missing nodes as `other`).
    if let Some(arr) = metadata.get("relations").and_then(|r| r.as_array()) {
        for rel in arr {
            let (Some(from), Some(to)) = (
                rel.get("from").and_then(|v| v.as_str()),
                rel.get("to").and_then(|v| v.as_str()),
            ) else {
                continue;
            };
            let relation = rel
                .get("relation")
                .and_then(|v| v.as_str())
                .unwrap_or("related_to");
            let confidence = rel.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.8);
            let from_id = entity_id(from, &type_of(&entities, from))?;
            let to_id = entity_id(to, &type_of(&entities, to))?;
            db.add_entity_edge(&from_id, &to_id, relation, Some(memory_id), source, confidence)
                .with_context(|| format!("edge {from} --{relation}--> {to}"))?;
            count += 1;
        }
    }
    Ok(count)
}

// ── row mappers ──────────────────────────────────────────────────────────────

fn map_node(r: &rusqlite::Row<'_>) -> rusqlite::Result<EntityNode> {
    let meta: String = r.get(3)?;
    Ok(EntityNode {
        id: r.get(0)?,
        name: r.get(1)?,
        entity_type: r.get(2)?,
        metadata: serde_json::from_str(&meta).unwrap_or(serde_json::json!({})),
        created_at: r.get(4)?,
    })
}

/// Node columns start at offset 8 in the joined neighbour queries.
fn map_node_offset(r: &rusqlite::Row<'_>) -> rusqlite::Result<EntityNode> {
    let meta: String = r.get(11)?;
    Ok(EntityNode {
        id: r.get(8)?,
        name: r.get(9)?,
        entity_type: r.get(10)?,
        metadata: serde_json::from_str(&meta).unwrap_or(serde_json::json!({})),
        created_at: r.get(12)?,
    })
}

fn map_edge(r: &rusqlite::Row<'_>) -> rusqlite::Result<EntityEdge> {
    Ok(EntityEdge {
        id: r.get(0)?,
        from_id: r.get(1)?,
        to_id: r.get(2)?,
        relation: r.get(3)?,
        memory_id: r.get(4)?,
        source: r.get(5)?,
        confidence: r.get(6)?,
        created_at: r.get(7)?,
    })
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> MemoryDb {
        let db = MemoryDb::in_memory().unwrap();
        db.init_graph_schema().unwrap();
        db
    }

    #[test]
    fn upsert_dedups_by_name_and_type() {
        let db = db();
        let a = db.upsert_entity("Ivan Petrov", "person", None).unwrap();
        let b = db.upsert_entity("ivan petrov", "person", None).unwrap();
        let c = db.upsert_entity("Ivan Petrov", "company", None).unwrap();
        assert_eq!(a, b, "case-insensitive same-type must reuse the node");
        assert_ne!(a, c, "different type is a different node");

        // Whitespace normalization.
        let d = db.upsert_entity("  Ivan   Petrov ", "person", None).unwrap();
        assert_eq!(a, d);
    }

    #[test]
    fn entity_by_name_lookup() {
        let db = db();
        db.upsert_entity("Acme LLC", "company", None).unwrap();
        let found = db.entity_by_name("acme llc", "company").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "Acme LLC");
        assert!(db.entity_by_name("acme llc", "person").unwrap().is_none());
    }

    #[test]
    fn edges_and_neighbors() {
        let db = db();
        let ivan = db.upsert_entity("Ivan Petrov", "person", None).unwrap();
        let acme = db.upsert_entity("Acme LLC", "company", None).unwrap();
        db.add_entity_edge(&ivan, &acme, "works_at", None, "test", 0.9).unwrap();

        let out = db.entity_neighbors(&ivan).unwrap();
        assert_eq!(out.len(), 1);
        assert!(out[0].outgoing);
        assert_eq!(out[0].node.id, acme);
        assert_eq!(out[0].edge.relation, "works_at");

        let inc = db.entity_neighbors(&acme).unwrap();
        assert_eq!(inc.len(), 1);
        assert!(!inc[0].outgoing);
        assert_eq!(inc[0].node.id, ivan);
    }

    #[test]
    fn multi_hop_two_levels() {
        let db = db();
        // Ivan --works_at--> Acme --located_in--> Kazan
        let ivan = db.upsert_entity("Ivan Petrov", "person", None).unwrap();
        let acme = db.upsert_entity("Acme LLC", "company", None).unwrap();
        let kazan = db.upsert_entity("Kazan", "location", None).unwrap();
        db.add_entity_edge(&ivan, &acme, "works_at", None, "s", 0.9).unwrap();
        db.add_entity_edge(&acme, &kazan, "located_in", None, "s", 0.9).unwrap();

        let start = db.entity(&ivan).unwrap().unwrap();
        let paths = multi_hop(&db, &start, 2).unwrap();
        // depth 1: Ivan→Acme; depth 2: Ivan→Acme, Ivan→Acme→Kazan
        assert!(paths.iter().any(|p| p.nodes.len() == 2));
        let full = paths.iter().find(|p| p.nodes.len() == 3).unwrap();
        assert_eq!(full.nodes[2].name, "Kazan");
        assert_eq!(full.render(), "Ivan Petrov (person) --works_at--> Acme LLC (company) --located_in--> Kazan (location)");
    }

    #[test]
    fn multi_hop_does_not_loop() {
        let db = db();
        // A --related_to--> B --related_to--> A must not produce infinite paths.
        let a = db.upsert_entity("Node A", "other", None).unwrap();
        let b = db.upsert_entity("Node B", "other", None).unwrap();
        db.add_entity_edge(&a, &b, "related_to", None, "s", 0.5).unwrap();
        db.add_entity_edge(&b, &a, "related_to", None, "s", 0.5).unwrap();

        let start = db.entity(&a).unwrap().unwrap();
        let paths = multi_hop(&db, &start, 3).unwrap();
        // All paths stay simple: max 2 nodes here (A→B), no revisit.
        assert!(paths.iter().all(|p| p.nodes.len() <= 2));
    }

    #[test]
    fn ingest_entities_from_metadata() {
        let db = db();
        let meta = serde_json::json!({
            "entities": [
                {"name": "Maria Ivanova", "type": "person"},
                {"name": "Globex LLC", "type": "company"}
            ],
            "relations": [
                {"from": "Maria Ivanova", "to": "Globex LLC", "relation": "leads", "confidence": 0.95}
            ]
        });
        let n = ingest_entities(&db, &meta, "mem-1", "session:x").unwrap();
        assert_eq!(n, 3, "2 entities + 1 relation");

        let maria = db.entity_by_name("Maria Ivanova", "person").unwrap().unwrap();
        let neighbors = db.entity_neighbors(&maria.id).unwrap();
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].edge.relation, "leads");
        assert_eq!(neighbors[0].edge.memory_id.as_deref(), Some("mem-1"));
        assert_eq!(neighbors[0].node.name, "Globex LLC");
    }

    #[test]
    fn ingest_relations_create_missing_nodes_as_other() {
        let db = db();
        let meta = serde_json::json!({
            "relations": [{"from": "Unknown Person", "to": "Unknown Co", "relation": "works_at"}]
        });
        let n = ingest_entities(&db, &meta, "mem-2", "s").unwrap();
        assert_eq!(n, 1);
        let node = db.entity_by_name("Unknown Person", "other").unwrap();
        assert!(node.is_some());
    }

    #[test]
    fn delete_entity_removes_edges() {
        let db = db();
        let a = db.upsert_entity("A", "other", None).unwrap();
        let b = db.upsert_entity("B", "other", None).unwrap();
        db.add_entity_edge(&a, &b, "related_to", None, "s", 0.5).unwrap();
        assert!(db.delete_entity(&a).unwrap());
        assert!(db.entity(&a).unwrap().is_none());
        assert!(db.entity_neighbors(&b).unwrap().is_empty());
        let (nodes, edges) = db.count_entities().unwrap();
        assert_eq!((nodes, edges), (1, 0));
    }

    #[test]
    fn list_entities_filter_and_counts() {
        let db = db();
        db.upsert_entity("X Corp", "company", None).unwrap();
        db.upsert_entity("Y Corp", "company", None).unwrap();
        db.upsert_entity("John", "person", None).unwrap();

        let companies = db.list_entities(Some("company"), 10).unwrap();
        assert_eq!(companies.len(), 2);
        let all = db.list_entities(None, 10).unwrap();
        assert_eq!(all.len(), 3);
        let (nodes, _) = db.count_entities().unwrap();
        assert_eq!(nodes, 3);
    }

    #[test]
    fn self_edge_rejected() {
        let db = db();
        let a = db.upsert_entity("Solo", "other", None).unwrap();
        assert!(db.add_entity_edge(&a, &a, "related_to", None, "s", 0.5).is_err());
    }

    #[test]
    fn graph_schema_migration_is_idempotent() {
        let db = db();
        db.init_graph_schema().unwrap();
        db.init_graph_schema().unwrap();
        db.upsert_entity("After migration", "other", None).unwrap();
        assert_eq!(db.count_entities().unwrap().0, 1);
    }
}
