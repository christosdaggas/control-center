//! Internationalization support using gettext.
//!
//! Provides the [`init`] function to bind the text domain at startup,
//! and the [`tr`] function for translating UI strings at runtime.

use gettextrs::{bindtextdomain, gettext, setlocale, textdomain, LocaleCategory};
use tracing::{debug, warn};

/// The gettext text domain, matching the application name.
const DOMAIN: &str = "control-center";

/// Standard system locale directory for compiled `.mo` files.
const LOCALE_DIR: &str = "/usr/share/locale";

/// Initializes gettext for the application.
///
/// Must be called early in `main()`, before any translated strings are used.
pub fn init() {
    setlocale(LocaleCategory::LcAll, "");

    // Try development path first (po/ next to the executable)
    let dev_locale_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("../../po")))
        .and_then(|p| p.canonicalize().ok());

    let locale_dir = if let Some(ref dev_dir) = dev_locale_dir {
        if dev_dir.exists() {
            debug!(path = %dev_dir.display(), "Using development locale directory");
            dev_dir.to_str().unwrap_or(LOCALE_DIR)
        } else {
            LOCALE_DIR
        }
    } else {
        LOCALE_DIR
    };

    if let Err(e) = bindtextdomain(DOMAIN, locale_dir) {
        warn!(error = %e, "Failed to bind text domain");
    }

    if let Err(e) = textdomain(DOMAIN) {
        warn!(error = %e, "Failed to set text domain");
    }

    debug!(domain = DOMAIN, locale_dir, "Gettext initialized");
}

/// Translates a string using gettext.
///
/// This is the primary entry point for translating UI strings.
/// In source code, wrap user-visible strings like: `tr("Hello")`
#[inline]
pub fn tr(msg: &str) -> String {
    gettext(msg)
}
