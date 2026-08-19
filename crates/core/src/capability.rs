//! Capability evidence for the context window (ouroboros
//! `capability_evidence.py` inspiration).
//!
//! Rather than guessing a per-model context window from a stale static table,
//! we track evidence of what window a run can actually use and resolve a
//! **fail-closed** number to budget against:
//!
//! - When the operator pins an explicit `context_window`, we treat it as
//!   confirmed evidence and budget against it directly.
//! - Otherwise we fall back to a **conservative** default driven by the
//!   `context_window_profile` (`low` ≈ 128K, `max` ≈ 256K). We never budget to
//!   an ambitious number we have not earned — a context-frame that cannot fit
//!   the window is worse than one we undershoot, because truncation/crash
//!   destroys a run that only slightly overshoots a proven cap.
//!
//! The explicit window *always* wins; the profile is only consulted when the
//! operator left it at the default, so there is no way for a mistaken model
//! listing to inflate the budget.

use serde::{Deserialize, Serialize};

/// How the context window was proven.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityEvidence {
    /// Operator explicitly configured the window — strongest evidence.
    Confirmed(u32),
    /// Resolved from a profile default because nothing explicit was set.
    Asserted(u32),
    /// No usable evidence at all — callers should use a minimal safe floor.
    Unknown,
}

/// Conservative floor we never budget above without explicit evidence.
pub const LOW_WINDOW_FLOOR: u32 = 128_000;
/// Ceiling for an unproven "max" profile.
pub const MAX_WINDOW_FLOOR: u32 = 256_000;
/// Absolute minimal window used when nothing at all is known.
pub const MIN_SAFE_WINDOW: u32 = 32_000;

/// Window profile used when the operator did not pin an explicit window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WindowProfile {
    /// Conservative single-budget profile (local/cheap model).
    #[default]
    Low,
    /// Larger window **only** used when the operator opts into it; still
    /// capped at an unproven ceiling so we never trust a model-listing guess.
    Max,
}

/// Resolve the fail-closed effective window from explicit config + a profile.
pub fn resolve_window(explicit_window: Option<u32>, profile: WindowProfile) -> CapabilityEvidence {
    match explicit_window {
        Some(w) if w > 0 => CapabilityEvidence::Confirmed(w),
        Some(_) | None => match profile {
            WindowProfile::Low => CapabilityEvidence::Asserted(LOW_WINDOW_FLOOR),
            WindowProfile::Max => CapabilityEvidence::Asserted(MAX_WINDOW_FLOOR),
        },
    }
}

/// The effective numeric window for budgeting, further guarded by the
/// absolute safety floor even when evidence claims something implausibly small.
pub fn effective_window(evidence: CapabilityEvidence) -> u32 {
    match evidence {
        CapabilityEvidence::Confirmed(w) => w.max(MIN_SAFE_WINDOW),
        CapabilityEvidence::Asserted(w) => w.max(MIN_SAFE_WINDOW),
        CapabilityEvidence::Unknown => MIN_SAFE_WINDOW,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_window_is_confirmed() {
        assert_eq!(resolve_window(Some(200_000), WindowProfile::Low), CapabilityEvidence::Confirmed(200_000));
    }

    #[test]
    fn profile_falls_back_to_conservative_floor() {
        assert_eq!(resolve_window(None, WindowProfile::Low), CapabilityEvidence::Asserted(LOW_WINDOW_FLOOR));
        assert_eq!(resolve_window(None, WindowProfile::Max), CapabilityEvidence::Asserted(MAX_WINDOW_FLOOR));
    }

    #[test]
    fn zero_explicit_counts_as_unset() {
        assert_eq!(resolve_window(Some(0), WindowProfile::Low), CapabilityEvidence::Asserted(LOW_WINDOW_FLOOR));
    }

    #[test]
    fn effective_never_below_safety_floor() {
        assert_eq!(effective_window(CapabilityEvidence::Confirmed(1000)), MIN_SAFE_WINDOW);
        assert_eq!(effective_window(CapabilityEvidence::Asserted(LOW_WINDOW_FLOOR)), LOW_WINDOW_FLOOR);
        assert_eq!(effective_window(CapabilityEvidence::Unknown), MIN_SAFE_WINDOW);
    }
}
