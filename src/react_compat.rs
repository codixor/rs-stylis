//! React-compatible normalization policy.
//!
//! This module only owns the profile switch used by the real parser, minifier,
//! and printer stages. It must not contain product-specific or source-string
//! rewrite logic.

/// Controls how valid CSS is normalized during printing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NormalizationProfile {
    /// Prefer standards-oriented normalization and minification.
    #[default]
    Standards,
    /// Prefer React-like emitted text when multiple valid serializations exist.
    ReactCompat,
}

impl NormalizationProfile {
    /// Returns whether the profile prefers the JS React-compatible output shape.
    pub const fn is_react_compat(self) -> bool {
        matches!(self, Self::ReactCompat)
    }

    /// Returns whether the trailing semicolon should be preserved in minified output.
    pub const fn preserve_trailing_semicolon(self) -> bool {
        self.is_react_compat()
    }
}
