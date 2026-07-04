use crate::item::TrendingItem;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Serialize)]
pub struct AiOutput {
    pub tool: String,
    pub version: &'static str,
    pub fetched_at: String,
    pub count: usize,
    pub items: Vec<TrendingItem>,
    pub cache: CacheContext,
}

#[derive(Debug, Serialize)]
pub struct CacheContext {
    pub status: String,
    pub new_count: usize,
    pub old_count: usize,
    pub new_items: Vec<String>,
    pub is_duplicate: bool,
    pub fetched_content: usize,
    pub cached_content: usize,
    pub failed_content: usize,
    pub by_source: HashMap<String, SourceDiff>,
}

#[derive(Debug, Serialize, Clone)]
pub struct SourceDiff {
    pub new: usize,
    pub old: usize,
}

impl AiOutput {
    pub fn new(
        items: &[TrendingItem],
        new_items: &[&TrendingItem],
        old_items: &[&TrendingItem],
        fetched_content: usize,
        cached_content: usize,
        failed_content: usize,
        by_source: &HashMap<String, SourceDiff>,
    ) -> Self {
        let (status, is_duplicate) = if old_items.is_empty() && !new_items.is_empty() {
            ("all_new".to_string(), false)
        } else if !new_items.is_empty() {
            ("partial_update".to_string(), false)
        } else {
            ("no_change".to_string(), true)
        };

        let new_item_ids: Vec<String> = new_items.iter().map(|i| i.id.clone()).collect();

        AiOutput {
            tool: "trending-bot".to_string(),
            version: env!("CARGO_PKG_VERSION"),
            fetched_at: chrono::Local::now().to_rfc3339(),
            count: items.len(),
            items: items.to_vec(),
            cache: CacheContext {
                status,
                new_count: new_items.len(),
                old_count: old_items.len(),
                new_items: new_item_ids,
                is_duplicate,
                fetched_content,
                cached_content,
                failed_content,
                by_source: by_source.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_item(source: &str, id: &str) -> TrendingItem {
        TrendingItem {
            source: source.to_string(),
            id: id.to_string(),
            title: "test".into(),
            url: "https://example.com".into(),
            description: None,
            score: None,
            author: None,
            comments_url: None,
            external_content: None,
        }
    }

    #[test]
    fn test_ai_output_cache_status_all_new() {
        let items = vec![make_item("github_trending", "a/b")];
        let new = vec![&items[0]];
        let old: Vec<&TrendingItem> = vec![];
        let by_source = [("github_trending".into(), SourceDiff { new: 1, old: 0 })].into();

        let output = AiOutput::new(&items, &new, &old, 1, 0, 0, &by_source);
        assert_eq!(output.cache.status, "all_new");
        assert_eq!(output.cache.new_count, 1);
        assert_eq!(output.cache.fetched_content, 1);
    }

    #[test]
    fn test_ai_output_cache_status_partial() {
        let items = vec![
            make_item("github_trending", "a/b"),
            make_item("hacker_news", "story_1"),
        ];
        let new = vec![&items[1]];
        let old = vec![&items[0]];
        let by_source = [
            ("github_trending".into(), SourceDiff { new: 0, old: 1 }),
            ("hacker_news".into(), SourceDiff { new: 1, old: 0 }),
        ]
        .into();

        let output = AiOutput::new(&items, &new, &old, 1, 0, 0, &by_source);
        assert_eq!(output.cache.status, "partial_update");
        assert_eq!(output.cache.new_count, 1);
        assert_eq!(output.cache.old_count, 1);
        assert_eq!(output.cache.by_source["hacker_news"].new, 1);
    }

    #[test]
    fn test_ai_output_cache_status_no_change() {
        let items = vec![make_item("github_trending", "a/b")];
        let new: Vec<&TrendingItem> = vec![];
        let old = vec![&items[0]];
        let by_source = [("github_trending".into(), SourceDiff { new: 0, old: 1 })].into();

        let output = AiOutput::new(&items, &new, &old, 0, 0, 0, &by_source);
        assert_eq!(output.cache.status, "no_change");
        assert!(output.cache.is_duplicate);
    }

    #[test]
    fn test_ai_output_contains_items() {
        let items = vec![
            make_item("github_trending", "a/b"),
            make_item("lobsters", "story_abc"),
        ];
        let new = vec![&items[0], &items[1]];
        let old = vec![];
        let by_source = HashMap::new();

        let output = AiOutput::new(&items, &new, &old, 0, 0, 0, &by_source);
        assert_eq!(output.count, 2);
        assert_eq!(output.items.len(), 2);
        assert_eq!(output.items[0].source, "github_trending");
        assert_eq!(output.items[1].source, "lobsters");
    }
}
