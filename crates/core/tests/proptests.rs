//! Property tests (proptest) for core primitives: token estimation,
//! normalization and export dedup.

use proptest::prelude::*;

#[test]
fn estimate_tokens_empty_is_zero() {
    assert_eq!(pr_core::estimate_tokens(""), 0);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn estimate_tokens_nonzero_for_nonempty(s in "\\PC{1,300}") {
        let n = pr_core::estimate_tokens(&s);
        prop_assert!(n >= 1, "non-empty text must estimate >= 1 token");
        // Sanity upper bound: no tokenizer needs more than ~4 tokens per
        // char plus a small constant.
        let bound = (s.chars().count() as u32) * 4 + 8;
        prop_assert!(n <= bound, "{n} exceeds sanity bound {bound}");
    }

    #[test]
    fn normalize_email_idempotent(s in "\\PC{0,100}") {
        let once = pr_core::normalize_email(&s);
        let twice = pr_core::normalize_email(&once);
        prop_assert_eq!(once, twice);
    }

    #[test]
    fn normalize_phone_idempotent(s in "\\PC{0,60}") {
        let once = pr_core::normalize_phone(&s);
        let twice = pr_core::normalize_phone(&once);
        prop_assert_eq!(once, twice);
    }

    #[test]
    fn dedup_contacts_idempotent(emails in prop::collection::vec("[a-z]{1,8}@[a-z]{1,6}\\.[a-z]{2,3}", 0..12)) {
        let contacts: Vec<pr_core::Contact> = emails
            .iter()
            .map(|e| {
                let mut c = pr_core::Contact::new();
                c.email = Some(e.clone());
                c
            })
            .collect();
        let (first_pass, dropped1) = pr_core::dedup_contacts(contacts.clone());
        let (second_pass, dropped2) = pr_core::dedup_contacts(first_pass.clone());
        prop_assert_eq!(second_pass.len(), first_pass.len(), "second pass must drop nothing more");
        prop_assert_eq!(dropped2, 0);
        prop_assert!(first_pass.len() + dropped1 == contacts.len());
    }
}
