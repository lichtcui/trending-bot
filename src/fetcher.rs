use scraper::{Html, Selector};

use crate::item::{ContentType, ExternalContent, TrendingItem};

/// 内容抓取器
pub struct ContentFetcher {
    client: reqwest::blocking::Client,
}

impl ContentFetcher {
    pub fn new() -> Self {
        let client = reqwest::blocking::Client::builder()
            .user_agent("trending-bot/0.1.0")
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("创建 HTTP 客户端失败");
        ContentFetcher { client }
    }

    pub fn with_client(client: reqwest::blocking::Client) -> Self {
        ContentFetcher { client }
    }

    /// 抓取一个帖子的外部链接内容
    pub fn fetch(&self, item: &TrendingItem) -> Option<ExternalContent> {
        let url = &item.url;
        match detect_content_type(url) {
            ContentType::GitHubReadme => self.fetch_github_readme(url),
            ContentType::GitHubIssue => self.fetch_github_issue(url),
            ContentType::WebArticle => self.fetch_web_article(url),
        }
    }

    fn fetch_github_readme(&self, url: &str) -> Option<ExternalContent> {
        let (owner, repo) = extract_repo_name(url)?;
        let api_url = format!("https://api.github.com/repos/{}/{}/readme", owner, repo);

        let resp = self.client
            .get(&api_url)
            .header("User-Agent", "trending-bot/0.1.0")
            .header("Accept", "application/vnd.github.raw")
            .send()
            .ok()?;

        if !resp.status().is_success() {
            return None;
        }

        let text = resp.text().ok()?;
        let truncated = truncate_text(&text, 5000);
        let word_count = truncated.split_whitespace().count();

        Some(ExternalContent {
            url: format!("https://github.com/{}/{}/blob/main/README.md", owner, repo),
            content_type: ContentType::GitHubReadme,
            text: truncated,
            word_count,
        })
    }

    fn fetch_github_issue(&self, url: &str) -> Option<ExternalContent> {
        let (owner, repo) = extract_repo_name(url)?;
        // 从 URL 中提取 issue number
        let issue_number = url.split('/')
            .filter_map(|s| s.parse::<u64>().ok())
            .next()?;
        let api_url = format!("https://api.github.com/repos/{}/{}/issues/{}", owner, repo, issue_number);

        let resp = self.client
            .get(&api_url)
            .header("User-Agent", "trending-bot/0.1.0")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .ok()?;

        if !resp.status().is_success() {
            return None;
        }

        let data: serde_json::Value = resp.json().ok()?;
        let title = data.get("title").and_then(|v| v.as_str()).unwrap_or("");
        let body = data.get("body").and_then(|v| v.as_str()).unwrap_or("");

        let full_text = format!("# {}\n\n{}", title, body);
        let truncated = truncate_text(&full_text, 5000);
        let word_count = truncated.split_whitespace().count();

        Some(ExternalContent {
            url: url.to_string(),
            content_type: ContentType::GitHubIssue,
            text: truncated,
            word_count,
        })
    }

    fn fetch_web_article(&self, url: &str) -> Option<ExternalContent> {
        let resp = self.client
            .get(url)
            .header("User-Agent", "Mozilla/5.0 (compatible; trending-bot/0.1.0)")
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .ok()?;

        let html = resp.text().ok()?;
        let text = extract_article_text(&html, 5000);
        let word_count = text.split_whitespace().count();

        Some(ExternalContent {
            url: url.to_string(),
            content_type: ContentType::WebArticle,
            text,
            word_count,
        })
    }

    /// 批量抓取，跳过重复 URL
    pub fn fetch_batch(&self, items: &[TrendingItem], skip_urls: &std::collections::HashSet<String>) -> Vec<(String, Option<ExternalContent>)> {
        let mut seen = skip_urls.clone();
        let mut results = Vec::new();

        for item in items {
            if seen.contains(&item.url) {
                results.push((item.id.clone(), None));
                continue;
            }
            seen.insert(item.url.clone());
            let content = self.fetch(item);
            results.push((item.id.clone(), content));
        }

        results
    }
}

