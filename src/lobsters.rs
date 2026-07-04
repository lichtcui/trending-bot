use anyhow::{Context, Result};
use crate::item::TrendingItem;
use crate::source::TrendingSource;

pub struct Lobsters {
    client: reqwest::blocking::Client,
}

impl Lobsters {
    pub fn new() -> Self {
        let client = reqwest::blocking::Client::builder()
            .user_agent("trending-bot/0.1.0")
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("创建 HTTP 客户端失败");
        Lobsters { client }
    }

    pub fn with_client(client: reqwest::blocking::Client) -> Self {
        Lobsters { client }
    }
}

impl Default for Lobsters {
    fn default() -> Self {
        Self::new()
    }
}

impl TrendingSource for Lobsters {
    fn source_name(&self) -> &'static str {
        "lobsters"
    }

    fn fetch(&self, count: usize) -> Result<Vec<TrendingItem>> {
        let url = "https://lobste.rs/hottest.json";
        let data: Vec<serde_json::Value> = self.client
            .get(url)
            .send()
            .context("请求 Lobsters 热点列表失败")?
            .json()
            .context("解析 Lobsters JSON 失败")?;

        Ok(parse_stories(&data, "lobsters")
            .into_iter()
            .take(count)
            .collect())
    }
}

fn parse_stories(data: &[serde_json::Value], source: &str) -> Vec<TrendingItem> {
    data.iter().filter_map(|item| {
        let short_id = item.get("short_id")?.as_str()?;
        let title = item.get("title")?.as_str()?.to_string();
        let url = item.get("url")?.as_str()?.to_string();
        let score = item.get("score").and_then(|v| v.as_u64());
        let author = item.get("submitter_user")
            .and_then(|u| u.get("username"))
            .and_then(|v| v.as_str())
            .map(String::from);
        let description = item.get("description")
            .and_then(|v| v.as_str())
            .map(String::from)
            .filter(|s| !s.is_empty());
        let comments_url = item.get("comments_url")
            .and_then(|v| v.as_str())
            .map(String::from);

        Some(TrendingItem {
            source: source.to_string(),
            id: format!("story_{}", short_id),
            title,
            url,
            description,
            score,
            author,
            comments_url,
            external_content: None,
        })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_lobsters_full() {
        let json = serde_json::json!([{
            "short_id": "abc123",
            "title": "A Lobsters Story",
            "url": "https://example.com/article",
            "score": 85,
            "submitter_user": { "username": "lobster_user" },
            "comment_count": 15,
            "comments_url": "https://lobste.rs/s/abc123",
            "tags": ["rust"],
            "description": "A great article about Rust"
        }]);
        let items = parse_stories(json.as_array().unwrap(), "lobsters");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "lobsters");
        assert_eq!(items[0].id, "story_abc123");
        assert_eq!(items[0].score, Some(85));
        assert_eq!(items[0].author.as_deref(), Some("lobster_user"));
        assert_eq!(items[0].description.as_deref(), Some("A great article about Rust"));
    }

    #[test]
    fn test_parse_lobsters_minimal() {
        let json = serde_json::json!([{
            "short_id": "def456",
            "title": "Minimal Post",
            "url": "https://example.com/minimal",
            "score": 10
        }]);
        let items = parse_stories(json.as_array().unwrap(), "lobsters");
        assert_eq!(items.len(), 1);
        assert!(items[0].author.is_none());
        assert!(items[0].description.is_none());
        assert!(items[0].comments_url.is_none());
    }

    #[test]
    fn test_parse_lobsters_empty() {
        let items = parse_stories(&[], "lobsters");
        assert!(items.is_empty());
    }
}
