# Trending Bot 设计文档

> GitHub 热门项目抓取并推送飞书机器人

## 概述

一个 Rust CLI 工具，每天抓取 GitHub Trending 前 5 个热门项目，生成精简简介，通过飞书机器人 Webhook 推送 interactive 卡片消息。支持缓存去重，连续多天 trending 相同时不重复推送。

## 架构

```
┌─────────────┐     ┌──────────────┐     ┌─────────────┐     ┌──────────────┐
│   获取      │ ──→ │    解析      │ ──→ │  缓存对比   │ ──→ │    推送      │
│ HTTP GET    │     │ CSS 选择器   │     │  去重/精简  │     │ POST Webhook │
│ github.com  │     │ scraper 解析 │     │             │     │ 飞书机器人   │
└─────────────┘     └──────────────┘     └─────────────┘     └──────────────┘
                                                    ↑
                                           ~/.cache/trending-bot/last_repos.json
```

## 技术栈

| 组件 | 技术 | 理由 |
|---|---|---|
| 语言 | Rust 1.98.0 nightly | 用户运行环境
| HTTP 客户端 | reqwest 0.13 (blocking) | CLI 单次请求，无需异步 |
| HTML 解析 | scraper 0.27 | CSS 选择器解析 GitHub Trending 静态 HTML |
| JSON | serde 1.0 + serde_json 1.0 | Rust 标准 JSON 方案 |
| 错误处理 | anyhow 1.0 | CLI 项目，简洁易用 |
| 时间 | chrono 0.4 | 生成时间戳 |
| 配置 | dotenvy 0.15 | 开发时加载 .env 文件 |

## 模块设计

### main.rs — 入口与编排

```
1. 读取环境变量 FEISHU_WEBHOOK_URL
2. 调用 GitHubTrending::fetch_trending(5)
3. 读取缓存 ~/.cache/trending-bot/last_repos.json
4. 对比本次结果与缓存，分三种情况：
   a. 全部重复（5 个全在缓存中）
      → 推送一句："📌 今日热门与昨日相同，无新项目上榜"
      → 不更新缓存
   b. 部分重复（1-4 个是新项目）
      → 推送精简版卡片，只显示新增/变化的项目，每个标注 "🆕"
      → 更新缓存
   c. 全部新（0 个重复）
      → 推送完整版 5 个项目卡片
      → 更新缓存
5. 推送失败时打印错误，非零退出
```

### repo.rs — 数据模型

```rust
pub struct Repo {
    pub name: String,          // "rust-lang/rust"
    pub url: String,           // "https://github.com/rust-lang/rust"
    pub description: Option<String>,
    pub language: Option<String>,
    pub stars_total: u64,      // 总 Star 数
    pub stars_today: u64,      // 今日新增 Star
}
```

辅助函数 `parse_star_count(s: &str) -> Option<u64>`：解析 "12.3k"、"1,234"、"5.2m" 等格式。

### cache.rs — 缓存管理

```rust
pub struct RepoCache {
    cache_dir: PathBuf,  // ~/.cache/trending-bot/
}

impl RepoCache {
    // 从 ~/.cache/trending-bot/last_repos.json 读取上次的 repo 名称集合
    pub fn load_last_names(&self) -> Result<HashSet<String>>;
    // 将本次的 repo 名称集合写入缓存
    pub fn save_current_names(&self, repos: &[Repo]) -> Result<()>;
    // 判断重复程度：返回新旧项目列表
    pub fn diff(&self, repos: &[Repo]) -> Result<(Vec<&Repo>, Vec<&Repo>)>;
}
```

缓存文件格式 (`last_repos.json`)：
```json
{
  "date": "2026-07-03",
  "names": ["rust-lang/rust", "tauri-apps/tauri", "astral-sh/ruff", ...]
}
```

### source.rs — 数据源

```rust
pub trait TrendingSource {
    fn fetch_trending(&self, count: usize) -> Result<Vec<Repo>>;
}

pub struct GitHubTrending;
```

