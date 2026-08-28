use serde::{Deserialize, Serialize};

/// Playwright stealth and fingerprint evasion configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StealthConfig {
    pub mask_webdriver: bool,
    pub spoof_user_agent: bool,
    pub user_agent: String,
    pub emulate_webgl_vendor: String,
    pub emulate_webgl_renderer: String,
    pub realistic_mouse_curves: bool,
    pub solve_captchas: bool,
}

impl Default for StealthConfig {
    fn default() -> Self {
        Self {
            mask_webdriver: true,
            spoof_user_agent: true,
            user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36".to_string(),
            emulate_webgl_vendor: "Apple Inc.".to_string(),
            emulate_webgl_renderer: "Apple M4".to_string(),
            realistic_mouse_curves: true,
            solve_captchas: false,
        }
    }
}

/// JavaScript snippet injected into Playwright before page load to mask automation fingerprints.
pub const STEALTH_INJECTION_SCRIPT: &str = r#"
(() => {
    // 1. Overwrite navigator.webdriver
    Object.defineProperty(navigator, 'webdriver', {
        get: () => undefined,
    });

    // 2. Mock chrome object
    window.chrome = {
        runtime: {},
        loadTimes: function() {},
        csi: function() {},
        app: {},
    };

    // 3. Mock permissions API
    const originalQuery = window.navigator.permissions.query;
    window.navigator.permissions.query = (parameters) => (
        parameters.name === 'notifications' ?
            Promise.resolve({ state: Notification.permission }) :
            originalQuery(parameters)
    );

    // 4. Overwrite plugins length
    Object.defineProperty(navigator, 'plugins', {
        get: () => [1, 2, 3, 4, 5],
    });
})();
"#;
