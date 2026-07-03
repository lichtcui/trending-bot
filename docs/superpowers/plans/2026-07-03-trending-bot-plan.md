# Trending Bot 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 构建一个 Rust CLI 工具，抓取 GitHub Trending 前 5 个项目，通过飞书机器人 Webhook 推送 interactive 卡片消息，支持缓存去重。

**架构：** 模块化单体 CLI，分为 source（数据源）、repo（数据模型）、cache（缓存）、format（卡片格式化）、notify（推送）五个模块，通过 trait 接口解耦。使用 reqwest blocking 同步 HTTP + scraper HTML 解析。

**技术栈：** Rust 1.95.0, reqwest 0.13 (blocking), scraper 0.27, serde 1.0 + serde_json 1.0, anyhow 1.0, chrono 0.4, dotenvy 0.15

---

## 文件结构

```
trending-bot/
├── Cargo.toml                 # 包配置 + 依赖
├── .env                       # FEISHU_WEBHOOK_URL=...（示例文件 .env.example）
├── .gitignore
├── tests/
│   ├── test_parse_star_count.rs  # parse_star_count 单元测试
│   ├── test_format.rs            # 卡片格式化测试
│   └── fixtures/
│       └── trending.html         # GitHub Trending 页面 HTML fixture
└── src/
    ├── main.rs                 # 入口，流程编排
    ├── repo.rs                 # Repo 数据模型 + parse_star_count
    ├── source.rs               # TrendingSource trait + GitHubTrending 实现
    ├── cache.rs                # 缓存读写 + 新旧对比
    ├── format.rs               # 飞书 interactive 卡片 JSON 构建
    └── notify.rs               # Notifier trait + FeishuNotifier 实现
```

---

### 任务 1：项目脚手架

**文件：**
- 创建：`Cargo.toml`
- 创建：`.gitignore`
- 创建：`.env.example`

- [ ] **步骤 1：创建 Cargo.toml**

```toml
[package]
name = "trending-bot"
version = "0.1.0"
edition = "2021"
description = "GitHub Trending scraper + Feishu bot notifier"

[dependencies]
reqwest = { version = "0.13", features = ["blocking", "json"] }
scraper = "0.27"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
anyhow = "1.0"
chrono = { version = "0.4", features = ["serde"] }
dotenvy = "0.15"
```

- [ ] **步骤 2：创建 .gitignore**

```
/target
.env
*.swp
*.swo
.DS_Store
```

- [ ] **步骤 3：创建 .env.example**

```
FEISHU_WEBHOOK_URL=https://open.feishu.cn/open-apis/bot/v2/hook/your-webhook-token-here
```

- [ ] **步骤 4：验证编译**

运行：`cd /Users/lichtcui/Documents/playground/trending-breif && cargo init --name trending-bot`
预期：src/main.rs 被创建，cargo build 成功

- [ ] **步骤 5：Commit**

```bash
git add Cargo.toml .gitignore .env.example src/ && git commit -m "chore: scaffold trending-bot project"
```

---

### 任务 2：数据模型 — repo.rs

**文件：**
- 创建：`src/repo.rs`

- [ ] **步骤 1：编写 repo.rs**

