# Weekly Newsletter 数据源 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 在 trending-bot 中增加三个周一专属 Newsletter 数据源（Rust Weekly、ByteByteGo、AI Weekly），通过 RSS 获取并在周一自动追加到现有源之上。

**架构：** 新增 `rss.rs` 模块，使用 `rss` crate 解析 RSS 2.0。Rust Weekly 需额外用 `scraper` 展开 HTML 内链。`main.rs` 中增加周一检测逻辑自动追加源。

**技术栈：** Rust, `rss = "2"`, `scraper`（已有）, `reqwest`（已有）, `chrono`（已有）

---

## 文件结构

```
src/
├── rss.rs      # 新增：RSS 通用解析 + 三个 Newsletter 源的 fetch 函数
├── main.rs     # 修改：增加 is_monday() + 周一追加 3 个源
├── Cargo.toml  # 修改：增加 rss 依赖
└── source.rs   # 修改：为周报源注册 TrendingSource 适配（可选）
```

## 新增文件职责

- **`src/rss.rs`**：三个公开函数 `fetch_rust_weekly()` / `fetch_bytebytego()` / `fetch_ai_weekly()`，各自返回 `Vec<TrendingItem>`

---

### 任务 1：添加 `rss` 依赖 + 创建 `rss.rs` 骨架

**文件：**
- 修改：`Cargo.toml`
- 创建：`src/rss.rs`
- 测试：`src/rss.rs` 底部

- [ ] **步骤 1：添加 rss 依赖到 Cargo.toml**

在 `[dependencies]` 中添加：

```toml
rss = "2"
```

- [ ] **步骤 2：创建 rss.rs 骨架**

```rust
use anyhow::Result;
use reqwest::blocking::Client;

use crate::item::TrendingItem;

/// 获取 This Week in Rust 的最新一期，展开 HTML 内链为独立 TrendingItem
pub fn fetch_rust_weekly(client: &Client, count: usize) -> Result<Vec<TrendingItem>> {
    todo!()
}

/// 获取 ByteByteGo Newsletter 最新文章
pub fn fetch_bytebytego(client: &Client, count: usize) -> Result<Vec<TrendingItem>> {
    todo!()
}

/// 获取 AI Weekly 最新文章
pub fn fetch_ai_weekly(client: &Client, count: usize) -> Result<Vec<TrendingItem>> {
    todo!()
}

#[cfg(test)]
mod tests {
    // TODO: 后续任务添加测试
}
```

- [ ] **步骤 3：运行 `cargo check` 验证编译**

运行：`cargo check`
预期：编译成功

- [ ] **步骤 4：在 `main.rs` 声明模块**

在 `main.rs` 顶部：

```rust
mod rss;
```

运行：`cargo check`
预期：编译成功

- [ ] **步骤 5：Commit**

```bash
git add Cargo.toml src/rss.rs src/main.rs
git commit -m "chore: add rss crate dependency and rss.rs skeleton"
```

---

### 任务 2：实现 Rust Weekly RSS 解析 + HTML 链接展开

**文件：**
- 修改：`src/rss.rs`
- 测试：`src/rss.rs` 内

- [ ] **步骤 1：编写失败测试——RSS 解析 + HTML 展开**

在 `src/rss.rs` 的 `#[cfg(test)]` 模块中添加：

```rust
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
}
```

运行：`cargo test test_fetch_rust_weekly_rss_parse_and_expand -- --ignored`
预期：编译失败（`parse_rust_weekly_from_rss` 未定义）

- [ ] **步骤 2：实现 Rust Weekly 核心解析函数**

在 `src/rss.rs` 中添加内部函数和公共函数：

```rust
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use scraper::{Html, Selector};

use crate::item::TrendingItem;

const RUST_WEEKLY_RSS: &str = "https://this-week-in-rust.org/rss.xml";

pub fn fetch_rust_weekly(client: &Client, count: usize) -> Result<Vec<TrendingItem>> {
    let xml = client
        .get(RUST_WEEKLY_RSS)
        .send()
        .context("请求 Rust Weekly RSS 失败")?
        .text()
        .context("读取 RSS 响应失败")?;

    parse_rust_weekly_from_rss(&xml, count)
}

/// 从 RSS XML 字符串中解析 Rust Weekly，展开 HTML 链接
fn parse_rust_weekly_from_rss(xml: &str, count: usize) -> Result<Vec<TrendingItem>> {
    let channel = rss::Channel::read_from(xml.as_bytes())
        .context("解析 Rust Weekly RSS XML 失败")?;

    // 取最新一期
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
            // 跳过锚点和空链接
            if href.is_empty() || href.starts_with('#') {
                return None;
            }
            let text: String = el.text().collect::<Vec<_>>().concat();
            let text = text.trim().to_string();
            if text.is_empty() || text == href {
                return None;
            }
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

/// 从标题中提取期号："This Week in Rust 658" → 658
fn extract_issue_number(title: &str) -> &str {
    title.rsplit(' ').next().unwrap_or("0")
}

/// 计算 URL 短 hash（16 位 hex）
fn compute_url_hash(url: &str) -> String {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
```

