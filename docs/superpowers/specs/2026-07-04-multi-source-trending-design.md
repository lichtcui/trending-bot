# Multi-Source Trending — 多源热点聚合设计

## 概述

为 trending-bot 增加 Hacker News 和 Lobsters 两个数据源，统一输出结构化 JSON，
支持抓取帖子外部链接内容（GitHub README / Web 文章正文），缓存升级为多源统一管理。

## 数据流

```
GitHub Trending (HTML 解析)
     ↓
Hacker News (Firebase API)
     ↓
Lobsters (REST API)
     ↓
统一数据模型 TrendingItem[]
     ↓
缓存对比 → 区分新旧项目
     ↓
内容抓取器 (仅新项目 + 未缓存内容)
     ├─ github.com/* → 尝试 GitHub API 获取 README
     └─ 其他 URL     → HTML 下载 + 正文提取
     ↓
结构化 JSON 输出 (stdout)
     ↓
更新本地缓存
```

## 数据模型

```rust
pub struct TrendingItem {
    pub source: String,                // "github_trending" | "hacker_news" | "lobsters"
    pub id: String,                    // 各源唯一标识
    pub title: String,
    pub url: String,
    pub description: Option<String>,
    pub score: Option<u64>,
    pub author: Option<String>,
    pub comments_url: Option<String>,
    pub external_content: Option<ExternalContent>,
}

pub struct ExternalContent {
    pub url: String,
    pub content_type: ContentType,
    pub text: String,
    pub word_count: usize,
}

pub enum ContentType {
    GitHubReadme,
    WebArticle,
}
```

## Source 层

### TrendingSource Trait（泛化）

```rust
pub trait TrendingSource {
    fn source_name(&self) -> &'static str;
    fn fetch(&self, count: usize) -> Result<Vec<TrendingItem>>;
}
```

### GitHubTrending（已有，适配映射）

- 保留现有 HTML 解析逻辑不变
- `parse_repo` 结果映射为 `TrendingItem`
- `id` = `{owner}/{repo}`

### HackerNews（新增）

- 官方 Firebase API：`GET /v0/topstories.json` → 取前 N 个 story ID
- `GET /v0/item/{id}.json` → 逐个获取详情
- 只保留 `type=story` 且有 `url` 的条目（排除 Ask HN / Show HN）
- `id` = `story_{id}`
- 使用已有 `reqwest::blocking::Client`，顺序请求

### Lobsters（新增）

- REST API：`GET https://lobste.rs/hottest.json` → 返回数组
- 直接解析 JSON，无需逐个请求
- `id` = `story_{short_id}`
- 字段映射：`title`、`url`、`score`、`submitter_user` → `author`、`short_id` → `id`

## ContentFetcher 内容抓取器

独立组件，负责抓取帖子链接背后的内容。

### 逻辑

1. 判断 URL 类型
2. GitHub URL → 通过 GitHub API 获取 README（未认证，60次/小时）
3. 其他 URL → HTTP 请求 HTML → 正文提取
4. 正文提取优先级：`<article>` → `<main>` / `role="main"` → `<body>` 去标签
5. 最大正文长度限制 5000 字符

### 去重

- 批量抓取时，相同 URL 仅抓取一次
- 缓存中已有内容 hash 的 URL 跳过

## 缓存升级

### 文件格式

```json
{
  "version": "2",
  "sources": {
    "github_trending": {
      "date": "2026-07-04",
      "items": [
        { "id": "rust-lang/rust", "url": "https://github.com/rust-lang/rust", "content_hash": "abc123" }
      ]
    },
    "hacker_news": { ... },
    "lobsters": { ... }
  }
}
```

### 缓存对比

- 按 `(source, id)` 判断新旧
- 按 `url` 的 hash 判断链接内容是否已缓存
- 自动迁移旧版缓存（仅有 `names` 字段的旧格式）

### 缓存输出

```json
"cache": {
  "status": "partial_update",
  "new_items": 3,
  "old_items": 7,
  "fetched_content": 2,
  "cached_content": 1,
  "failed_content": 1,
  "by_source": {
    "github_trending": { "new": 1, "old": 4 },
    "hacker_news": { "new": 1, "old": 2 },
    "lobsters": { "new": 1, "old": 1 }
  }
}
```

## CLI 参数

| 参数 | 描述 |
|------|------|
| `--source / -s` | 指定源，可重复（github, hn, lobsters），默认全部 |
| `--content / -C` | 是否抓取外部链接内容，默认开启 |
| `--count / -c` | 每个源取前 N 条，默认 5 |
| `--json` | JSON 输出（默认开启） |
| `--dry-run` | 不更新缓存 |

无内容抓取参数时（仅 `--source hn`），HN 帖子仅输出元数据。

## 文件结构

```
src/
├── main.rs           # 入口：CLI 解析、编排流程
├── source.rs         # TrendingSource trait + GitHubTrending（已有，适配）
├── hn.rs             # HackerNews 实现（新增）
├── lobsters.rs       # Lobsters 实现（新增）
├── item.rs           # TrendingItem 统一数据模型（新增）
├── fetcher.rs        # ContentFetcher 内容抓取（新增）
├── cache.rs          # UnifiedCache 多源缓存（重构）
├── output.rs         # AiOutput JSON 输出（适配）
└── repo.rs           # Repo 旧模型 + parse_star_count（保留，内部使用）
```

## 错误处理

- 单个源失败不影响其他源
- 单个内容抓取失败不影响其他帖子
- 失败信息记录到输出 JSON 中
- 缓存加载失败时回退到空缓存，不中断运行