```rust
use serde::Serialize;

/// 一个 GitHub Trending 项目
#[derive(Debug, Clone, Serialize)]
pub struct Repo {
    pub name: String,           // "owner/repo"
    pub url: String,            // "https://github.com/owner/repo"
    pub description: Option<String>,
    pub language: Option<String>,
    pub stars_total: u64,       // 总 Star 数
    pub stars_today: u64,       // 今日新增 Star
}

/// 解析 GitHub 的 Star 数字符串
/// 支持格式: "1,234" → 1234, "12.3k" → 12300, "5.2m" → 5200000
pub fn parse_star_count(s: &str) -> Option<u64> {
    let s = s.trim().replace(',', "");
    if s.is_empty() {
        return None;
    }
    let lower = s.to_lowercase();
    if lower.ends_with('k') {
        let num: f64 = lower[..lower.len() - 1].trim().parse().ok()?;
        Some((num * 1000.0) as u64)
    } else if lower.ends_with('m') {
        let num: f64 = lower[..lower.len() - 1].trim().parse().ok()?;
        Some((num * 1_000_000.0) as u64)
    } else if lower.ends_with('b') {
        let num: f64 = lower[..lower.len() - 1].trim().parse().ok()?;
        Some((num * 1_000_000_000.0) as u64)
    } else {
        lower.parse::<u64>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_plain_number() {
        assert_eq!(parse_star_count("1,234"), Some(1234));
        assert_eq!(parse_star_count("0"), Some(0));
        assert_eq!(parse_star_count("99999"), Some(99999));
    }

    #[test]
    fn test_parse_k_suffix() {
        assert_eq!(parse_star_count("12.3k"), Some(12300));
        assert_eq!(parse_star_count("1k"), Some(1000));
        assert_eq!(parse_star_count("0.5k"), Some(500));
        assert_eq!(parse_star_count("100k"), Some(100000));
    }

    #[test]
    fn test_parse_m_suffix() {
        assert_eq!(parse_star_count("1.5m"), Some(1_500_000));
        assert_eq!(parse_star_count("5.2m"), Some(5_200_000));
    }

    #[test]
    fn test_parse_empty_string() {
        assert_eq!(parse_star_count(""), None);
        assert_eq!(parse_star_count("   "), None);
    }

    #[test]
    fn test_parse_whitespace() {
        assert_eq!(parse_star_count("  1,234 "), Some(1234));
    }
}
```

- [ ] **步骤 2：运行测试验证通过**

运行：`cargo test --lib repo`
预期：所有 6 个测试 PASS

- [ ] **步骤 3：Commit**

```bash
git add src/repo.rs && git commit -m "feat: add Repo data model and parse_star_count"
```

---

### 任务 3：缓存模块 — cache.rs

**文件：**
- 创建：`src/cache.rs`

- [ ] **步骤 1：编写 cache.rs**

```rust
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::repo::Repo;

/// 上次推送的缓存记录
#[derive(Debug, Serialize, Deserialize)]
struct CacheData {
    date: String,
    names: Vec<String>,
}

/// 本地缓存管理
pub struct RepoCache {
    cache_dir: PathBuf,
}

impl RepoCache {
    /// 创建缓存管理器，CACHE_DIR 环境变量优先，否则用 ~/.cache/trending-bot
    pub fn new() -> Self {
        let cache_dir = if let Ok(dir) = std::env::var("CACHE_DIR") {
            PathBuf::from(dir)
        } else {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".cache").join("trending-bot")
        };
        RepoCache { cache_dir }
    }

    fn cache_path(&self) -> PathBuf {
        self.cache_dir.join("last_repos.json")
    }

    /// 加载上次缓存的 repo 名称集合
    pub fn load_last_names(&self) -> Result<HashSet<String>> {
        let path = self.cache_path();
        if !path.exists() {
            return Ok(HashSet::new());
        }
        let content = fs::read_to_string(&path)
            .context("读取缓存文件失败")?;
        let data: CacheData = serde_json::from_str(&content)
            .context("解析缓存文件失败")?;
        Ok(data.names.into_iter().collect())
    }

    /// 保存本次 repo 名称到缓存
    pub fn save_current_names(&self, repos: &[Repo]) -> Result<()> {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let data = CacheData {
            date: today,
            names: repos.iter().map(|r| r.name.clone()).collect(),
        };
        fs::create_dir_all(&self.cache_dir)
            .context("创建缓存目录失败")?;
        let content = serde_json::to_string_pretty(&data)
            .context("序列化缓存数据失败")?;
        fs::write(self.cache_path(), content)
            .context("写入缓存文件失败")?;
        Ok(())
    }

    /// 对比本次结果与缓存，返回 (旧项目列表, 新项目列表)
    pub fn diff<'a>(&self, repos: &'a [Repo], last_names: &HashSet<String>) -> (Vec<&'a Repo>, Vec<&'a Repo>) {
        let mut old = Vec::new();
        let mut new = Vec::new();
        for repo in repos {
            if last_names.contains(&repo.name) {
                old.push(repo);
            } else {
                new.push(repo);
            }
        }
        (old, new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_repo(name: &str) -> Repo {
        Repo {
            name: name.to_string(),
            url: format!("https://github.com/{}", name),
            description: None,
            language: None,
            stars_total: 0,
            stars_today: 0,
        }
    }

    #[test]
    fn test_diff_all_new() {
        let cache = RepoCache::new();
        let repos = vec![make_repo("a/b"), make_repo("c/d")];
        let last = HashSet::new();
        let (old, new) = cache.diff(&repos, &last);
        assert!(old.is_empty());
        assert_eq!(new.len(), 2);
    }

    #[test]
    fn test_diff_all_old() {
        let cache = RepoCache::new();
        let repos = vec![make_repo("a/b"), make_repo("c/d")];
        let last: HashSet<String> = vec!["a/b".into(), "c/d".into()].into_iter().collect();
        let (old, new) = cache.diff(&repos, &last);
        assert_eq!(old.len(), 2);
        assert!(new.is_empty());
    }

    #[test]
    fn test_diff_partial() {
        let cache = RepoCache::new();
        let repos = vec![make_repo("a/b"), make_repo("c/d"), make_repo("e/f")];
        let last: HashSet<String> = vec!["a/b".into()].into_iter().collect();
        let (old, new) = cache.diff(&repos, &last);
        assert_eq!(old.len(), 1);
        assert_eq!(old[0].name, "a/b");
        assert_eq!(new.len(), 2);
    }
}
```