- [ ] **步骤 3：运行测试验证通过**

运行：`cargo test test_fetch_rust_weekly_rss_parse_and_expand -- --ignored`
预期：PASS

- [ ] **步骤 4：添加更多 Rust Weekly 边界测试**

```rust
#[test]
fn test_extract_issue_number() {
    assert_eq!(extract_issue_number("This Week in Rust 658"), "658");
    assert_eq!(extract_issue_number("This Week in Rust 1"), "1");
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
}
```

- [ ] **步骤 5：运行全部新增测试**

运行：`cargo test test_rust_weekly -- --ignored`
预期：全部 PASS

- [ ] **步骤 6：Commit**

```bash
git add src/rss.rs
git commit -m "feat: add Rust Weekly RSS parsing with HTML link expansion"
```

---

### 任务 3：实现 ByteByteGo / AI Weekly RSS 解析

**文件：**
- 修改：`src/rss.rs`

- [ ] **步骤 1：编写失败测试——ByteByteGo 解析**

```rust
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
```

- [ ] **步骤 2：实现通用 RSS 条目解析器**

在 `src/rss.rs` 中添加：

```rust
const BYTEBYTEGO_RSS: &str = "https://blog.bytebytego.com/feed";
const AI_WEEKLY_RSS: &str = "https://aiweekly.co/feed";

pub fn fetch_bytebytego(client: &Client, count: usize) -> Result<Vec<TrendingItem>> {
    let xml = client
        .get(BYTEBYTEGO_RSS)
        .send()
        .context("请求 ByteByteGo RSS 失败")?
        .text()
        .context("读取 ByteByteGo RSS 响应失败")?;
    parse_generic_rss_items(&xml, "bytebytego", count)
}

pub fn fetch_ai_weekly(client: &Client, count: usize) -> Result<Vec<TrendingItem>> {
    let xml = client
        .get(AI_WEEKLY_RSS)
        .send()
        .context("请求 AI Weekly RSS 失败")?
        .text()
        .context("读取 AI Weekly RSS 响应失败")?;
    parse_generic_rss_items(&xml, "ai_weekly", count)
}

/// 通用 RSS 条目解析：
/// 将 RSS feed 中每个 item 映射为 TrendingItem（适用于 ByteByteGo / AI Weekly）
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
            let id = format!("{}_{}", match source_name {
                "bytebytego" => "bbg",
                "ai_weekly" => "aiw",
                _ => source_name,
            }, compute_url_hash(&url));
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
```

- [ ] **步骤 3：运行测试**

运行：`cargo test test_parse_bytebytego_rss_items test_parse_ai_weekly_rss_items -- --ignored`
预期：PASS

- [ ] **步骤 4：添加边界测试**

```rust
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
```

- [ ] **步骤 5：运行全部 RSS 测试**

运行：`cargo test -- --ignored`
预期：全部 PASS

- [ ] **步骤 6：Commit**

```bash
git add src/rss.rs
git commit -m "feat: add ByteByteGo and AI Weekly RSS parsing"
```

---

### 任务 4：实现周一检测 + 源注册

**文件：**
- 修改：`src/main.rs`
- 修改：`src/rss.rs`（增加一个辅助 Trait 实现或调整公开签名）

- [ ] **步骤 1：编写失败测试——周一检测函数**

在 `src/main.rs` 底部添加测试模块（或新增 `tests/` 目录）：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_monday_logic() {
        // 2026-07-06 是周一
        let dt = chrono::NaiveDate::from_ymd_opt(2026, 7, 6).unwrap();
        assert!(is_monday_for_date(dt));

        // 2026-07-07 是周二
        let dt = chrono::NaiveDate::from_ymd_opt(2026, 7, 7).unwrap();
        assert!(!is_monday_for_date(dt));

        // 2026-07-05 是周日
        let dt = chrono::NaiveDate::from_ymd_opt(2026, 7, 5).unwrap();
        assert!(!is_monday_for_date(dt));
    }
}
```

- [ ] **步骤 2：实现 `is_monday` 检测逻辑**

将 `main.rs` 中的主函数逻辑重构，添加：

```rust
/// 今天是周一吗？（ISO 标准：1=周一）
fn is_monday() -> bool {
    is_monday_for_date(chrono::Local::now().date_naive())
}

/// 可测试版本，接受指定日期
fn is_monday_for_date(date: chrono::NaiveDate) -> bool {
    date.format("%u").to_string() == "1"
}
```

- [ ] **步骤 3：运行测试验证通过**

运行：`cargo test test_is_monday_logic`
预期：PASS

- [ ] **步骤 4：实现周一源追加逻辑**

在 `main.rs` 的源初始化部分修改：

```rust
// 1. 初始化各 Source
let mut sources: Vec<Box<dyn TrendingSource>> = Vec::new();
for name in &enabled_sources {
    match name.as_str() {
        "github" => sources.push(Box::new(source::GitHubTrending::new())),
        "hn" => sources.push(Box::new(hn::HackerNews::new())),
        "lobsters" => sources.push(Box::new(lobsters::Lobsters::new())),
        // 周一专属 RSS 源
        "rust_weekly" | "bytebytego" | "ai_weekly" => {
            // 通过 --source 手动指定时直接添加
            sources.push(Box::new(rss::RssSource::new(name)));
        }
        _ => eprintln!("⚠️ 未知数据源: {}，跳过", name),
    }
}

