use anyhow::{Context, Result};
use scraper::{Html, Selector};

use crate::item::TrendingItem;
use crate::repo::{parse_star_count, Repo};

/// 预编译的 CSS 选择器，避免每行重复编译
struct TrendingSelectors {
    link: Selector,
    description: Selector,
    language: Selector,
    star_total: Selector,
    star_today: Selector,
}

impl TrendingSelectors {
    fn new() -> Result<Self> {
        Ok(TrendingSelectors {
            link: Selector::parse("h2.h3.lh-condensed a")
                .map_err(|e| anyhow::anyhow!("CSS 选择器解析失败: {}", e))?,
            description: Selector::parse("p.col-9.color-fg-muted")
                .map_err(|e| anyhow::anyhow!("CSS 选择器解析失败: {}", e))?,
            language: Selector::parse("span[itemprop='programmingLanguage']")
                .map_err(|e| anyhow::anyhow!("CSS 选择器解析失败: {}", e))?,
            star_total: Selector::parse("a[href$='/stargazers']")
                .map_err(|e| anyhow::anyhow!("CSS 选择器解析失败: {}", e))?,
            star_today: Selector::parse("span.d-inline-block.float-sm-right")
                .map_err(|e| anyhow::anyhow!("CSS 选择器解析失败: {}", e))?,
        })
    }
}

/// 数据源 trait — 泛化为多源支持
pub trait TrendingSource {
    fn source_name(&self) -> &'static str;
    fn fetch(&self, count: usize) -> Result<Vec<TrendingItem>>;
}

/// GitHub Trending 页面抓取器
pub struct GitHubTrending {
    client: reqwest::blocking::Client,
}

impl GitHubTrending {
    /// 创建抓取器（使用默认 HTTP Client）
    pub fn new() -> Self {
        let client = reqwest::blocking::Client::builder()
            .user_agent("Mozilla/5.0 (compatible; trending-bot/0.1.0)")
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("创建 HTTP 客户端失败");
        GitHubTrending { client }
    }

    /// 使用自定义 HTTP Client 创建抓取器（方便测试时注入 mock Client）
    #[allow(dead_code)]
    pub fn with_client(client: reqwest::blocking::Client) -> Self {
        GitHubTrending { client }
    }

    /// 从单个 HTML 行元素中解析 Repo
    ///
    /// `selectors` 由 `parse_from_html` 预先编译好并传入，避免每行重复编译。
    fn parse_repo(row: &scraper::ElementRef, selectors: &TrendingSelectors) -> Option<Repo> {
        // --- 名称 + URL ---
        let link = row.select(&selectors.link).next()?;
        let href = link.value().attr("href")?;
        let name = href
            .trim_start_matches('/')
            .split('/')
            .collect::<Vec<_>>()
            .join("/");
        let url = format!("https://github.com{}", href);

        // --- 描述 ---
        let description = row
            .select(&selectors.description)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .filter(|s| !s.is_empty());

        // --- 编程语言 ---
        let language = row
            .select(&selectors.language)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .filter(|s| !s.is_empty());

        // --- 总 Star 数 ---
        // 从 stargazers 链接的文本中提取（跳过 SVG 图标文本）
        let stars_total = row
            .select(&selectors.star_total)
            .next()
            .and_then(|e| {
                let text: String = e.text().collect();
                // 过滤掉 SVG 图标的内容，只保留数字
                let cleaned: String = text.chars().filter(|c| c.is_ascii_digit() || *c == ',' || *c == '.' || *c == 'k' || *c == 'm' || *c == 'K' || *c == 'M').collect();
                parse_star_count(&cleaned)
            })
            .unwrap_or(0);

        // --- 今日 Star 数 ---
        // 文本形如 "2,804 stars today" → 提取第一个数字
        let stars_today = row
            .select(&selectors.star_today)
            .next()
            .and_then(|e| {
                let text = e.text().collect::<String>();
                text.split_whitespace()
                    .find(|s| s.chars().next().is_some_and(|c| c.is_ascii_digit()))
                    .and_then(|n| n.replace(',', "").parse::<u64>().ok())
            })
            .unwrap_or(0);

        Some(Repo {
            name,
            url,
            description,
            language,
            stars_total,
            stars_today,
        })
    }
}

impl Default for GitHubTrending {
    fn default() -> Self {
        Self::new()
    }
}

impl GitHubTrending {
    pub fn source_name(&self) -> &'static str {
        "github_trending"
    }
}

impl TrendingSource for GitHubTrending {
    fn source_name(&self) -> &'static str {
        "github_trending"
    }

    fn fetch(&self, count: usize) -> Result<Vec<TrendingItem>> {
        let url = "https://github.com/trending?since=daily";
        let html = self
            .client
            .get(url)
            .send()
            .context("请求 GitHub Trending 页面失败")?
            .text()
            .context("读取响应内容失败")?;

        let repos = parse_from_html(&html, count)?;
        Ok(repos_to_items(&repos, "github_trending"))
    }
}

