use serde::{Deserialize, Serialize};

use crate::PluginSource;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Marketplace {
    pub plugins: Vec<MarketplaceEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MarketplaceEntry {
    pub source: PluginSource,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_marketplace_with_string_sources() {
        let json = r#"{"plugins": [{"source": "owner/repo"}]}"#;
        let m: Marketplace = serde_json::from_str(json).unwrap();
        assert_eq!(m.plugins.len(), 1);
        assert!(matches!(m.plugins[0].source, PluginSource::Relative(_)));
    }

    #[test]
    fn deserialize_marketplace_with_object_sources() {
        let json = r#"{"plugins": [{"source": {"github": "owner/repo"}}]}"#;
        let m: Marketplace = serde_json::from_str(json).unwrap();
        assert_eq!(m.plugins.len(), 1);
        assert!(matches!(m.plugins[0].source, PluginSource::GitHub { .. }));
    }

    #[test]
    fn deserialize_marketplace_with_repo_alias() {
        let json = r#"{"plugins": [{"source": {"repo": "owner/repo"}}]}"#;
        let m: Marketplace = serde_json::from_str(json).unwrap();
        assert_eq!(m.plugins.len(), 1);
        assert!(matches!(m.plugins[0].source, PluginSource::GitHub { .. }));
    }

    #[test]
    fn deserialize_marketplace_with_url_source() {
        let json = r#"{"plugins": [{"source": {"url": "https://example.com/plugin.zip"}}]}"#;
        let m: Marketplace = serde_json::from_str(json).unwrap();
        assert_eq!(m.plugins.len(), 1);
        assert!(matches!(m.plugins[0].source, PluginSource::Url { .. }));
    }

    #[test]
    fn deserialize_marketplace_mixed_sources() {
        let json = r#"{
            "plugins": [
                {"source": "relative/path"},
                {"source": {"github": "owner/repo"}},
                {"source": {"url": "https://example.com/plugin.zip"}}
            ]
        }"#;
        let m: Marketplace = serde_json::from_str(json).unwrap();
        assert_eq!(m.plugins.len(), 3);
    }
}
