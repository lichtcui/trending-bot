use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use scraper::{Html, Selector};

use crate::item::TrendingItem;

const RUST_WEEKLY_RSS: &str = "https://this-week-in-rust.org/rss.xml";
const BYTEBYTEGO_RSS: &str = "https://blog.bytebytego.com/feed";
const AI_WEEKLY_RSS: &str = "https://aiweekly.co/feed";

/// 获取 This Week in Rust 的最新一期，展开 HTML 内链为独立 TrendingItem
pub fn fetch_rust_weekly(client: &Client, count: usize) -> Result<Vec<TrendingItem>> {
    let xml = client
        .get(RUST_WEEKLY_RSS)
        .send()
        .context("请求 Rust Weekly RSS 失败")?
        .text()
        .context("读取 RSS 响应失败")?;
    parse_rust_weekly_from_rss(&xml, count)
}

/// 从 RSS XML 字符串中解析 Rust Weekly，展开 HTML 链接为独立条目
fn parse_rust_weekly_from_rss(xml: &str, count: usize) -> Result<Vec<TrendingItem>> {
    let channel = rss::Channel::read_from(xml.as_bytes())
        .context("解析 Rust Weekly RSS XML 失败")?;

    let latest = channel.items().first()
        .context("Rust Weekly RSS 无任何 item")?;

    let title = latest.title().unwrap_or("This Week in Rust");
    let issue_num = extract_issue_number(title);
    let description = latest.description().unwrap_or("");

    // 从 HTML 中提取所有 <a href>
    let doc = Html::parse_fragment(description);
    let link_sel = Selector::parse("a[href]")
        .map_err(|e| anyhow::anyhow!("CSS 选择器解析失败: {}", e))?;

    let items: Vec<TrendingItem> = doc
        .select(&link_sel)
        .filter_map(|el| {
            let href = el.value().attr("href")?;
            // 跳过空链接和锚点链接
            if href.is_empty() || href.starts_with('#') {
                return None;
            }
            let text: String = el.text().collect::<Vec<_>>().concat();
            let text = text.trim().to_string();
            // 跳过无文本或链接文本等于 URL 的条目
            if text.is_empty() || text == href {
                return None;
            }
            // 处理相对路径
            let url = if href.starts_with("http") {
                href.to_string()
            } else if href.starts_with('/') {
                format!("https://this-week-in-rust.org{}", href)
            } else {
                href.to_string()
            };
            let id = format!("twir_{}_{}", issue_num, compute_url_hash(&url));
            Some(TrendingItem {
                source: "rust_weekly".to_string(),
                id,
                title: text,
                url,
                score: None,
                external_content: None,
                summary: None,
            })
        })
        .take(count)
        .collect();

    Ok(items)
}

/// 从标题中提取期号："This Week in Rust 658" → "658"
fn extract_issue_number(title: &str) -> &str {
    if title.is_empty() {
        return "0";
    }
    let last = title.rsplit(' ').next().unwrap_or("0");
    // 确保返回的是纯数字，否则回退到 "0"
    if last.chars().all(|c| c.is_ascii_digit()) {
        last
    } else {
        "0"
    }
}

/// 获取 ByteByteGo Newsletter 最新文章
pub fn fetch_bytebytego(client: &Client, count: usize) -> Result<Vec<TrendingItem>> {
    let xml = client
        .get(BYTEBYTEGO_RSS)
        .send()
        .context("请求 ByteByteGo RSS 失败")?
        .text()
        .context("读取 ByteByteGo RSS 响应失败")?;
    parse_generic_rss_items(&xml, "bytebytego", count)
}

/// 获取 AI Weekly 最新文章
pub fn fetch_ai_weekly(client: &Client, count: usize) -> Result<Vec<TrendingItem>> {
    let xml = client
        .get(AI_WEEKLY_RSS)
        .send()
        .context("请求 AI Weekly RSS 失败")?
        .text()
        .context("读取 AI Weekly RSS 响应失败")?;
    parse_generic_rss_items(&xml, "ai_weekly", count)
}

