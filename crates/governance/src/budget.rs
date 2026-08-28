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
