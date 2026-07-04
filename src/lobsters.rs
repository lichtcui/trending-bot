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
        Some(TrendingItem {
            source: source.to_string(),
            id: format!("story_{}", short_id),
            title,
            url,
            score,
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
            "submitter_user": "lobster_user",
            "comment_count": 15,
            "tags": ["rust"]
        }]);
        let items = parse_stories(json.as_array().unwrap(), "lobsters");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "lobsters");
        assert_eq!(items[0].id, "story_abc123");
        assert_eq!(items[0].score, Some(85));
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
        assert_eq!(items[0].id, "story_def456");
        assert_eq!(items[0].title, "Minimal Post");
    }

    #[test]
    fn test_parse_lobsters_empty() {
        let items = parse_stories(&[], "lobsters");
        assert!(items.is_empty());
    }

    #[test]
    fn test_parse_lobsters_reality_format() {
        // Real Lobsters /hottest.json: submitter_user is a plain string, not an object
        // description can be empty or HTML, url can be empty for self-posts
        let json = serde_json::json!([{
            "short_id": "real1",
            "title": "Real Lobsters Story",
            "url": "https://example.com/real",
            "score": 99,
            "submitter_user": "real_user"
        }]);
        let items = parse_stories(json.as_array().unwrap(), "lobsters");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "lobsters");
        assert_eq!(items[0].id, "story_real1");
        assert_eq!(items[0].score, Some(99));
    }

    #[test]
    fn test_parse_lobsters_self_post() {
        // Self-posts on Lobsters have empty url
        let json = serde_json::json!([{
            "short_id": "self1",
            "title": "Ask Lobsters: Something",
            "url": "",
            "score": 5,
            "submitter_user": "asker"
        }]);
        let items = parse_stories(json.as_array().unwrap(), "lobsters");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].url, "");
    }

    #[test]
    fn test_parse_lobsters_missing_required_field() {
        // Items missing short_id or title should be filtered out
        let json = serde_json::json!([
            {
                "short_id": "ok1",
                "title": "Valid Story",
                "url": "https://example.com/ok"
            },
            {
                // missing short_id
                "title": "No Short ID",
                "url": "https://example.com/no-id"
            },
            {
                "short_id": "no-title",
                // missing title
                "url": "https://example.com/no-title"
            }
        ]);
        let items = parse_stories(json.as_array().unwrap(), "lobsters");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "story_ok1");
    }
}