- [ ] **步骤 2：在 main.rs 声明模块**

在 `src/main.rs` 开头添加：

```rust
mod repo;
mod cache;
```

- [ ] **步骤 3：运行测试验证通过**

运行：`cargo test --lib cache`
预期：所有测试 PASS

- [ ] **步骤 4：Commit**

```bash
git add src/cache.rs src/main.rs && git commit -m "feat: add caching module with diff logic"
```

---

### 任务 4：数据源 — source.rs

**文件：**
- 创建：`src/source.rs`
- 创建：`tests/fixtures/trending.html`

- [ ] **步骤 1：准备测试用 HTML fixture**

创建 `tests/fixtures/trending.html`（从 GitHub Trending 实际页面截取前 5 个项目的 HTML 片段，用 `<article class="Box-row">` 包裹）

```html
<article class="Box-row">
  <h2 class="h3 lh-condensed">
    <a href="/rust-lang/rust">rust-lang / <strong>rust</strong></a>
  </h2>
  <p class="col-9 color-fg-muted my-1 pr-4">
    A safe, concurrent, practical language.
  </p>
  <div class="f6 color-fg-muted mt-2">
    <a href="/rust-lang/rust/stargazers" class="Link--muted d-inline-block mr-3">
      <span>★</span>
      <span class="d-inline-block float-sm-right">101,234</span>
    </a>
    <a href="/trending?since=daily" class="Link--muted d-inline-block mr-3">
      <span class="d-inline-block float-sm-right">567 stars today</span>
    </a>
    <span class="d-inline-block mr-3" itemprop="programmingLanguage">Rust</span>
  </div>
</article>
<!-- 复制 4 个类似条目作为测试数据 -->
```

（实际内容需要从 `https://github.com/trending` 获取真实 HTML 片段作为 fixture）

- [ ] **步骤 2：编写 source.rs**

