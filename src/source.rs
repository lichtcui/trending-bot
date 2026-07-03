use anyhow::{Context, Result};
use scraper::{Html, Selector};

use crate::repo::{parse_star_count, Repo};

/// 数据源 trait — 为今后扩展其他来源（如 GitHub API）做准备
pub trait TrendingSource {
    fn fetch_trending(&self, count: usize) -> Result<Vec<Repo>>;
}

/// GitHub Trending 页面抓取器
pub struct GitHubTrending;

impl GitHubTrending {
    /// 从单个 HTML 行元素中解析 Repo
    fn parse_repo(row: &scraper::ElementRef) -> Option<Repo> {
        // 选择器（一次解析，复用）
        let link_sel = Selector::parse("h2.h3.lh-condensed a").ok()?;
        let desc_sel = Selector::parse("p.col-9.color-fg-muted").ok()?;
        let lang_sel = Selector::parse("span[itemprop='programmingLanguage']").ok()?;
        let star_total_sel = Selector::parse("a[href$='/stargazers']").ok()?;
        let star_today_sel = Selector::parse("span.d-inline-block.float-sm-right").ok()?;

        // --- 名称 + URL ---
        let link = row.select(&link_sel).next()?;
        let href = link.value().attr("href")?;
        let name = href
            .trim_start_matches('/')
            .split('/')
            .collect::<Vec<_>>()
            .join("/");
        // 取 text 节点内容，忽略内部 strong 等子元素干扰
        let url = format!("https://github.com{}", href);

        // --- 描述 ---
        let description = row
            .select(&desc_sel)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .filter(|s| !s.is_empty());

        // --- 编程语言 ---
        let language = row
            .select(&lang_sel)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .filter(|s| !s.is_empty());

        // --- 总 Star 数 ---
        // 从 stargazers 链接的文本中提取（跳过 SVG 图标文本）
        let stars_total = row
            .select(&star_total_sel)
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
            .select(&star_today_sel)
            .next()
            .and_then(|e| {
                let text = e.text().collect::<String>();
                text.split_whitespace()
                    .find(|s| s.chars().next().map_or(false, |c| c.is_ascii_digit()))
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

impl TrendingSource for GitHubTrending {
    fn fetch_trending(&self, count: usize) -> Result<Vec<Repo>> {
        let url = "https://github.com/trending?since=daily";
        let client = reqwest::blocking::Client::builder()
            .user_agent("Mozilla/5.0 (compatible; trending-bot/0.1.0)")
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .context("创建 HTTP 客户端失败")?;

        let html = client
            .get(url)
            .send()
            .context("请求 GitHub Trending 页面失败")?
            .text()
            .context("读取响应内容失败")?;

        parse_from_html(&html, count)
    }
}

/// 从 HTML 文本中解析 Trending 项目列表（暴露为 pub 方便测试）
pub fn parse_from_html(html: &str, count: usize) -> Result<Vec<Repo>> {
    let doc = Html::parse_document(html);
    let row_sel =
        Selector::parse("article.Box-row").map_err(|e| anyhow::anyhow!("CSS 选择器解析失败: {}", e))?;

    let repos: Vec<Repo> = doc
        .select(&row_sel)
        .filter_map(|row| GitHubTrending::parse_repo(&row))
        .take(count)
        .collect();

    anyhow::ensure!(
        !repos.is_empty(),
        "未从 Trending 页面解析到任何项目，页面结构可能已变更"
    );

    Ok(repos)
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
