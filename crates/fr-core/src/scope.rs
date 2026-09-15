//! Provider scopes, mirroring Nest's `Scope.DEFAULT | REQUEST | TRANSIENT`.

/// Lifetime scope of an injected provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scope {
    /// One instance for the application's lifetime. Default.
    #[default]
    Singleton,
    /// A fresh instance per incoming request.
    Request,
    /// A fresh instance every time it is injected.
    Transient,
}

impl Scope {
    /// Parse a scope from a string literal used in `#[injectable(scope = "...")]`.
    pub fn parse_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "request" => Scope::Request,
            "transient" => Scope::Transient,
            _ => Scope::Singleton,
        }
    }
}
