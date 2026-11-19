//! The product's config directory name.

// xtask:allow(game-name) reason: the working product title, not game knowledge. The name is an open owner decision (doc 14 § Product name); it changes here when settled.

/// `%APPDATA%\<this>` on Windows.
pub(crate) const APP_DIR_WINDOWS: &str = "StellarisModDoctor";
/// `$XDG_CONFIG_HOME/<this>` elsewhere.
pub(crate) const APP_DIR_XDG: &str = "stellaris-mod-doctor";
