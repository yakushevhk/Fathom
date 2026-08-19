//! Property tests (proptest) for the extraction parsers.
//!
//! The parsers run on arbitrary web content — they must never panic and
//! must only ever emit well-shaped contacts, regardless of input.

use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn extract_emails_never_panics_and_yields_valid_shapes(s in "\\PC{0,400}") {
        for contact in pr_tools::extract::extract_emails(&s) {
            let email = contact.email;
            prop_assert!(email.contains('@'), "email without @: {email}");
            let (local, domain) = email.split_once('@').unwrap();
            prop_assert!(!local.is_empty(), "empty local part: {email}");
            prop_assert!(domain.contains('.'), "domain without dot: {email}");
            prop_assert!(!email.chars().any(char::is_whitespace));
        }
    }

    #[test]
    fn extract_phones_never_panics_and_yields_digitful_shapes(s in "\\PC{0,400}") {
        for contact in pr_tools::extract::extract_phones(&s) {
            let digits = contact.phone.chars().filter(|c| c.is_ascii_digit()).count();
            prop_assert!(digits >= 5, "phone with <5 digits: {}", contact.phone);
        }
    }

    #[test]
    fn extract_contacts_never_panics(
        text in "\\PC{0,300}",
        html in "\\PC{0,300}",
    ) {
        let _ = pr_tools::extract::extract_contacts(&text, &html);
    }

    #[test]
    fn parse_entities_json_never_panics(s in "\\PC{0,400}") {
        // Returns Ok or Err — must never panic.
        let _ = pr_tools::extract::parse_entities_json(&s);
    }

    #[test]
    fn classify_social_url_never_panics(url in "\\PC{0,200}") {
        let _ = pr_tools::extract::classify_social_url(&url);
    }

    #[test]
    fn extraction_is_deterministic(s in "\\PC{0,200}") {
        let a = pr_tools::extract::extract_emails(&s);
        let b = pr_tools::extract::extract_emails(&s);
        prop_assert_eq!(a.len(), b.len());
    }
}