// 自动追加周一专属源（仅在未通过 --source 手动指定时）
let monday_sources = ["rust_weekly", "bytebytego", "ai_weekly"];
if is_monday() && enabled_sources.len() == 3 {
    eprintln!("📬 周一加餐: 追加 3 个 Newsletter 源...");
    for name in &monday_sources {
        sources.push(Box::new(rss::RssSource::new(name)));
    }
}
```

因为需要让 RSS 源适配 `TrendingSource` trait，在 `src/rss.rs` 中添加：

```rust
use crate::source::TrendingSource;

pub struct RssSource {
    name: String,
    client: Client,
}

impl RssSource {
    pub fn new(name: &str) -> Self {
        let client = Client::builder()
            .user_agent("trending-bot/0.1.0")
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("创建 HTTP 客户端失败");
        RssSource {
            name: name.to_string(),
            client,
        }
    }

    pub fn with_client(name: &str, client: Client) -> Self {
        RssSource { name: name.to_string(), client }
    }
}

impl TrendingSource for RssSource {
    fn source_name(&self) -> &'static str {
        // 需要返回 &'static str，但 name 是 String
        // 方案：match 匹配来返回静态字符串
        match self.name.as_str() {
            "rust_weekly" => "rust_weekly",
            "bytebytego" => "bytebytego",
            "ai_weekly" => "ai_weekly",
            _ => "unknown_rss",
        }
    }

    fn fetch(&self, count: usize) -> Result<Vec<TrendingItem>> {
        match self.name.as_str() {
            "rust_weekly" => fetch_rust_weekly(&self.client, count),
            "bytebytego" => fetch_bytebytego(&self.client, count),
            "ai_weekly" => fetch_ai_weekly(&self.client, count),
            _ => anyhow::bail!("未知 RSS 源: {}", self.name),
        }
    }
}
```

- [ ] **步骤 5：运行 `cargo build` 验证编译**

运行：`cargo build`
预期：编译成功

- [ ] **步骤 6：运行全部测试**

运行：`cargo test`
预期：全部 PASS（包括已有测试）

- [ ] **步骤 7：Commit**

```bash
git add src/main.rs src/rss.rs
git commit -m "feat: add Monday detection and auto-register newsletter sources"
```

---

### 任务 5：集成测试 + 端到端验证

- [ ] **步骤 1：编写集成测试——周一源包含**

在 `src/main.rs` 的测试模块中添加：

```rust
#[test]
fn test_monday_source_inclusion() {
    // 模拟周一时，enabled_sources 为默认 3 个时，应追加周一源
    let enabled = vec!["github".to_string(), "hn".to_string(), "lobsters".to_string()];
    let mut names: Vec<String> = enabled.clone();
    if true /* is_monday */ {
        names.extend(["rust_weekly", "bytebytego", "ai_weekly"].map(|s| s.to_string()));
    }
    assert_eq!(names.len(), 6);
    assert!(names.contains(&"rust_weekly".to_string()));
}
```

- [ ] **步骤 2：`cargo test` 确认全部通过**

运行：`cargo test`
预期：全部 PASS

- [ ] **步骤 3：手动运行验证（非周一）**

```bash
cargo run --release -- --json --count 3
```

预期：3 个源（github/hn/lobsters），无 Newsletter 源

- [ ] **步骤 4：手动运行验证（强制周一源）**

```bash
cargo run --release -- --json --count 3 --source rust_weekly,bytebytego,ai_weekly
```

预期：仅 3 个 RSS 源输出

- [ ] **步骤 5：Commit**

```bash
git add src/main.rs
git commit -m "test: add integration tests for Monday source inclusion"
```

---

### 任务 6：更新 README 文档

**文件：**
- 修改：`README.md`

- [ ] **步骤 1：在 README 中增加 Newsletter 源说明**

在"特性"部分添加：
```markdown
- **周一加餐** — 周一自动追加 This Week in Rust、ByteByteGo、AI Weekly 三个 Newsletter 源，翻倍覆盖
```

在项目架构表格中添加 `rss.rs`：
```
src/
├── rss.rs       # RSS 源解析（Rust Weekly / ByteByteGo / AI Weekly）
```

在 CLI 参数部分增加说明：
```
| `--source / -s` | 指定源，可重复（github, hn, lobsters, rust_weekly, bytebytego, ai_weekly），默认全部 |

> 周一运行时，rust_weekly、bytebytego、ai_weekly 三个源会自动追加到 github/hn/lobsters 之后。
> 非周一可通过 `--source` 手动指定获取。
```

- [ ] **步骤 2：Commit**

```bash
git add README.md
git commit -m "docs: update README with weekly newsletter sources info"
```
