//! GPUI Theme and design tokens matching OpenBot and Fathom UI-SPEC.
//! Glassmorphic dark metal aesthetics, typography and accents.

use gpui::{rgb, rgba, Rgba};

pub struct Theme;

impl Theme {
    // Backgrounds
    pub fn bg_window() -> Rgba {
        rgb(0x111115)
    }

    pub fn bg_surface() -> Rgba {
        rgb(0x131317)
    }

    pub fn bg_topbar() -> Rgba {
        rgb(0x16161b)
    }

    pub fn bg_elevated() -> Rgba {
        rgb(0x1e1e26)
    }

    pub fn bg_elevated_hover() -> Rgba {
        rgb(0x272733)
    }

    pub fn bg_card() -> Rgba {
        rgb(0x1a1a22)
    }

    // Borders
    pub fn border_subtle() -> Rgba {
        rgba(0xffffff0f)
    }

    pub fn border_medium() -> Rgba {
        rgba(0xffffff18)
    }

    pub fn border_focus() -> Rgba {
        rgba(0x6366f166)
    }

    // Foregrounds
    pub fn text_primary() -> Rgba {
        rgb(0xf4f4f5)
    }

    pub fn text_secondary() -> Rgba {
        rgb(0xa1a1aa)
    }

    pub fn text_muted() -> Rgba {
        rgb(0x71717a)
    }

    // Accents & Signals
    pub fn accent_purple() -> Rgba {
        rgb(0x6366f1)
    }

    pub fn accent_blue() -> Rgba {
        rgb(0x38bdf8)
    }

    pub fn success_green() -> Rgba {
        rgb(0x34d399)
    }

    pub fn warning_yellow() -> Rgba {
        rgb(0xfbbf24)
    }

    pub fn danger_red() -> Rgba {
        rgb(0xf87171)
    }

    // Traffic Lights
    pub fn traffic_red() -> Rgba {
        rgb(0xff5f56)
    }

    pub fn traffic_yellow() -> Rgba {
        rgb(0xffbd2e)
    }

    pub fn traffic_green() -> Rgba {
        rgb(0x27c93f)
    }
}