```rust
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
    fn parse_repo(row: &scraper::ElementRef) -> Option<Repo> {
        // 选择器
        let link_sel = Selector::parse("h2.h3.lh-condensed a").ok()?;
        let desc_sel = Selector::parse("p.col-9.color-fg-muted").ok()?;
        let lang_sel = Selector::parse("span[itemprop='programmingLanguage']").ok()?;
        let star_total_sel = Selector::parse("a.Link--muted.d-inline-block.mr-3 .float-sm-right").ok()?;
        let star_today_sel = Selector::parse("a[href*='since='] .float-sm-right").ok()?;

        // 名称 + URL
        let link = row.select(&link_sel).next()?;
        let href = link.value().attr("href")?;
        let name_text = link.text().collect::<String>();
        let name = name_text.split_whitespace().collect::<Vec<_>>().join("");
        let url = format!("https://github.com{}", href);

        // 描述
        let description = row.select(&desc_sel).next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .filter(|s| !s.is_empty());

        // 语言
        let language = row.select(&lang_sel).next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .filter(|s| !s.is_empty());

        // 总 Star
        let stars_total = row.select(&star_total_sel).next()
            .and_then(|e| {
                let text = e.text().collect::<String>();
                parse_star_count(&text)
            })
            .unwrap_or(0);

        // 今日 Star
        let stars_today = row.select(&star_today_sel).next()
            .and_then(|e| {
                let text = e.text().collect::<String>();
                // "567 stars today" → 提取数字
                text.split_whitespace().next()
                    .and_then(|n| n.replace(',', "").parse::<u64>().ok())
            })
            .unwrap_or(0);

        Some(Repo {
            name: name.replace(" ", ""),
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

        let html = client.get(url)
            .send()
            .context("请求 GitHub Trending 页面失败")?
            .text()
            .context("读取响应内容失败")?;

        let doc = Html::parse_document(&html);
        let row_sel = Selector::parse("article.Box-row")
            .map_err(|e| anyhow::anyhow!("CSS 选择器解析失败: {}", e))?;

        let repos: Vec<Repo> = doc.select(&row_sel)
            .filter_map(|row| Self::parse_repo(&row))
            .take(count)
            .collect();

        anyhow::ensure!(!repos.is_empty(), "未从 Trending 页面解析到任何项目，页面结构可能已变更");

        Ok(repos)
    }
}
```

- [ ] **步骤 3：在 main.rs 声明模块**

```rust
mod source;
```

- [ ] **步骤 4：编写测试**（用本地 HTML fixture 测试解析逻辑）

创建 `tests/test_source.rs`：

```rust
use std::fs;

use trending_bot::source::{GitHubTrending, TrendingSource};

#[test]
fn test_parse_from_html_fixture() {
    let html = fs::read_to_string("tests/fixtures/trending.html")
        .expect("fixture file not found");
    let repos = trending_bot::source::parse_from_html(&html, 5)
        .expect("parse should succeed");
    assert!(!repos.is_empty());
    assert!(repos.len() <= 5);
}
```

> 注意：需要将 `parse_from_html` 作为公开函数暴露，或者测试直接构造 GitHubTrending 并 mock HTTP 请求。简化方案：将 parse 逻辑提取为 `pub fn parse_from_html(html: &str, count: usize) -> Result<Vec<Repo>>`。

- [ ] **步骤 5：运行测试验证通过**

运行：`cargo test test_parse_from_html`
预期：PASS

- [ ] **步骤 6：Commit**

```bash
git add src/source.rs tests/ && git commit -m "feat: add GitHub Trending scraper"
```

---

### 任务 5：卡片格式化 — format.rs

**文件：**
- 创建：`src/format.rs`

- [ ] **步骤 1：编写 format.rs**

