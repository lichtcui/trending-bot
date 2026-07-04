# Multi-Source Trending 多源热点聚合 — 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 为 trending-bot 增加 Hacker News 和 Lobsters 数据源，统一结构化 JSON 输出，支持抓取外部链接内容（GitHub README / Web 文章正文）

**架构：** 统一 `TrendingItem` 数据模型 + 泛化 `TrendingSource` trait + 独立的 `ContentFetcher` 组件 + 多源统一缓存。各源顺序执行，外部内容按需抓取，所有输出汇聚为统一 JSON。

**技术栈：** Rust, reqwest (blocking), scraper, serde, chrono, serde_json

---

## 文件结构

```
src/
├── main.rs       # 重构：CLI 多源选择 + 编排流程
├── source.rs     # 重构：泛化 TrendingSource trait + GitHubTrending 适配 TrendingItem 输出
├── item.rs       # 新增：TrendingItem 统一数据模型
├── hn.rs         # 新增：HackerNews 实现
├── lobsters.rs   # 新增：Lobsters 实现
├── fetcher.rs    # 新增：ContentFetcher 内容抓取器
├── cache.rs      # 重构：UnifiedCache 多源缓存
├── output.rs     # 重构：AiOutput 适配多源
└── repo.rs       # 保留：Repo + parse_star_count（仅 GitHubTrending 内部使用）
```

### 任务 1：TrendingItem 统一数据模型

**文件：**
- 创建：`src/item.rs`
- 测试：内联 `#[cfg(test)]` 模块

- [ ] **步骤 1：编写 TrendingItem 及 ExternalContent 结构体测试**

```rust
// 文件末尾的测试模块
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
            description: Some("A test story".into()),
            score: Some(100),
            author: Some("testuser".into()),
            comments_url: Some("https://news.ycombinator.com/item?id=12345".into()),
            external_content: None,
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
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cargo test -- item::tests --nocapture`
预期：编译失败，提示 `TrendingItem` / `ExternalContent` / `ContentType` 未定义

- [ ] **步骤 3：编写最少实现代码**

```rust
use serde::Serialize;

/// 统一数据源条目
#[derive(Debug, Clone, Serialize)]
pub struct TrendingItem {
    pub source: String,
    pub id: String,
    pub title: String,
    pub url: String,
    pub description: Option<String>,
    pub score: Option<u64>,
    pub author: Option<String>,
    pub comments_url: Option<String>,
    pub external_content: Option<ExternalContent>,
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
    GitHubReadme,
    WebArticle,
}
```

需要在 `Cargo.toml` 中确认已有 `serde` with `derive` feature（已存在）。

- [ ] **步骤 4：运行测试验证通过**

运行：`cargo test -- item::tests --nocapture`
预期：PASS

- [ ] **步骤 5：Commit**

```bash
git add src/item.rs
git commit -m "feat: add TrendingItem unified data model"
```

---

### 任务 2：TrendingSource trait 泛化 + GitHubTrending 适配

**文件：**
- 修改：`src/source.rs`
- 创建：`src/repo.rs` 保持不变

- [ ] **步骤 1：编写测试 — 验证 GitHubTrending 实现新 trait 并输出 TrendingItem**

```rust
// 在 source.rs 的 tests 模块中

#[test]
fn test_github_trending_source_trait() {
    // TrendingSource trait 应该有 source_name() 和 fetch() 方法
    let source = GitHubTrending::new();
    assert_eq!(source.source_name(), "github_trending");

    // 用测试 HTML 验证返回 TrendingItem 而非 Repo
    let html = r#"
<article class="Box-row">
  <h2 class="h3 lh-condensed">
    <a class="Link" href="/rust-lang/rust">
      <span class="text-normal">rust-lang /</span> rust
    </a>
  </h2>
  <p class="col-9 color-fg-muted my-1 tmp-pr-4">A safe language.</p>
  <div class="f6 color-fg-muted mt-2">
    <span class="tmp-mr-3 d-inline-block">
      <span itemprop="programmingLanguage">Rust</span>
    </span>
    <a href="/rust-lang/rust/stargazers" class="tmp-mr-3 Link Link--muted d-inline-block">
      <svg class="octicon octicon-star"><path d="..."></path></svg>
      100k
    </a>
    <span class="d-inline-block float-sm-right">
      <svg class="octicon octicon-star"><path d="..."></path></svg>
      500 stars today
    </span>
  </div>
</article>
"#;

    // parse_from_html 应该保持原样返回 Vec<Repo>
    let repos = parse_from_html(html, 5).unwrap();
    assert_eq!(repos.len(), 1);

    // 新增一个转换函数: repos_to_items()
    let items = repos_to_items(&repos, "github_trending");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].source, "github_trending");
    assert_eq!(items[0].id, "rust-lang/rust");
    assert_eq!(items[0].title, "rust-lang/rust");
    assert_eq!(items[0].url, "https://github.com/rust-lang/rust");
    assert_eq!(items[0].description.as_deref(), Some("A safe language."));
    assert_eq!(items[0].score, Some(500));  // stars_today → score
    assert!(items[0].external_content.is_none());
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cargo test -- source::tests::test_github_trending_source_trait --nocapture`
预期：编译失败，`source_name()` / `repos_to_items()` 未定义，`TrendingItem` 可能未引入

