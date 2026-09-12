/**
 * Tactile haptic vibration utility for touch-enabled devices and mobile web.
 */

export type HapticType = "tap" | "light" | "medium" | "heavy" | "success" | "warning" | "error" | "selection";

const HAPTIC_PATTERNS: Record<HapticType, number | number[]> = {
  tap: 10,
  light: 15,
  medium: 25,
  heavy: 40,
  selection: 8,
  success: [15, 60, 20],
  warning: [30, 80, 30],
  error: [40, 100, 40, 100, 40],
};

export function triggerHaptic(type: HapticType = "tap"): void {
  if (typeof window === "undefined" || typeof navigator === "undefined") {
    return;
  }
  if (!("vibrate" in navigator) || typeof navigator.vibrate !== "function") {
    return;
  }

  try {
    const pattern = HAPTIC_PATTERNS[type];
    navigator.vibrate(pattern);
  } catch {
    // Suppress vibration exceptions in sandboxed iframes or restricted webviews
  }
}
