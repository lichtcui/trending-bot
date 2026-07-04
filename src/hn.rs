use anyhow::{Context, Result};
use crate::item::TrendingItem;
use crate::source::TrendingSource;

pub struct HackerNews {
    client: reqwest::blocking::Client,
}

impl HackerNews {
    pub fn new() -> Self {
        let client = reqwest::blocking::Client::builder()
            .user_agent("trending-bot/0.1.0")
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("创建 HTTP 客户端失败");
        HackerNews { client }
    }

    pub fn with_client(client: reqwest::blocking::Client) -> Self {
        HackerNews { client }
    }
}

impl Default for HackerNews {
    fn default() -> Self {
        Self::new()
    }
}

impl TrendingSource for HackerNews {
    fn source_name(&self) -> &'static str {
        "hacker_news"
    }

    fn fetch(&self, count: usize) -> Result<Vec<TrendingItem>> {
        // 1. 获取 top stories ID 列表
        let ids_url = "https://hacker-news.firebaseio.com/v0/topstories.json";
        let ids: Vec<u64> = self.client
            .get(ids_url)
            .send()
            .context("请求 HN topstories 失败")?
            .json()
            .context("解析 HN topstories JSON 失败")?;

        // 2. 取前 count 个 ID
        let target_ids: Vec<u64> = ids.into_iter().take(count).collect();

        // 3. 逐个获取详情
        let mut items = Vec::new();
        for id in target_ids {
            let item_url = format!("https://hacker-news.firebaseio.com/v0/item/{}.json", id);
            match self.client.get(&item_url).send() {
                Ok(resp) => {
                    match resp.json::<serde_json::Value>() {
                        Ok(data) => {
                            if let Some(item) = parse_story(&data) {
                                items.push(item);
                            }
                        }
                        Err(e) => {
                            eprintln!("⚠️ HN item {} JSON 解析失败: {}", id, e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("⚠️ HN item {} 获取失败: {}", id, e);
                }
            }
        }

        Ok(items)
    }
}

fn parse_story(data: &serde_json::Value) -> Option<TrendingItem> {
    let id = data.get("id")?.as_u64()?;
    let title = data.get("title")?.as_str()?.to_string();
    let url = data.get("url")?.as_str()?.to_string();
    if url.is_empty() {
        return None; // 过滤无外部链接的条目（如某些文本帖）
    }
    let score = data.get("score").and_then(|v| v.as_u64());
    Some(TrendingItem {
        source: "hacker_news".to_string(),
        id: format!("story_{}", id),
        title,
        url,
        score,
        external_content: None,
        summary: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_story_normal() {
        let json = serde_json::json!({
            "id": 42424242,
            "title": "Test Story",
            "url": "https://example.com/1",
            "score": 150,
            "by": "author1",
            "descendants": 42,
            "type": "story"
        });
        let item = parse_story(&json).unwrap();
        assert_eq!(item.source, "hacker_news");
        assert_eq!(item.id, "story_42424242");
        assert_eq!(item.title, "Test Story");
        assert_eq!(item.url, "https://example.com/1");
        assert_eq!(item.score, Some(150));
    }

    #[test]
    fn test_parse_story_no_url_filtered() {
        let json = serde_json::json!({
            "id": 42424243,
            "title": "Ask HN: What are you working on?",
            "score": 50,
            "type": "story"
        });
        assert!(parse_story(&json).is_none());
    }

    #[test]
    fn test_parse_story_empty_url_filtered() {
        let json = serde_json::json!({
            "id": 42424244,
            "title": "Some post",
            "url": "",
            "score": 10,
            "type": "story"
        });
        assert!(parse_story(&json).is_none());
    }


}
