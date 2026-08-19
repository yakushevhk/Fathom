//! Property tests (proptest) for the memory store.

use pr_memory::{content_hash, AbsorbFact, AbsorbRequest, Memory, Scope};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn content_hash_deterministic(s in "\\PC{0,200}") {
        prop_assert_eq!(content_hash(&s), content_hash(&s));
        prop_assert!(!content_hash(&s).is_empty());
    }

    #[test]
    fn content_hash_differs_for_different_inputs(
        a in "[a-z]{1,40}",
        b in "[a-z]{1,40}",
    ) {
        prop_assume!(a != b);
        prop_assert_ne!(content_hash(&a), content_hash(&b));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(8))]

    #[test]
    fn absorb_same_fact_twice_is_idempotent(fact in "[a-zA-Z0-9 ,.]{20,120}") {
        // A fresh runtime per case: proptest is synchronous, absorb is async.
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mem = Memory::in_memory(pr_core::MemoryConfig::default()).unwrap();
            let req = AbsorbRequest {
                facts: vec![AbsorbFact {
                    content: fact.clone(),
                    metadata: serde_json::json!({}),
                    tags: vec![],
                    confidence: None,
                    memory_class: None,
                }],
                source: "proptest".into(),
                scope: Scope::Agent,
                scope_key: String::new(),
                context: None,
                dry_run: false,
            };
            let first = mem.pipeline().absorb(req.clone()).await.unwrap();
            let second = mem.pipeline().absorb(req).await.unwrap();
            assert_eq!(first.created, 1, "first absorb creates the fact");
            assert_eq!(second.created, 0, "second absorb must not duplicate");
            // The store holds exactly one active copy.
            let rows = mem
                .db
                .list(&pr_memory::ScopeFilter::persistent(), Some("active"), usize::MAX)
                .unwrap();
            assert_eq!(rows.len(), 1);
        });
    }
}
