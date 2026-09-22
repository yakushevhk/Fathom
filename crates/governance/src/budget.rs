use serde::{Deserialize, Serialize};

/// Financial and token usage budget guardrails to enforce hard policy constraints
/// per session or per coworker.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BudgetPolicy {
    /// Maximum monetary budget allowed in USD (e.g. 5.0 for $5.00)
    pub max_usd: Option<f64>,
    /// Hard token cap across total session tokens
    pub max_total_tokens: Option<u64>,
    /// Action when budget is exceeded: "pause", "deny", or "escalate"
    #[serde(default = "default_exceeded_action")]
    pub on_exceeded: String,
}

fn default_exceeded_action() -> String {
    "pause".to_string()
}

impl Default for BudgetPolicy {
    fn default() -> Self {
        Self {
            max_usd: None,
            max_total_tokens: None,
            on_exceeded: default_exceeded_action(),
        }
    }
}

impl BudgetPolicy {
    pub fn check_limits(&self, current_usd: f64, current_tokens: u64) -> Result<(), String> {
        if let Some(max_cost) = self.max_usd {
            if current_usd >= max_cost {
                return Err(format!(
                    "Financial budget exceeded: consumed ${:.4} >= limit ${:.4} (action: {})",
                    current_usd, max_cost, self.on_exceeded
                ));
            }
        }

        if let Some(max_toks) = self.max_total_tokens {
            if current_tokens >= max_toks {
                return Err(format!(
                    "Token budget exceeded: consumed {} tokens >= limit {} tokens (action: {})",
                    current_tokens, max_toks, self.on_exceeded
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_is_permissive() {
        let p = BudgetPolicy::default();
        assert!(p.max_usd.is_none());
        assert!(p.max_total_tokens.is_none());
        assert_eq!(p.on_exceeded, "pause");
        assert!(p.check_limits(f64::MAX, u64::MAX).is_ok());
    }

    #[test]
    fn usd_limit_enforced() {
        let p = BudgetPolicy {
            max_usd: Some(5.0),
            ..Default::default()
        };
        assert!(p.check_limits(4.99, 0).is_ok());
        assert!(p.check_limits(5.0, 0).is_err()); // >= boundary
        assert!(p.check_limits(5.01, 0).is_err());
    }

    #[test]
    fn token_limit_enforced() {
        let p = BudgetPolicy {
            max_total_tokens: Some(1000),
            ..Default::default()
        };
        assert!(p.check_limits(0.0, 999).is_ok());
        assert!(p.check_limits(0.0, 1000).is_err());
    }

    #[test]
    fn usd_checked_before_tokens() {
        let p = BudgetPolicy {
            max_usd: Some(1.0),
            max_total_tokens: Some(10),
            ..Default::default()
        };
        let err = p.check_limits(2.0, 20).unwrap_err();
        assert!(err.contains("Financial"), "{err}");
    }

    #[test]
    fn error_messages_include_action() {
        let p = BudgetPolicy {
            max_usd: Some(1.0),
            on_exceeded: "deny".into(),
            ..Default::default()
        };
        let err = p.check_limits(9.0, 0).unwrap_err();
        assert!(err.contains("deny"), "{err}");
    }

    #[test]
    fn serde_default_action_is_pause() {
        let p: BudgetPolicy = serde_json::from_str(r#"{"max_usd":1.0}"#).unwrap();
        assert_eq!(p.on_exceeded, "pause");
    }
}