`GitHubTrending::fetch_trending` 实现：
1. `reqwest::blocking::get("https://github.com/trending?since=daily")` 带自定义 User-Agent
2. `scraper::Html::parse_document(&html)`
3. CSS 选择器提取字段：
   - 行容器: `article.Box-row`
   - 名称: `h2.h3.lh-condensed a` → href + text
   - 描述: `p.col-9.color-fg-muted`
   - 语言: `span[itemprop='programmingLanguage']`
   - 总 Star: `a.Link--muted.d-inline-block.mr-3` → text
   - 今日 Star: `span.d-inline-block.float-sm-right` → text
4. 返回前 `count` 个

### format.rs — 卡片格式化

根据是否包含新项目，生成不同卡片：

**完整版 (5 个项目全部新)：**
```json
{
  "msg_type": "interactive",
  "card": {
    "header": {
      "title": { "tag": "plain_text", "content": "🔥 GitHub 今日热门 (2026-07-03)" },
      "template": "blue"
    },
    "elements": [
      {
        "tag": "div",
        "text": {
          "tag": "lark_md",
          "content": "**#1 [owner/repo](url)**\n⭐ 12,345 stars · 🦀 Rust\n📈 +567 stars today\n\ndescription text"
        }
      },
      { "tag": "hr" },
      ... (共 5 个 repo，之间用 hr 分隔)
      {
        "tag": "note",
        "elements": [
          { "tag": "plain_text", "content": "数据来源: GitHub Trending" }
        ]
      }
    ]
  }
}
```

**精简短版 (部分重复，只显示新项目)：**
- 标题改为 `"🔥 GitHub 今日热门 · 更新"`
- 新项目前面加 `🆕` 标记
- 底部 note 显示 "仅显示新增项目，共 N 个新上榜"

**极简版 (全部重复)：**
```json
{
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
          "content": "今日热门与昨日相同，无新项目上榜。\n\n上次更新: 2026-07-03"
        }
      }
    ]
  }
}
```

### notify.rs — 推送

```rust
pub trait Notifier {
    fn send(&self, payload: &serde_json::Value) -> Result<()>;
}

pub struct FeishuNotifier {
    webhook_url: String,
}
```

`FeishuNotifier::send`: POST JSON 到飞书 Webhook URL，检查响应 `code: 0`。

## 配置

通过环境变量配置：

| 变量 | 必填 | 说明 |
|---|---|---|
| `FEISHU_WEBHOOK_URL` | 是 | 飞书机器人 Webhook URL |
| `TRENDING_COUNT` | 否 | 抓取项目数，默认 5 |
| `TRENDING_LANGUAGE` | 否 | 过滤语言（如 rust），默认所有 |
| `CACHE_DIR` | 否 | 缓存目录，默认 `~/.cache/trending-bot` |

## 错误处理

- 所有错误通过 `anyhow::Result` + `.context()` 链式传递
- 网络请求失败 → 打印 "❌ 获取 GitHub Trending 页面失败: {原因}"
- HTML 解析失败 → 打印 "❌ 解析 Trending 页面失败: {原因}"
- 推送失败 → 打印 "❌ 推送飞书消息失败: {原因}"
- 缓存读写失败（非关键路径）→ 打印 "⚠️ 缓存读写失败，跳过缓存: {原因}"，不影响主流程

## 测试策略

- `repo.rs`: 测试 `parse_star_count` 的边界情况（"12.3k"、"1,234"、"5.2m"、空字符串）
- `cache.rs`: 测试写入/读取/对比逻辑，测试空缓存场景
- `format.rs`: 测试三种卡片格式输出是否符合预期 JSON 结构
- `source.rs`: 用本地 HTML fixture 文件 mock GitHub 响应，测试解析逻辑
- `notify.rs`: 用 `httpmock` 测试 Webhook 调用

## 非功能性需求

- 二进制体积小，编译后在 crontab 中一行运行
- 依赖最小化，不引入不必要的 crate
- 单次运行不超过 10 秒
- 任何错误确保非零退出