impl Default for ContentFetcher {
    fn default() -> Self {
        Self::new()
    }
}

/// 检测内容类型
pub(crate) fn detect_content_type(url: &str) -> ContentType {
    if is_github_repo_url(url) {
        ContentType::GitHubReadme
    } else if is_github_issue_url(url) {
        ContentType::GitHubIssue
    } else {
        ContentType::WebArticle
    }
}

/// 判断是否为 GitHub repo 主页 URL（精确匹配，排除 issue/PR/tree 等子页面）
/// 匹配格式: github.com/owner/repo 或 github.com/owner/repo/
pub(crate) fn is_github_repo_url(url: &str) -> bool {
    let parts: Vec<&str> = url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    parts.len() == 3 && parts[0] == "github.com" && !parts[1].is_empty() && !parts[2].is_empty()
}

/// 判断是否为 GitHub Issue URL
/// 匹配格式: github.com/owner/repo/issues/N
pub(crate) fn is_github_issue_url(url: &str) -> bool {
    let parts: Vec<&str> = url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    parts.len() >= 5
        && parts[0] == "github.com"
        && !parts[1].is_empty()
        && !parts[2].is_empty()
        && parts[3] == "issues"
        && parts[4].parse::<u64>().is_ok()
}

/// 提取 owner/repo（同时兼容 repo 主页和 issue URL）
pub(crate) fn extract_repo_name(url: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .collect();
    if parts.len() >= 3 && parts[0] == "github.com" {
        Some((parts[1].to_string(), parts[2].to_string()))
    } else {
        None
    }
}

/// 从 HTML 提取正文文本
pub(crate) fn extract_article_text(html: &str, max_chars: usize) -> String {
    let doc = Html::parse_document(html);

    // 优先取 <article>
    if let Ok(sel) = Selector::parse("article") {
        if let Some(article) = doc.select(&sel).next() {
            let text = article.text().collect::<Vec<_>>().join(" ");
            let cleaned = clean_text(&text);
            return truncate_text(&cleaned, max_chars);
        }
    }

    // 其次取 <main> 或 role="main"
    if let Ok(sel) = Selector::parse("main, [role='main']") {
        if let Some(main) = doc.select(&sel).next() {
            let text = main.text().collect::<Vec<_>>().join(" ");
            let cleaned = clean_text(&text);
            return truncate_text(&cleaned, max_chars);
        }
    }

    // 退回到 body
    if let Ok(sel) = Selector::parse("body") {
        if let Some(body) = doc.select(&sel).next() {
            let text = body.text().collect::<Vec<_>>().join(" ");
            let cleaned = clean_text(&text);
            return truncate_text(&cleaned, max_chars);
        }
    }

    String::new()
}

