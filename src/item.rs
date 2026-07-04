use serde::Serialize;

/// 统一数据源条目
#[derive(Debug, Clone, Serialize)]
pub struct TrendingItem {
    pub source: String,
    pub id: String,
    pub title: String,
    pub url: String,
    pub score: Option<u64>,
    #[serde(skip)]
    pub external_content: Option<ExternalContent>,
    /// LLM 生成的总结文本（供 AI 消费）
    pub summary: Option<String>,
}

/// 链接背后抓取的内容
#[derive(Debug, Clone, Serialize)]
pub struct ExternalContent {
    pub url: String,
    pub content_type: ContentType,
    pub text: String,
    pub word_count: usize,
}

/// 内容类型
#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum ContentType {
    /// GitHub 仓库的 README
    GitHubReadme,
    /// GitHub Issue 内容
    GitHubIssue,
    /// 普通网页文章
    WebArticle,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trending_item_construction() {
        let item = TrendingItem {
            source: "hacker_news".into(),
            id: "story_12345".into(),
            title: "Test Story".into(),
            url: "https://example.com".into(),
            score: Some(100),
            external_content: None,
            summary: None,
        };
        assert_eq!(item.source, "hacker_news");
        assert_eq!(item.id, "story_12345");
    }

    #[test]
    fn test_external_content_github_readme() {
        let content = ExternalContent {
            url: "https://github.com/rust-lang/rust".into(),
            content_type: ContentType::GitHubReadme,
            text: "# Rust\n\nA safe language.".into(),
            word_count: 5,
        };
        assert_eq!(content.content_type, ContentType::GitHubReadme);
        assert_eq!(content.word_count, 5);
    }

    #[test]
    fn test_external_content_web_article() {
        let content = ExternalContent {
            url: "https://example.com/blog".into(),
            content_type: ContentType::WebArticle,
            text: "Some article text.".into(),
            word_count: 3,
        };
        assert_eq!(content.content_type, ContentType::WebArticle);
    }

    #[test]
    fn test_trending_item_with_content() {
        let content = ExternalContent {
            url: "https://github.com/rust-lang/rust".into(),
            content_type: ContentType::GitHubReadme,
            text: "# Rust\n\nA safe, concurrent, practical language.".into(),
            word_count: 7,
        };
        let item = TrendingItem {
            source: "hacker_news".into(),
            id: "story_4242".into(),
            title: "Rust is great".into(),
            url: "https://github.com/rust-lang/rust".into(),
            score: None,
            external_content: Some(content),
            summary: None,
        };
        assert!(item.external_content.is_some());
        assert_eq!(item.external_content.as_ref().unwrap().content_type, ContentType::GitHubReadme);
    }
}