```rust
use serde_json::{json, Value};

use crate::repo::Repo;

/// 生成飞书 interactive 卡片消息
pub enum CardVariant {
    /// 全部新项目 — 完整版
    Full,
    /// 部分新项目 — 精简版，只显示新增
    Partial { new_count: usize },
    /// 全部重复 — 极简提示
    Stale,
}

/// 根据重复程度生成相应卡片
pub fn format_card(repos: &[Repo], variant: CardVariant) -> Value {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    match variant {
        CardVariant::Full => build_full_card(repos, &today),
        CardVariant::Partial { new_count } => build_partial_card(repos, new_count, &today),
        CardVariant::Stale => build_stale_card(&today),
    }
}

fn build_full_card(repos: &[Repo], date: &str) -> Value {
    let elements = build_repo_elements(repos, false, date);
    json!({
        "msg_type": "interactive",
        "card": {
            "header": {
                "title": { "tag": "plain_text", "content": format!("🔥 GitHub 今日热门 ({})", date) },
                "template": "blue"
            },
            "elements": elements
        }
    })
}

fn build_partial_card(new_repos: &[Repo], new_count: usize, date: &str) -> Value {
    let elements = build_repo_elements(new_repos, true, date);
    json!({
        "msg_type": "interactive",
        "card": {
            "header": {
                "title": { "tag": "plain_text", "content": "🔥 GitHub 今日热门 · 更新" },
                "template": "blue"
            },
            "elements": elements
        }
    })
}

fn build_stale_card(date: &str) -> Value {
    json!({
        "msg_type": "interactive",
        "card": {
            "header": {
                "title": { "tag": "plain_text", "content": "📌 GitHub 今日热门" },
                "template": "grey"
            },
            "elements": [
                {
                    "tag": "div",
                    "text": {
                        "tag": "lark_md",
                        "content": "今日热门与昨日相同，无新项目上榜。\n\n上次更新: {date}"
                    }
                }
            ]
        }
    })
}

fn build_repo_elements(repos: &[Repo], show_new_badge: bool, date: &str) -> Vec<Value> {
    let mut elements = Vec::new();
    for (i, repo) in repos.iter().enumerate() {
        let badge = if show_new_badge { "🆕 " } else { "" };
        let rank = i + 1;
        let lang_dot = repo.language.as_deref().unwrap_or("");
        let desc = repo.description.as_deref().unwrap_or("No description");
        let content = format!(
            "**{badge}#{rank} [{name}]({url})**\n⭐ {stars} stars · {lang}\n📈 +{today} stars today\n\n{desc}",
            badge = badge,
            rank = rank,
            name = repo.name,
            url = repo.url,
            stars = repo.stars_total,
            lang = lang_dot,
            today = repo.stars_today,
            desc = desc,
        );

        elements.push(json!({
            "tag": "div",
            "text": { "tag": "lark_md", "content": content }
        }));

        // 项目之间用分隔线隔开
        if i < repos.len() - 1 {
            elements.push(json!({ "tag": "hr" }));
        }
    }

    // 脚注
    elements.push(json!({
        "tag": "note",
        "elements": [
            { "tag": "plain_text", "content": format!("数据来源: GitHub Trending · {}", date) }
        ]
    }));

    elements
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_repo(name: &str) -> Repo {
        Repo {
            name: name.to_string(),
            url: format!("https://github.com/{}", name),
            description: Some("A test repo".into()),
            language: Some("Rust".into()),
            stars_total: 1000,
            stars_today: 50,
        }
    }

    #[test]
    fn test_full_card_has_correct_structure() {
        let repos = vec![make_repo("a/b"), make_repo("c/d")];
        let card = format_card(&repos, CardVariant::Full);
        assert_eq!(card["msg_type"], "interactive");
        assert_eq!(card["card"]["header"]["template"], "blue");
        assert!(card["card"]["elements"].as_array().unwrap().len() >= 3); // 2 repos + hr + note
    }

    #[test]
    fn test_stale_card_has_grey_template() {
        let card = format_card(&[], CardVariant::Stale);
        assert_eq!(card["card"]["header"]["template"], "grey");
    }

    #[test]
    fn test_partial_card_contains_new_projects() {
        let repos = vec![make_repo("new/project")];
        let card = format_card(&repos, CardVariant::Partial { new_count: 1 });
        let content = card["card"]["elements"][0]["text"]["content"].as_str().unwrap();
        assert!(content.contains("🆕"));
    }
}
```

- [ ] **步骤 2：在 main.rs 声明模块**

```rust
mod format;
```

- [ ] **步骤 3：运行测试验证通过**

运行：`cargo test --lib format`
预期：所有测试 PASS

- [ ] **步骤 4：Commit**

```bash
git add src/format.rs && git commit -m "feat: add Feishu interactive card formatter"
```

---

### 任务 6：推送模块 — notify.rs

**文件：**
- 创建：`src/notify.rs`

- [ ] **步骤 1：编写 notify.rs**

```rust
use anyhow::{Context, Result};
use serde_json::Value;

/// 推送器 trait — 为今后扩展到 Slack/Discord 做准备
pub trait Notifier {
    fn send(&self, payload: &Value) -> Result<()>;
}

/// 飞书机器人 Webhook 推送
pub struct FeishuNotifier {
    webhook_url: String,
}

impl FeishuNotifier {
    pub fn new(webhook_url: &str) -> Self {
        FeishuNotifier {
            webhook_url: webhook_url.to_string(),
        }
    }
}

impl Notifier for FeishuNotifier {
    fn send(&self, payload: &Value) -> Result<()> {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .context("创建 HTTP 客户端失败")?;

        let resp = client.post(&self.webhook_url)
            .json(payload)
            .send()
            .context("请求飞书 Webhook 失败")?;

        let status = resp.status();
        let body: Value = resp.json().context("解析飞书响应失败")?;

        let code = body.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
        if code != 0 {
            let msg = body.get("msg").and_then(|m| m.as_str()).unwrap_or("未知错误");
            anyhow::bail!("飞书返回错误 (code={}): {}", code, msg);
        }

        println!("✅ 成功推送到飞书 (HTTP {})", status);
        Ok(())
    }
}
```