/// 通用 RSS 条目解析：将 RSS feed 中每个 item 映射为 TrendingItem
/// 适用于 ByteByteGo / AI Weekly 这类每个 item 就是一条独立文章的 feed
fn parse_generic_rss_items(xml: &str, source_name: &str, count: usize) -> Result<Vec<TrendingItem>> {
    let channel = rss::Channel::read_from(xml.as_bytes())
        .with_context(|| format!("解析 {} RSS XML 失败", source_name))?;

    let items: Vec<TrendingItem> = channel.items().iter()
        .filter_map(|item| {
            let title = item.title()?.to_string();
            let url = item.link()?.to_string();
            if url.is_empty() {
                return None;
            }
            let prefix = match source_name {
                "bytebytego" => "bbg",
                "ai_weekly" => "aiw",
                _ => source_name,
            };
            let id = format!("{}_{}", prefix, compute_url_hash(&url));
            Some(TrendingItem {
                source: source_name.to_string(),
                id,
                title,
                url,
                score: None,
                external_content: None,
                summary: None,
            })
        })
        .take(count)
        .collect();

    Ok(items)
}

/// 计算 URL 的 16 位 hex hash
fn compute_url_hash(url: &str) -> String {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch_rust_weekly_rss_parse_and_expand() {
        let rss_xml = r#"<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0"><channel><title>This Week in Rust</title><link>https://this-week-in-rust.org/</link>
<item>
<title>This Week in Rust 658</title>
<link>https://this-week-in-rust.org/blog/2026/07/01/this-week-in-rust-658/</link>
<description>&lt;h2 id=&quot;official&quot;&gt;Official&lt;/h2&gt;
&lt;ul&gt;
&lt;li&gt;&lt;a href=&quot;https://blog.rust-lang.org/2026/06/30/Rust-1.96.1/&quot;&gt;Announcing Rust 1.96.1&lt;/a&gt;&lt;/li&gt;
&lt;li&gt;&lt;a href=&quot;https://blog.rust-lang.org/2026/06/25/vision-doc/&quot;&gt;The many journeys of learning Rust&lt;/a&gt;&lt;/li&gt;
&lt;/ul&gt;
&lt;h3&gt;Project/Tooling Updates&lt;/h3&gt;
&lt;ul&gt;
&lt;li&gt;&lt;a href=&quot;https://slint.dev/blog/slint-1.17-released&quot;&gt;Slint 1.17 Released&lt;/a&gt;&lt;/li&gt;
&lt;/ul&gt;</description>
</item></channel></rss>"#;

        let items = parse_rust_weekly_from_rss(rss_xml, 10).unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].source, "rust_weekly");
        assert!(items[0].id.starts_with("twir_658_"));
        assert_eq!(items[0].title, "Announcing Rust 1.96.1");
        assert_eq!(items[0].url, "https://blog.rust-lang.org/2026/06/30/Rust-1.96.1/");
        assert!(items[0].score.is_none());

        assert_eq!(items[1].title, "The many journeys of learning Rust");
        assert_eq!(items[2].title, "Slint 1.17 Released");
    }

    #[test]
    fn test_extract_issue_number() {
        assert_eq!(extract_issue_number("This Week in Rust 658"), "658");
        assert_eq!(extract_issue_number("This Week in Rust 1"), "1");
        assert_eq!(extract_issue_number(""), "0");
    }

    #[test]
    fn test_rust_weekly_empty_html() {
        let xml = r#"<?xml version="1.0"?>
<rss version="2.0"><channel><title>Test</title>
<item><title>Issue 1</title><link>https://x.com</link><description></description></item>
</channel></rss>"#;
        let items = parse_rust_weekly_from_rss(xml, 10).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn test_rust_weekly_skip_anchor_links() {
        let xml = r#"<?xml version="1.0"?>
<rss version="2.0"><channel><title>Test</title>
<item><title>Issue 1</title><link>https://x.com</link>
<description>&lt;a href=&quot;#toc&quot;&gt;Table of Contents&lt;/a&gt;</description>
</item></channel></rss>"#;
        let items = parse_rust_weekly_from_rss(xml, 10).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn test_rust_weekly_count_limit() {
        let xml = r#"<?xml version="1.0"?>
<rss version="2.0"><channel><title>Test</title>
<item><title>Issue 1</title><link>https://x.com</link>
<description>&lt;a href=&quot;https://a.com&quot;&gt;Link A&lt;/a&gt;&lt;a href=&quot;https://b.com&quot;&gt;Link B&lt;/a&gt;&lt;a href=&quot;https://c.com&quot;&gt;Link C&lt;/a&gt;</description>
</item></channel></rss>"#;
        let items = parse_rust_weekly_from_rss(xml, 2).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Link A");
        assert_eq!(items[1].title, "Link B");
    }

    #[test]
    fn test_rust_weekly_relative_url() {
        let xml = r#"<?xml version="1.0"?>
<rss version="2.0"><channel><title>Test</title>
<item><title>Issue 1</title><link>https://x.com</link>
<description>&lt;a href=&quot;/blog/2026/01/01/post/&quot;&gt;Relative Post&lt;/a&gt;</description>
</item></channel></rss>"#;
        let items = parse_rust_weekly_from_rss(xml, 10).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].url, "https://this-week-in-rust.org/blog/2026/01/01/post/");
    }

    #[test]
    fn test_parse_bytebytego_rss_items() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