fn clean_text(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

pub(crate) fn truncate_text(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        return text.to_string();
    }
    // 安全截断：找到不超过 max_chars 的最新 UTF-8 字符边界，避免 panic
    // 使用 char_indices 逐个检查字符结束位置是否在边界内
    let cutoff = text
        .char_indices()
        .take_while(|(i, c)| i + c.len_utf8() <= max_chars)
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    let mut truncated = text[..cutoff].to_string();
    truncated.push_str("...");
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_github_repo_url() {
        assert!(is_github_repo_url("https://github.com/rust-lang/rust"));
        assert!(is_github_repo_url("https://github.com/rust-lang/rust/"));
        // Issue URLs 不再是 repo URL
        assert!(!is_github_repo_url("https://github.com/rust-lang/rust/issues/1"));
        assert!(!is_github_repo_url("https://github.com/rust-lang/rust/pull/42"));
        assert!(!is_github_repo_url("https://example.com/article"));
        assert!(!is_github_repo_url("https://github.com"));
    }

    #[test]
    fn test_is_github_issue_url() {
        assert!(is_github_issue_url("https://github.com/rust-lang/rust/issues/1"));
        assert!(is_github_issue_url("https://github.com/anthropics/claude-code/issues/74066"));
        // Repo 主页不是 issue
        assert!(!is_github_issue_url("https://github.com/rust-lang/rust"));
        assert!(!is_github_issue_url("https://github.com/rust-lang/rust/"));
        // PR 不是 issue
        assert!(!is_github_issue_url("https://github.com/rust-lang/rust/pull/42"));
        // 非 GitHub 不是 issue
        assert!(!is_github_issue_url("https://example.com/article"));
    }

    #[test]
    fn test_extract_repo_name() {
        let (owner, repo) = extract_repo_name("https://github.com/rust-lang/rust").unwrap();
        assert_eq!(owner, "rust-lang");
        assert_eq!(repo, "rust");
        let (owner2, repo2) = extract_repo_name("https://github.com/denoland/deno/issues/1").unwrap();
        assert_eq!(owner2, "denoland");
        assert_eq!(repo2, "deno");
        assert!(extract_repo_name("https://example.com").is_none());
    }

    #[test]
    fn test_extract_article_text_with_article_tag() {
        let html = r#"<html><body>
            <nav>Skip Nav</nav>
            <article>
                <h1>Title</h1>
                <p>Article body text here.</p>
                <p>Second paragraph.</p>
            </article>
            <footer>Skip Footer</footer>
        </body></html>"#;
        let text = extract_article_text(html, 5000);
        assert!(text.contains("Article body text here."));
        assert!(text.contains("Second paragraph"));
        assert!(!text.contains("Skip Nav"));
        assert!(!text.contains("Skip Footer"));
    }

    #[test]
    fn test_extract_article_fallback_to_main() {
        let html = r#"<html><body>
            <div role="main">
                <h1>Main Content</h1>
                <p>Body paragraph text.</p>
            </div>
        </body></html>"#;
        let text = extract_article_text(html, 5000);
        assert!(text.contains("Main Content"));
        assert!(text.contains("Body paragraph text."));
    }

    #[test]
    fn test_extract_article_empty() {
        assert_eq!(extract_article_text("<html></html>", 100), "");
    }

    #[test]
    fn test_truncate_text() {
        let long = "a".repeat(6000);
        let truncated = truncate_text(&long, 5000);
        assert_eq!(truncated.len(), 5003); // 5000 + "..."
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn test_short_text_not_truncated() {
        let text = "hello world";
        assert_eq!(truncate_text(text, 100), text);
    }

    #[test]
    fn test_truncate_multibyte_char_boundary() {
        // 中文文本，如果按字节截断会 panic，校验安全回退到字符边界
        let text = "你好世界，这是一个测试";
        let truncated = truncate_text(text, 10); // 字节 10 落在 3 字节字符中间
        assert!(truncated.ends_with("..."));
        // '世'结束于字节 8（6+3），回退到字节 9（'世'的结尾），内容 9 字节 + "..." = 12 字节
        assert_eq!(truncated.len(), 12);
        assert_eq!(truncated, "你好世...");
    }

    #[test]
    fn test_detect_content_type() {
        assert_eq!(detect_content_type("https://github.com/rust-lang/rust"), ContentType::GitHubReadme);
        assert_eq!(detect_content_type("https://github.com/rust-lang/rust/"), ContentType::GitHubReadme);
        assert_eq!(detect_content_type("https://github.com/rust-lang/rust/issues/123"), ContentType::GitHubIssue);
        assert_eq!(detect_content_type("https://github.com/anthropics/claude-code/issues/74066"), ContentType::GitHubIssue);
        assert_eq!(detect_content_type("https://example.com/blog"), ContentType::WebArticle);
        // PR 暂时走 WebArticle 抓取 HTML
        assert_eq!(detect_content_type("https://github.com/rust-lang/rust/pull/42"), ContentType::WebArticle);
    }
}