/// 从 HTML 文本中解析 Trending 项目列表
pub(crate) fn parse_from_html(html: &str, count: usize) -> Result<Vec<Repo>> {
    let doc = Html::parse_document(html);
    let row_sel =
        Selector::parse("article.Box-row").map_err(|e| anyhow::anyhow!("CSS 选择器解析失败: {}", e))?;

    // 编译所有 CSS 选择器一次，在所有行中复用
    let selectors = TrendingSelectors::new()?;

    let repos: Vec<Repo> = doc
        .select(&row_sel)
        .filter_map(|row| GitHubTrending::parse_repo(&row, &selectors))
        .take(count)
        .collect();

    anyhow::ensure!(
        !repos.is_empty(),
        "未从 Trending 页面解析到任何项目，页面结构可能已变更"
    );

    Ok(repos)
}

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
            score,
            external_content: None,
            summary: None,
        }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_repo_name_from_href() {
        let html = r#"
<article class="Box-row">
  <h2 class="h3 lh-condensed">
    <a data-view-component="true" class="Link" href="/rust-lang/rust">
      <span data-view-component="true" class="text-normal">rust-lang /</span>
      rust
    </a>
  </h2>
  <p class="col-9 color-fg-muted my-1 tmp-pr-4">
    A safe, concurrent, practical language.
  </p>
  <div class="f6 color-fg-muted mt-2">
    <span class="tmp-mr-3 d-inline-block ml-0 tmp-ml-0">
      <span class="repo-language-color" style="background-color: #3572A5"></span>
      <span itemprop="programmingLanguage">Rust</span>
    </span>
    <a href="/rust-lang/rust/stargazers" data-view-component="true" class="tmp-mr-3 Link Link--muted d-inline-block">
      <svg aria-label="star" role="img" class="octicon octicon-star"><path d="..."></path></svg>
      101,234
    </a>
    <a href="/rust-lang/rust/forks" data-view-component="true" class="tmp-mr-3 Link Link--muted d-inline-block">
      <svg aria-label="fork" role="img" class="octicon octicon-repo-forked"><path d="..."></path></svg>
      5,678
    </a>
    <span data-view-component="true" class="tmp-mr-3 d-inline-block">Built by someone</span>
    <span data-view-component="true" class="d-inline-block float-sm-right">
      <svg aria-hidden="true" class="octicon octicon-star"><path d="..."></path></svg>
      567 stars today
    </span>
  </div>
</article>
"#;

        let repos = parse_from_html(html, 5).unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].name, "rust-lang/rust");
        assert_eq!(repos[0].url, "https://github.com/rust-lang/rust");
        assert_eq!(
            repos[0].description.as_deref(),
            Some("A safe, concurrent, practical language.")
        );
        assert_eq!(repos[0].language.as_deref(), Some("Rust"));
        assert_eq!(repos[0].stars_total, 101234);
        assert_eq!(repos[0].stars_today, 567);
    }

    #[test]
    fn test_parse_multiple_repos() {
        let html = r#"
<article class="Box-row">
  <h2 class="h3 lh-condensed">
    <a data-view-component="true" class="Link" href="/rust-lang/rust">
      <span class="text-normal">rust-lang /</span> rust
    </a>
  </h2>
  <p class="col-9 color-fg-muted my-1 tmp-pr-4">A safe language.</p>
  <div class="f6 color-fg-muted mt-2">
    <span class="tmp-mr-3 d-inline-block">
      <span class="repo-language-color"></span>
      <span itemprop="programmingLanguage">Rust</span>
    </span>
    <a href="/rust-lang/rust/stargazers" class="tmp-mr-3 Link Link--muted d-inline-block">
      <svg class="octicon octicon-star"><path d="..."></path></svg>
      101k
    </a>
    <span class="d-inline-block float-sm-right">
      <svg class="octicon octicon-star"><path d="..."></path></svg>
      567 stars today
    </span>
  </div>
</article>
<article class="Box-row">
  <h2 class="h3 lh-condensed">
    <a data-view-component="true" class="Link" href="/denoland/deno">
      <span class="text-normal">denoland /</span> deno
    </a>
  </h2>
  <p class="col-9 color-fg-muted my-1 tmp-pr-4">A modern runtime.</p>
  <div class="f6 color-fg-muted mt-2">
    <span class="tmp-mr-3 d-inline-block">
      <span class="repo-language-color"></span>
      <span itemprop="programmingLanguage">TypeScript</span>
    </span>
    <a href="/denoland/deno/stargazers" class="tmp-mr-3 Link Link--muted d-inline-block">
      <svg class="octicon octicon-star"><path d="..."></path></svg>
      100k
    </a>
    <span class="d-inline-block float-sm-right">
      <svg class="octicon octicon-star"><path d="..."></path></svg>
      234 stars today
    </span>
  </div>
</article>
"#;

        let repos = parse_from_html(html, 5).unwrap();
        assert_eq!(repos.len(), 2);
        assert_eq!(repos[0].name, "rust-lang/rust");
        assert_eq!(repos[1].name, "denoland/deno");
        assert_eq!(repos[1].language.as_deref(), Some("TypeScript"));
        assert_eq!(repos[1].stars_today, 234);
    }

    #[test]
    fn test_parse_empty_html_returns_error() {
        let html = "<html><body>nothing here</body></html>";
        let result = parse_from_html(html, 5);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("未从 Trending 页面解析到"), "错误信息应包含提示: {}", msg);
    }

    #[test]
    fn test_parse_with_count_limit() {
        let mut html = String::new();
        html.push_str("<html><body>");
        for i in 0..10 {
            html.push_str(&format!(
                r#"
<article class="Box-row">
  <h2 class="h3 lh-condensed">
    <a class="Link" href="/owner/repo-{i}">
      <span class="text-normal">owner /</span> repo-{i}
    </a>
  </h2>
  <p class="col-9 color-fg-muted my-1 tmp-pr-4">Repo {i}</p>
  <div class="f6 color-fg-muted mt-2">
    <a href="/owner/repo-{i}/stargazers" class="tmp-mr-3 Link Link--muted d-inline-block">
      <svg class="octicon octicon-star"><path d="..."></path></svg>
      {i}k
    </a>
    <span class="d-inline-block float-sm-right">
      <svg class="octicon octicon-star"><path d="..."></path></svg>
      {i}00 stars today
    </span>
    <span class="tmp-mr-3 d-inline-block">
      <span itemprop="programmingLanguage">Rust</span>
    </span>
  </div>
</article>
"#, i = i));
        }
        html.push_str("</body></html>");

        let repos = parse_from_html(&html, 3).unwrap();
        assert_eq!(repos.len(), 3);
    }

    #[test]
    fn test_with_client_constructor() {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(1))
            .build()
            .unwrap();
        let gh = GitHubTrending::with_client(client);
        // 不发起请求，仅验证构造成功
        let _ = gh;
    }

    #[test]
    fn test_repos_to_items() {
        let repos = vec![
            Repo {
                name: "rust-lang/rust".into(),
                url: "https://github.com/rust-lang/rust".into(),
                description: Some("A safe language.".into()),
                language: Some("Rust".into()),
                stars_total: 100000,
                stars_today: 500,
            }
        ];
        let items = repos_to_items(&repos, "github_trending");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "github_trending");
        assert_eq!(items[0].id, "rust-lang/rust");
        assert_eq!(items[0].title, "rust-lang/rust");
        assert_eq!(items[0].url, "https://github.com/rust-lang/rust");
        assert_eq!(items[0].score, Some(500));
        assert!(items[0].external_content.is_none());
    }

    #[test]
    fn test_repos_to_items_zero_stars_today() {
        let repos = vec![
            Repo {
                name: "owner/repo".into(),
                url: "https://github.com/owner/repo".into(),
                description: None,
                language: None,
                stars_total: 100,
                stars_today: 0,
            }
        ];
        let items = repos_to_items(&repos, "github_trending");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "github_trending");
        assert_eq!(items[0].id, "owner/repo");
        assert_eq!(items[0].score, None);
        assert!(items[0].external_content.is_none());
    }

    #[test]
    fn test_repos_to_items_multiple() {
        let repos = vec![
            Repo {
                name: "a/b".into(),
                url: "https://github.com/a/b".into(),
                description: None,
                language: None,
                stars_total: 0,
                stars_today: 10,
            },
            Repo {
                name: "c/d".into(),
                url: "https://github.com/c/d".into(),
                description: None,
                language: None,
                stars_total: 0,
                stars_today: 20,
            },
        ];
        let items = repos_to_items(&repos, "github_trending");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, "a/b");
        assert_eq!(items[1].id, "c/d");
    }

    #[test]
    fn test_parse_repo_missing_description() {
        let html = r#"
<article class="Box-row">
  <h2 class="h3 lh-condensed">
    <a class="Link" href="/org/empty-desc">
      <span class="text-normal">org /</span> empty-desc
    </a>
  </h2>
  <div class="f6 color-fg-muted mt-2">
    <a href="/org/empty-desc/stargazers" class="tmp-mr-3 Link Link--muted d-inline-block">
      <svg class="octicon octicon-star"><path d="..."></path></svg>
      42
    </a>
    <span class="d-inline-block float-sm-right">
      <svg class="octicon octicon-star"><path d="..."></path></svg>
      5 stars today
    </span>
  </div>
</article>
"#;

        let repos = parse_from_html(html, 5).unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].name, "org/empty-desc");
        assert!(repos[0].description.is_none(), "描述缺失时应为 None");
        assert!(repos[0].language.is_none(), "语言缺失时应为 None");
    }
}
