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
        let star_total_sel = Selector::parse("a.Link--muted.d-inline-block.mr-3 .float-sm-right").ok()?;
        let star_today_sel = Selector::parse("a[href*='since='] .float-sm-right").ok()?;

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
        // 取第一个匹配的 .float-sm-right 数字文本
        let stars_total = row
            .select(&star_total_sel)
            .next()
            .and_then(|e| {
                let text = e.text().collect::<String>();
                parse_star_count(&text)
            })
            .unwrap_or(0);

        // --- 今日 Star 数 ---
        // 文本形如 "567 stars today" → 提取第一个数字
        let stars_today = row
            .select(&star_today_sel)
            .next()
            .and_then(|e| {
                let text = e.text().collect::<String>();
                text.split_whitespace()
                    .next()
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
    <a href="/rust-lang/rust">rust-lang / <strong>rust</strong></a>
  </h2>
  <p class="col-9 color-fg-muted my-1 pr-4">
    A safe, concurrent, practical language.
  </p>
  <div class="f6 color-fg-muted mt-2">
    <a href="/rust-lang/rust" class="Link--muted d-inline-block mr-3">
      <span>★</span>
      <span class="d-inline-block float-sm-right">101,234</span>
    </a>
    <a href="/trending?since=daily" class="Link--muted d-inline-block mr-3">
      <span class="d-inline-block float-sm-right">567 stars today</span>
    </a>
    <span class="d-inline-block mr-3" itemprop="programmingLanguage">Rust</span>
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
    <a href="/rust-lang/rust">rust-lang / <strong>rust</strong></a>
  </h2>
  <p class="col-9 color-fg-muted my-1 pr-4">A safe language.</p>
  <div class="f6 color-fg-muted mt-2">
    <a href="/rust-lang/rust" class="Link--muted d-inline-block mr-3">
      <span>★</span><span class="d-inline-block float-sm-right">101k</span>
    </a>
    <a href="/trending?since=daily" class="Link--muted d-inline-block mr-3">
      <span class="d-inline-block float-sm-right">567 stars today</span>
    </a>
    <span class="d-inline-block mr-3" itemprop="programmingLanguage">Rust</span>
  </div>
</article>
<article class="Box-row">
  <h2 class="h3 lh-condensed">
    <a href="/denoland/deno">denoland / <strong>deno</strong></a>
  </h2>
  <p class="col-9 color-fg-muted my-1 pr-4">A modern runtime.</p>
  <div class="f6 color-fg-muted mt-2">
    <a href="/denoland/deno" class="Link--muted d-inline-block mr-3">
      <span>★</span><span class="d-inline-block float-sm-right">100k</span>
    </a>
    <a href="/trending?since=daily" class="Link--muted d-inline-block mr-3">
      <span class="d-inline-block float-sm-right">234 stars today</span>
    </a>
    <span class="d-inline-block mr-3" itemprop="programmingLanguage">TypeScript</span>
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
        assert!(msg.contains("未解析到"), "错误信息应包含提示: {}", msg);
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
    <a href="/owner/repo-{i}">owner / <strong>repo-{i}</strong></a>
  </h2>
  <p class="col-9 color-fg-muted my-1 pr-4">Repo {i}</p>
  <div class="f6 color-fg-muted mt-2">
    <a href="/owner/repo-{i}" class="Link--muted d-inline-block mr-3">
      <span>★</span><span class="d-inline-block float-sm-right">{i}k</span>
    </a>
    <a href="/trending?since=daily" class="Link--muted d-inline-block mr-3">
      <span class="d-inline-block float-sm-right">{i}00 stars today</span>
    </a>
    <span class="d-inline-block mr-3" itemprop="programmingLanguage">Rust</span>
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
    <a href="/org/empty-desc">org / <strong>empty-desc</strong></a>
  </h2>
  <div class="f6 color-fg-muted mt-2">
    <a href="/org/empty-desc" class="Link--muted d-inline-block mr-3">
      <span>★</span><span class="d-inline-block float-sm-right">42</span>
    </a>
    <a href="/trending?since=daily" class="Link--muted d-inline-block mr-3">
      <span class="d-inline-block float-sm-right">5 stars today</span>
    </a>
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