- [ ] **步骤 3：实现修改**

在 `source.rs` 顶部添加 `use crate::item::TrendingItem;`

```rust
/// 数据源 trait — 泛化为多源支持
pub trait TrendingSource {
    fn source_name(&self) -> &'static str;
    fn fetch(&self, count: usize) -> anyhow::Result<Vec<TrendingItem>>;
}
```

为 `GitHubTrending` 添加 `source_name()`：

```rust
impl GitHubTrending {
    // ... 已有方法 ...
    pub fn source_name(&self) -> &'static str {
        "github_trending"
    }
}
```

映射函数：

```rust
/// 将 Repo 列表映射为统一 TrendingItem 列表
pub(crate) fn repos_to_items(repos: &[Repo], source_name: &str) -> Vec<TrendingItem> {
    repos.iter().map(|r| {
        let id = r.name.clone();
        let score = if r.stars_today > 0 { Some(r.stars_today) } else { None };
        TrendingItem {
            source: source_name.to_string(),
            id,
            title: r.name.clone(),
            url: r.url.clone(),
            description: r.description.clone(),
            score,
            author: None,
            comments_url: None,
            external_content: None,
        }
    }).collect()
}
```

更新 `TrendingSource` 实现：

```rust
impl TrendingSource for GitHubTrending {
    fn source_name(&self) -> &'static str {
        "github_trending"
    }

    fn fetch(&self, count: usize) -> anyhow::Result<Vec<TrendingItem>> {
        let url = "https://github.com/trending?since=daily";
        let html = self
            .client
            .get(url)
            .send()
            .context("请求 GitHub Trending 页面失败")?
            .text()
            .context("读取响应内容失败")?;

        let repos = parse_from_html(&html, count)?;
        Ok(repos_to_items(&repos, self.source_name()))
    }
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cargo test -- source::tests --nocapture`
预期：PASS

- [ ] **步骤 5：检查编译是否正常**

运行：`cargo build`
预期：编译成功（注意 main.rs 可能暂时报错，因为 `source.fetch_trending` 调用方式变了，下一任务调整）

- [ ] **步骤 6：Commit**

```bash
git add src/source.rs
git commit -m "refactor: generalize TrendingSource trait, adapt GitHubTrending to output TrendingItem"
```

---

### 任务 3：HackerNews 数据源

**文件：**
- 创建：`src/hn.rs`