<channel><title>ByteByteGo Newsletter</title>
<item>
<title>AI Routing: When to Save Money</title>
<link>https://blog.bytebytego.com/p/ai-routing</link>
<description>A deep dive into model routing...</description>
<guid>https://blog.bytebytego.com/p/ai-routing</guid>
</item>
<item>
<title>How Discord Scales</title>
<link>https://blog.bytebytego.com/p/discord-scale</link>
<description>Discord architecture...</description>
<guid>https://blog.bytebytego.com/p/discord-scale</guid>
</item>
</channel></rss>"#;

        let items = parse_generic_rss_items(xml, "bytebytego", 10).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].source, "bytebytego");
        assert_eq!(items[0].title, "AI Routing: When to Save Money");
        assert_eq!(items[0].url, "https://blog.bytebytego.com/p/ai-routing");
        assert!(items[0].id.starts_with("bbg_"));
    }

    #[test]
    fn test_parse_ai_weekly_rss_items() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
<channel><title>AI Weekly</title>
<item>
<title>Anthropic Files IPO</title>
<link>https://aiweekly.co/issues/anthropic-files-ipo</link>
<description>Anthropic filed S-1...</description>
<guid>https://aiweekly.co/issues/anthropic-files-ipo</guid>
</item>
</channel></rss>"#;

        let items = parse_generic_rss_items(xml, "ai_weekly", 10).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "ai_weekly");
        assert_eq!(items[0].title, "Anthropic Files IPO");
        assert!(items[0].id.starts_with("aiw_"));
    }

    #[test]
    fn test_generic_rss_empty_channel() {
        let xml = r#"<?xml version="1.0"?>
<rss version="2.0"><channel><title>Empty</title></channel></rss>"#;
        let items = parse_generic_rss_items(xml, "test_source", 10).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn test_generic_rss_count_limit() {
        let mut xml = r#"<?xml version="1.0"?>
<rss version="2.0"><channel><title>Test</title>"#.to_string();
        for i in 0..10 {
            xml.push_str(&format!(
                r#"<item><title>Article {}</title><link>https://x.com/{}</link></item>"#, i, i
            ));
        }
        xml.push_str("</channel></rss>");
        let items = parse_generic_rss_items(&xml, "test", 3).unwrap();
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn test_compute_url_hash_consistency() {
        let h1 = compute_url_hash("https://example.com");
        let h2 = compute_url_hash("https://example.com");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 16);
        assert!(h1.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