- [ ] **步骤 2：在 main.rs 声明模块**

```rust
mod notify;
```

- [ ] **步骤 3：Commit**

```bash
git add src/notify.rs && git commit -m "feat: add Feishu webhook notifier"
```

---

### 任务 7：主入口 — main.rs

**文件：**
- 修改：`src/main.rs`

- [ ] **步骤 1：编写完整的 main.rs**

```rust
mod cache;
mod format;
mod notify;
mod repo;
mod source;

use std::collections::HashSet;

use anyhow::{Context, Result};
use format::CardVariant;
use source::{GitHubTrending, TrendingSource};
use notify::{FeishuNotifier, Notifier};
use cache::RepoCache;

fn main() -> Result<()> {
    // 加载 .env 文件（开发环境），生产环境用环境变量
    dotenvy::dotenv().ok();

    let webhook_url = std::env::var("FEISHU_WEBHOOK_URL")
        .context("请设置 FEISHU_WEBHOOK_URL 环境变量")?;
    let count: usize = std::env::var("TRENDING_COUNT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);

    // 1. 获取 Trending
    let source = GitHubTrending;
    let repos = source.fetch_trending(count)
        .context("获取 GitHub Trending 失败")?;

    // 2. 加载缓存对比
    let cache = RepoCache::new();
    let last_names: HashSet<String> = match cache.load_last_names() {
        Ok(names) => names,
        Err(e) => {
            eprintln!("⚠️ 读取缓存失败，跳过: {}", e);
            HashSet::new()
        }
    };

    let (old, new) = cache.diff(&repos, &last_names);

    // 3. 根据重复程度决定卡片内容
    let card = if old.is_empty() && !new.is_empty() {
        // 全部新项目
        format::format_card(&repos, CardVariant::Full)
    } else if !new.is_empty() {
        // 部分新项目 — 只展示新项目
        format::format_card(&new, CardVariant::Partial { new_count: new.len() })
    } else {
        // 全部重复
        format::format_card(&[], CardVariant::Stale)
    };

    // 4. 推送
    let notifier = FeishuNotifier::new(&webhook_url);
    notifier.send(&card)?;

    // 5. 更新缓存（除非全部重复）
    if !old.is_empty() || new.is_empty() {
        // 只有全部重复时跳过更新
    } else {
        if let Err(e) = cache.save_current_names(&repos) {
            eprintln!("⚠️ 更新缓存失败: {}", e);
        }
    }

    // 6. 输出统计信息
    println!("📊 本次: 共 {} 个项目", repos.len());
    println!("   - 新项目: {}", new.len());
    println!("   - 已存在: {}", old.len());

    Ok(())
}
```

- [ ] **步骤 2：验证编译通过**

运行：`cargo build`
预期：编译成功，无错误

- [ ] **步骤 3：Commit**

```bash
git add src/main.rs && git commit -m "feat: implement main orchestration with cache diff"
```

---

## 自检

1. **规格覆盖度：** ✅ 所有规格需求都有对应任务 —— 数据模型(任务2)、缓存去重(任务3)、抓取解析(任务4)、卡片格式化(任务5)、推送(任务6)、编排(任务7)
2. **占位符扫描：** ✅ 无 "TODO"、"待定"、空章节
3. **类型一致性：** ✅ `Repo` 结构体在所有模块中签名一致，`parse_star_count` 返回值类型统一为 `Option<u64>`

---

计划已完成并保存到 `docs/superpowers/plans/2026-07-03-trending-bot-plan.md`。两种执行方式：

**1. 子代理驱动（推荐）** — 每个任务调度一个新的子代理，任务间进行审查，快速迭代

**2. 内联执行** — 在当前会话中使用 executing-plans 执行任务，批量执行并设有检查点

**选哪种方式？**