- [ ] **步骤 1：编写测试 — 使用 mock JSON 数据验证 HN API 解析**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::TrendingItem;

    #[test]
    fn test_hacker_news_fetch() {
        // mock topstories.json
        let topstories = "[42424242, 42424243]";

        // mock item 42424242
        let item1 = r#"{
            "id": 42424242,
            "title": "Test Story",
            "url": "https://example.com/1",
            "score": 150,
            "by": "author1",
            "descendants": 42,
            "type": "story"
        }"#;

        // mock item 42424243 (ask hn — 无 url，应被过滤)
        let item2 = r#"{
            "id": 42424243,
            "title": "Ask HN: What are you working on?",
            "score": 50,
            "by": "author2",
            "descendants": 100,
            "type": "story"
        }"#;

        // 这里需要通过自定义 client 注入 mock 响应
        // 测试 parse_story 函数
        let story: serde_json::Value = serde_json::from_str(item1).unwrap();
        let parsed = parse_story(&story);
        assert!(parsed.is_some());
        let item = parsed.unwrap();
        assert_eq!(item.source, "hacker_news");
        assert_eq!(item.id, "story_42424242");
        assert_eq!(item.title, "Test Story");
        assert_eq!(item.url, "https://example.com/1");
        assert_eq!(item.score, Some(150));
        assert_eq!(item.author.as_deref(), Some("author1"));
        assert!(item.comments_url.is_some());

        // Ask HN 应被过滤（无 url 字段或 url 为 null/空）
        let ask: serde_json::Value = serde_json::from_str(item2).unwrap();
        let parsed2 = parse_story(&ask);
        assert!(parsed2.is_none(), "Ask HN without url should be filtered");
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cargo test -- hn::tests --nocapture`
预期：编译失败

- [ ] **步骤 3：实现 HackerNews 模块**

```rust
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
        return None; // 过滤 Ask HN / Show HN 等内部页面
    }
    let score = data.get("score").and_then(|v| v.as_u64());
    let author = data.get("by").and_then(|v| v.as_str().map(String::from));
    let descendants = data.get("descendants").and_then(|v| v.as_u64());
    let comments_url = descendants.map(|_| {
        format!("https://news.ycombinator.com/item?id={}", id)
    });

    Some(TrendingItem {
        source: "hacker_news".to_string(),
        id: format!("story_{}", id),
        title,
        url,
        description: None,
        score,
        author,
        comments_url,
        external_content: None,
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
        assert_eq!(item.author.as_deref(), Some("author1"));
        assert_eq!(item.comments_url.as_deref(), Some("https://news.ycombinator.com/item?id=42424242"));
    }

    #[test]
    fn test_parse_story_no_url_filtered() {
        let json = serde_json::json!({
            "id": 42424243,
            "title": "Ask HN: What are you working on?",
            "score": 50,
            "by": "author2",
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
            "by": "author3",
            "type": "story"
        });
        assert!(parse_story(&json).is_none());
    }

    #[test]
    fn test_parse_story_no_comments() {
        let json = serde_json::json!({
            "id": 42424245,
            "title": "No Comments",
            "url": "https://example.com/2",
            "score": 5,
            "by": "author4",
            "type": "story"
        });
        // descendants 缺失 → comments_url 应为 None
        let item = parse_story(&json).unwrap();
        assert!(item.comments_url.is_none());
    }
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cargo test -- hn::tests --nocapture`
预期：PASS

- [ ] **步骤 5：Commit**

```bash
git add src/hn.rs
git commit -m "feat: add HackerNews data source"
```

---

### 任务 4：Lobsters 数据源

**文件：**
- 创建：`src/lobsters.rs`

- [ ] **步骤 1：编写测试 — 使用 mock JSON 验证 Lobsters API 解析**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_lobsters_item() {
        let json = serde_json::json!([{
            "short_id": "abc123",
            "title": "A Lobsters Story",
            "url": "https://example.com/article",
            "score": 85,
            "submitter_user": {
                "username": "lobster_user"
            },
            "comment_count": 15,
            "comments_url": "https://lobste.rs/s/abc123",
            "tags": ["rust", "programming"],
            "description": "A great article about Rust"
        }]);
        let items = parse_stories(&json, "lobsters");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "lobsters");
        assert_eq!(items[0].id, "story_abc123");
        assert_eq!(items[0].title, "A Lobsters Story");
        assert_eq!(items[0].score, Some(85));
        assert_eq!(items[0].author.as_deref(), Some("lobster_user"));
        assert_eq!(items[0].description.as_deref(), Some("A great article about Rust"));
    }

    #[test]
    fn test_parse_lobsters_missing_fields() {
        // Lobsters API 有时某些字段可选
        let json = serde_json::json!([{
            "short_id": "def456",
            "title": "Minimal Post",
            "url": "https://example.com/minimal",
            "score": 10
        }]);
        let items = parse_stories(&json, "lobsters");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].author, None);
        assert_eq!(items[0].description, None);
        assert_eq!(items[0].comments_url, None);
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cargo test -- lobsters::tests --nocapture`
预期：编译失败

- [ ] **步骤 3：实现 Lobsters 模块**

```rust
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

        Ok(parse_stories(&data, self.source_name())
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
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cargo test -- lobsters::tests --nocapture`
预期：PASS

- [ ] **步骤 5：Commit**

```bash
git add src/lobsters.rs
git commit -m "feat: add Lobsters data source"
```

---

### 任务 5：ContentFetcher 内容抓取器

**文件：**
- 创建：`src/fetcher.rs`

- [ ] **步骤 1：编写测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::*;

    #[test]
    fn test_detect_github_url() {
        assert!(is_github_repo_url("https://github.com/rust-lang/rust"));
        assert!(is_github_repo_url("https://github.com/rust-lang/rust/"));
        assert!(is_github_repo_url("https://github.com/rust-lang/rust/issues/1"));
        assert!(!is_github_repo_url("https://example.com/article"));
        assert!(!is_github_repo_url("https://github.com"));  // 只有域名，没有路径
    }

    #[test]
    fn test_extract_repo_name() {
        assert_eq!(extract_repo_name("https://github.com/rust-lang/rust").unwrap(), ("rust-lang", "rust"));
        assert_eq!(extract_repo_name("https://github.com/rust-lang/rust/issues/1").unwrap(), ("rust-lang", "rust"));
        assert!(extract_repo_name("https://example.com").is_none());
    }

    #[test]
    fn test_extract_article_text() {
        let html = r#"<html><body>
            <nav>Navigation</nav>
            <article>
                <h1>Title</h1>
                <p>This is the article body text.</p>
                <p>Second paragraph with more content.</p>
            </article>
            <footer>Footer</footer>
        </body></html>"#;
        let text = extract_article_text(html, 5000);
        assert!(text.contains("This is the article body text."));
        assert!(text.contains("Second paragraph"));
        assert!(!text.contains("Navigation"));
        assert!(!text.contains("Footer"));
    }

    #[test]
    fn test_extract_article_fallback_to_main() {
        let html = r#"<html><body>
            <div role="main">
                <h1>Main Content</h1>
                <p>Body paragraph here.</p>
            </div>
        </body></html>"#;
        let text = extract_article_text(html, 5000);
        assert!(text.contains("Main Content"));
        assert!(text.contains("Body paragraph here."));
    }

    #[test]
    fn test_truncate_long_text() {
        let text = "word ".repeat(6000);
        let truncated = truncate_text(&text, 5000);
        assert!(truncated.len() <= 5000);
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn test_short_text_not_truncated() {
        let text = "short text";
        let truncated = truncate_text(text, 100);
        assert_eq!(truncated, text);
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cargo test -- fetcher::tests --nocapture`
预期：编译失败

- [ ] **步骤 3：实现 ContentFetcher 模块**

```rust
use anyhow::{Context, Result};
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
        let content_type = detect_content_type(url);

        match content_type {
            ContentType::GitHubReadme => self.fetch_github_readme(url),
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

        let text = resp.text().ok()?;
        let word_count = text.split_whitespace().count();

        Some(ExternalContent {
            url: format!("https://github.com/{}/{}/blob/main/README.md", owner, repo),
            content_type: ContentType::GitHubReadme,
            text: truncate_text(&text, 5000),
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
    } else {
        ContentType::WebArticle
    }
}

/// 判断是否为 GitHub repo URL
pub(crate) fn is_github_repo_url(url: &str) -> bool {
    let parts: Vec<&str> = url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .collect();
    parts.len() >= 3 && parts[0] == "github.com" && !parts[1].is_empty() && !parts[2].is_empty()
}

/// 提取 owner/repo
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
    let mut truncated = text[..max_chars].to_string();
    truncated.push_str("...");
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_github_url() {
        assert!(is_github_repo_url("https://github.com/rust-lang/rust"));
        assert!(is_github_repo_url("https://github.com/rust-lang/rust/"));
        assert!(is_github_repo_url("https://github.com/rust-lang/rust/issues/1"));
        assert!(!is_github_repo_url("https://example.com/article"));
        assert!(!is_github_repo_url("https://github.com"));
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
    fn test_detect_content_type() {
        assert_eq!(detect_content_type("https://github.com/rust-lang/rust"), ContentType::GitHubReadme);
        assert_eq!(detect_content_type("https://example.com/blog"), ContentType::WebArticle);
    }
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cargo test -- fetcher::tests --nocapture`
预期：PASS

- [ ] **步骤 5：Commit**

```bash
git add src/fetcher.rs
git commit -m "feat: add ContentFetcher for external link content extraction"
```

---

### 任务 6：UnifiedCache 多源缓存重构

**文件：**
- 修改：`src/cache.rs`

- [ ] **步骤 1：编写测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::TrendingItem;

    fn make_item(source: &str, id: &str, url: &str) -> TrendingItem {
        TrendingItem {
            source: source.to_string(),
            id: id.to_string(),
            title: "test".into(),
            url: url.to_string(),
            description: None,
            score: None,
            author: None,
            comments_url: None,
            external_content: None,
        }
    }

    #[test]
    fn test_unified_cache_format() {
        let cache = RepoCache::new_for_test(); // 用临时目录
        let items = vec![
            make_item("github_trending", "a/b", "https://github.com/a/b"),
            make_item("hacker_news", "story_1", "https://example.com/1"),
        ];
        cache.save_from_items(&items).unwrap();
        let loaded = cache.load_source("github_trending").unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "a/b");
    }

    #[test]
    fn test_diff_by_source() {
        let cache = RepoCache::new_for_test();
        let items = vec![
            make_item("github_trending", "a/b", "https://github.com/a/b"),
            make_item("github_trending", "c/d", "https://github.com/c/d"),
            make_item("hacker_news", "story_1", "https://example.com/1"),
        ];

        // 模拟缓存中已有部分数据
        let last: std::collections::HashMap<String, std::collections::HashSet<String>> = [
            ("github_trending".into(), vec!["a/b".into()].into_iter().collect()),
        ].into();

        let (old, new) = cache.diff_multi(&items, &last);
        assert_eq!(old.len(), 1);
        assert_eq!(new.len(), 2);
        assert!(new.iter().any(|i| i.id == "c/d"));
        assert!(new.iter().any(|i| i.id == "story_1"));
    }

    #[test]
    fn test_content_hash_cache() {
        let cache = RepoCache::new_for_test();
        let url = "https://github.com/rust-lang/rust";
        let hash = cache.compute_url_hash(url);
        assert_eq!(hash.len(), 16); // 取前16字符的 hex
        assert!(!cache.is_content_cached(url).unwrap());
    }

    #[test]
    fn test_old_format_migration() {
        // 写入旧格式的数据，验证加载时自动迁移
        let cache = RepoCache::new_for_test();
        let old_data = r#"{"date":"2026-07-04","names":["a/b","c/d"]}"#;
        let path = cache.cache_path();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, old_data).unwrap();

        let loaded = cache.load_all().unwrap();
        assert!(loaded.contains_key("github_trending"));
        let names: Vec<_> = loaded["github_trending"].iter().collect();
        assert_eq!(names.len(), 2);
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cargo test -- cache::tests --nocapture`
预期：编译失败（旧代码仍使用 `load_last_names` / `save_current_names` 旧接口）

- [ ] **步骤 3：实现 UnifiedCache 重构**

将 `src/cache.rs` 重写为：

```rust
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::item::TrendingItem;

/// 缓存条目（每个源每个项目）
#[derive(Debug, Serialize, Deserialize, Clone)]
struct CacheEntry {
    id: String,
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_hash: Option<String>,
}

/// 单个源缓存
#[derive(Debug, Serialize, Deserialize)]
struct SourceCache {
    date: String,
    items: Vec<CacheEntry>,
}

/// 缓存文件格式
#[derive(Debug, Serialize, Deserialize)]
struct CacheData {
    version: String,
    sources: HashMap<String, SourceCache>,
}

/// 旧格式（用于自动迁移）
#[derive(Debug, Deserialize)]
struct OldCacheData {
    date: String,
    names: Vec<String>,
}

pub struct RepoCache {
    cache_dir: PathBuf,
}

impl RepoCache {
    /// 使用 macOS 标准缓存目录
    pub fn new() -> Self {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        let cache_dir = PathBuf::from(home)
            .join("Library")
            .join("Caches")
            .join("trending-bot");
        RepoCache { cache_dir }
    }

    /// 仅用于测试 — 使用自定义目录
    pub fn new_with_dir(dir: PathBuf) -> Self {
        RepoCache { cache_dir: dir }
    }

    pub(crate) fn cache_path(&self) -> PathBuf {
        self.cache_dir.join("data_v2.json")
    }

    /// 加载全部缓存，自动迁移旧格式
    pub fn load_all(&self) -> Result<HashMap<String, HashSet<String>>> {
        let path = self.cache_path();
        if !path.exists() {
            // 尝试加载旧格式
            return self.try_migrate_old_format();
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("读取缓存文件失败: {}", path.display()))?;
        let data: CacheData = serde_json::from_str(&content)
            .with_context(|| format!("解析缓存文件失败: {}", path.display()))?;

        Ok(data.sources.into_iter().map(|(k, v)| {
            (k, v.items.into_iter().map(|e| e.id).collect())
        }).collect())
    }

    /// 尝试加载并迁移旧格式缓存
    fn try_migrate_old_format(&self) -> Result<HashMap<String, HashSet<String>>> {
        let old_path = self.cache_dir.join("last_repos.json");
        if !old_path.exists() {
            return Ok(HashMap::new());
        }

        // 读取旧格式
        let content = fs::read_to_string(&old_path)?;
        let old: OldCacheData = serde_json::from_str(&content)?;
        let names: HashSet<String> = old.names.into_iter().collect();

        // 写入新版
        let today = old.date;
        let mut sources = HashMap::new();
        sources.insert("github_trending".to_string(), SourceCache {
            date: today,
            items: names.iter().map(|n| CacheEntry {
                id: n.clone(),
                url: format!("https://github.com/{}", n),
                content_hash: None,
            }).collect(),
        });

        let new_data = CacheData {
            version: "2".to_string(),
            sources,
        };

        fs::create_dir_all(&self.cache_dir)?;
        fs::write(&path, serde_json::to_string_pretty(&new_data)?)?;

        // 删除旧文件
        let _ = fs::remove_file(&old_path);

        let mut result = HashMap::new();
        result.insert("github_trending".to_string(), names);
        Ok(result)
    }

    /// 加载缓存中已抓取的内容 URL hash 集合
    pub fn load_content_hashes(&self) -> Result<HashMap<String, String>> {
        let path = self.cache_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = fs::read_to_string(&path)?;
        let data: CacheData = serde_json::from_str(&content)?;

        let mut hashes = HashMap::new();
        for (_source, sc) in data.sources {
            for entry in sc.items {
                if let Some(h) = entry.content_hash {
                    hashes.insert(entry.url, h);
                }
            }
        }
        Ok(hashes)
    }

    /// 计算 URL 的短 hash
    pub fn compute_url_hash(&self, url: &str) -> String {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        url.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    /// 内容是否已在缓存中（通过 URL hash 判断）
    pub fn is_content_cached(&self, url: &str) -> Result<bool> {
        let hashes = self.load_content_hashes()?;
        Ok(hashes.contains_key(url))
    }

    /// 保存当前项目列表到缓存
    pub fn save_from_items(&self, items: &[TrendingItem]) -> Result<()> {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();

        let mut sources: HashMap<String, Vec<CacheEntry>> = HashMap::new();
        for item in items {
            let entries = sources.entry(item.source.clone()).or_default();
            let content_hash = item.external_content.as_ref().map(|_| {
                self.compute_url_hash(&item.url)
            });
            entries.push(CacheEntry {
                id: item.id.clone(),
                url: item.url.clone(),
                content_hash,
            });
        }

        let data = CacheData {
            version: "2".to_string(),
            sources: sources.into_iter().map(|(k, v)| {
                (k, SourceCache { date: today.clone(), items: v })
            }).collect(),
        };

        fs::create_dir_all(&self.cache_dir)?;
        let content = serde_json::to_string_pretty(&data)?;
        fs::write(self.cache_path(), content)?;
        Ok(())
    }

    /// 多源差异对比
    pub fn diff_multi<'a>(
        &self,
        items: &'a [TrendingItem],
        last_by_source: &HashMap<String, HashSet<String>>,
    ) -> (Vec<&'a TrendingItem>, Vec<&'a TrendingItem>) {
        let mut old = Vec::new();
        let mut new = Vec::new();
        for item in items {
            let last_ids = last_by_source.get(&item.source);
            let is_old = last_ids.map_or(false, |ids| ids.contains(&item.id));
            if is_old {
                old.push(item);
            } else {
                new.push(item);
            }
        }
        (old, new)
    }
}

impl Default for RepoCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // 辅助：创建临时缓存
    fn make_cache() -> (RepoCache, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let cache = RepoCache::new_with_dir(dir.path().to_path_buf());
        (cache, dir)
    }

    fn make_item(source: &str, id: &str, url: &str) -> TrendingItem {
        TrendingItem {
            source: source.to_string(),
            id: id.to_string(),
            title: "test".into(),
            url: url.to_string(),
            description: None,
            score: None,
            author: None,
            comments_url: None,
            external_content: None,
        }
    }

    #[test]
    fn test_save_and_load_empty() {
        let (cache, _dir) = make_cache();
        let loaded = cache.load_all().unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_save_and_load_items() {
        let (cache, _dir) = make_cache();
        let items = vec![
            make_item("github_trending", "a/b", "https://github.com/a/b"),
            make_item("hacker_news", "story_1", "https://example.com/1"),
        ];
        cache.save_from_items(&items).unwrap();
        let loaded = cache.load_all().unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded["github_trending"].len(), 1);
        assert!(loaded["github_trending"].contains("a/b"));
        assert_eq!(loaded["hacker_news"].len(), 1);
        assert!(loaded["hacker_news"].contains("story_1"));
    }

    #[test]
    fn test_diff_multi_all_new() {
        let (cache, _dir) = make_cache();
        let items = vec![
            make_item("github_trending", "a/b", ""),
            make_item("hacker_news", "story_1", ""),
        ];
        let last = HashMap::new();
        let (old, new) = cache.diff_multi(&items, &last);
        assert_eq!(old.len(), 0);
        assert_eq!(new.len(), 2);
    }

    #[test]
    fn test_diff_multi_partial() {
        let (cache, _dir) = make_cache();
        let items = vec![
            make_item("github_trending", "a/b", ""),
            make_item("github_trending", "c/d", ""),
            make_item("hacker_news", "story_1", ""),
        ];
        let mut last = HashMap::new();
        last.insert("github_trending".into(), vec!["a/b".into()].into_iter().collect());
        let (old, new) = cache.diff_multi(&items, &last);
        assert_eq!(old.len(), 1);
        assert_eq!(old[0].id, "a/b");
        assert_eq!(new.len(), 2);
    }

    #[test]
    fn test_old_format_migration() {
        let (cache, dir) = make_cache();
        let old_path = dir.path().join("last_repos.json");
        let old_data = r#"{"date":"2026-07-04","names":["a/b","c/d"]}"#;
        fs::write(&old_path, old_data).unwrap();

        let loaded = cache.load_all().unwrap();
        assert_eq!(loaded.len(), 1);
        assert!(loaded.contains_key("github_trending"));
        assert_eq!(loaded["github_trending"].len(), 2);
        assert!(loaded["github_trending"].contains("a/b"));

        // 旧文件应已被删除
        assert!(!old_path.exists());
        // 新文件应存在
        assert!(cache.cache_path().exists());
    }

    #[test]
    fn test_content_hash() {
        let (cache, _dir) = make_cache();
        let hash1 = cache.compute_url_hash("https://github.com/rust-lang/rust");
        let hash2 = cache.compute_url_hash("https://github.com/rust-lang/rust");
        assert_eq!(hash1, hash2);
        let hash3 = cache.compute_url_hash("https://example.com");
        assert_ne!(hash1, hash3);
    }
}
```

注意：需要在 `Cargo.toml` 中添加 `tempfile` 作为开发依赖：

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cargo test -- cache::tests --nocapture`
预期：PASS

- [ ] **步骤 5：Commit**

```bash
git add src/cache.rs Cargo.toml
git commit -m "refactor: upgrade cache to multi-source UnifiedCache with auto-migration"
```

---

### 任务 7：Output 适配多源

**文件：**
- 修改：`src/output.rs`

- [ ] **步骤 1：编写测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::TrendingItem;

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
        let fetched = 1;
        let cached = 0;
        let failed = 0;
        let by_source = [("github_trending".into(), SourceDiff { new: 1, old: 0 })].into();

        let output = AiOutput::new(&items, &new, &old, fetched, cached, failed, &by_source);
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
        ].into();

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
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cargo test -- output::tests --nocapture`
预期：编译失败

- [ ] **步骤 3：实现 Output 适配**

```rust
use std::collections::HashMap;
use serde::Serialize;
use crate::item::TrendingItem;

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

#[derive(Debug, Serialize)]
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
```

主要改动点：
- `repos: Vec<Repo>` → `items: Vec<TrendingItem>`
- `new_repos: Vec<String>` → `new_items: Vec<String>`
- 增加 `fetched_content`, `cached_content`, `failed_content`, `by_source`

- [ ] **步骤 4：运行测试验证通过**

运行：`cargo test -- output::tests --nocapture`
预期：PASS

- [ ] **步骤 5：Commit**

```bash
git add src/output.rs
git commit -m "refactor: adapt AiOutput for multi-source support"
```

---

### 任务 8：Main.rs 编排

**文件：**
- 修改：`src/main.rs`

- [ ] **步骤 1：编写集成测试（可选）**
    
main.rs 的流程编排很难纯单元测试，主要通过 `cargo build` 验证编译通过。集成测试通过手动运行验证。

- [ ] **步骤 2：重构 main.rs**

```rust
mod cache;
mod fetcher;
mod hn;
mod item;
mod lobsters;
mod output;
mod repo;
mod source;

use std::collections::{HashMap, HashSet};

use anyhow::Result;

use cache::RepoCache;
use item::TrendingItem;
use output::{AiOutput, SourceDiff};
use source::TrendingSource;

fn main() -> Result<()> {
    // 简单的 CLI 参数解析
    let args: Vec<String> = std::env::args().collect();
    let json_mode = args.iter().any(|a| a == "--json");
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let fetch_content = !args.iter().any(|a| a == "--no-content");

    // 解析 --count N 或 -c N
    let count: usize = args.windows(2)
        .find(|w| w[0] == "--count" || w[0] == "-c")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(5);

    // 解析 --source / -s（默认全部）
    let enabled_sources: Vec<String> = {
        let specified: Vec<String> = args.windows(2)
            .filter(|w| w[0] == "--source" || w[0] == "-s")
            .map(|w| w[1].to_lowercase())
            .collect();
        if specified.is_empty() {
            vec!["github".into(), "hn".into(), "lobsters".into()]
        } else {
            specified
        }
    };

    // 1. 初始化各 Source
    let mut sources: Vec<Box<dyn TrendingSource>> = Vec::new();
    for name in &enabled_sources {
        match name.as_str() {
            "github" => sources.push(Box::new(source::GitHubTrending::new())),
            "hn" => sources.push(Box::new(hn::HackerNews::new())),
            "lobsters" => sources.push(Box::new(lobsters::Lobsters::new())),
            _ => eprintln!("⚠️ 未知数据源: {}，跳过", name),
        }
    }

    // 2. 顺序获取各源数据
    let mut all_items: Vec<TrendingItem> = Vec::new();
    for source in &sources {
        match source.fetch(count) {
            Ok(mut items) => {
                eprintln!("✓ {} 获取到 {} 条", source.source_name(), items.len());
                all_items.append(&mut items);
            }
            Err(e) => {
                eprintln!("⚠️ {} 获取失败: {}", source.source_name(), e);
            }
        }
    }

    // 3. 缓存对比
    let cache = RepoCache::new();
    let last_data = match cache.load_all() {
        Ok(data) => data,
        Err(e) => {
            eprintln!("⚠️ 读取缓存失败，跳过: {}", e);
            HashMap::new()
        }
    };

    let (old_items, new_items) = cache.diff_multi(&all_items, &last_data);

    // 4. 内容抓取（仅对新项目 + 已缓存但无内容的项目）
    let mut fetched_content = 0usize;
    let mut cached_content = 0usize;
    let mut failed_content = 0usize;

    if fetch_content {
        let fetcher = fetcher::ContentFetcher::new();
        let cached_urls: HashSet<String> = cache.load_content_hashes()
            .unwrap_or_default()
            .keys()
            .cloned()
            .collect();

        // 只对新项目抓取内容
        for item in &mut all_items {
            if new_items.iter().any(|ni| ni.id == item.id) {
                if cached_urls.contains(&item.url) {
                    cached_content += 1;
                    continue;
                }
                match fetcher.fetch(item) {
                    Some(content) => {
                        item.external_content = Some(content);
                        fetched_content += 1;
                    }
                    None => {
                        failed_content += 1;
                    }
                }
            }
        }
    }

    // 5. 统计各源
    let mut by_source: HashMap<String, SourceDiff> = HashMap::new();
    for item in &all_items {
        let entry = by_source.entry(item.source.clone()).or_insert(SourceDiff { new: 0, old: 0 });
        if new_items.iter().any(|ni| ni.id == item.id) {
            entry.new += 1;
        } else {
            entry.old += 1;
        }
    }

    // 6. JSON 输出
    if json_mode {
        let output = AiOutput::new(
            &all_items,
            &new_items,
            &old_items,
            fetched_content,
            cached_content,
            failed_content,
            &by_source,
        );
        let json = serde_json::to_string_pretty(&output)
            .context("序列化 JSON 输出失败")?;
        println!("{}", json);
    } else {
        // 非 JSON 模式：简要输出
        println!("=== 多源热点汇总 ===");
        for item in &all_items {
            let tag = if new_items.iter().any(|ni| ni.id == item.id) { "NEW" } else { "   " };
            println!("[{}] [{}] {} - {}", tag, item.source, item.title, item.url);
        }
        println!("\n缓存: {} 新 / {} 旧", new_items.len(), old_items.len());
        if fetch_content {
            println!("内容: {} 新抓取 / {} 缓存命中 / {} 失败", fetched_content, cached_content, failed_content);
        }
    }

    // 7. 更新缓存（同时保存内容 hash）
    if !dry_run && !all_items.is_empty() {
        if let Err(e) = cache.save_from_items(&all_items) {
            eprintln!("⚠️ 更新缓存失败: {}", e);
        }
    }

    Ok(())
}
```

- [ ] **步骤 3：验证编译通过**

运行：`cargo build`
预期：编译成功

- [ ] **步骤 4：验证测试全部通过**

运行：`cargo test`
预期：全部 PASS

- [ ] **步骤 5：手动运行验证**

```bash
# 默认运行（全部源，前5条）
cargo run -- --json

# 仅 HN 源
cargo run -- --source hn --json

# 仅 GitHub + HN
cargo run -- -s github -s hn --json

# 不抓取外部内容
cargo run -- --json --no-content

# 预览模式
cargo run -- --json --dry-run
```

预期：输出结构化 JSON，包含各源数据及缓存信息

- [ ] **步骤 6：Commit**

```bash
git add src/main.rs
git commit -m "feat: multi-source orchestration with HN and Lobsters"
```

---

## 自检

- [ ] **规格覆盖度：** 设计文档中的每个章节在计划中都有对应任务
- [ ] **占位符扫描：** 无 TODO / 待定 / "后续补充" 等占位符
- [ ] **类型一致性：** `TrendingItem` 在所有文件中签名一致，`source_name()` 返回 `&'static str`
- [ ] **测试覆盖：** 每个新模块都有单元测试，关键函数有边界情况测试
