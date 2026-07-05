# Weekly Newsletter 数据源 — 周一专属 RSS 源设计

## 概述

为 trending-bot 增加三个 Newsletter 数据源（Rust Weekly、ByteByteGo、AI Weekly），
仅在**周一**运行时自动追加到现有 GitHub Trending / HN / Lobsters 之上。
三个源均通过 RSS 获取，Rust Weekly 需额外展开 HTML 内链为独立条目。

## 周一 vs 非周一的数据流

```
非周一（周二~周日）：
  GitHub Trending → HN → Lobsters → 缓存对比 → JSON 输出

周一：
  GitHub Trending → HN → Lobsters       → 缓存对比 → JSON 输出
  Rust Weekly (RSS → HTML 展开链接)
  ByteByteGo (RSS)
  AI Weekly (RSS)
```

周一运行时共 6 个源，全部走统一 `TrendingSource` trait，输出一致 `TrendingItem`。

## 源确认

| 名称 | 实际网站 | RSS URL | 发布频率 | RSS 结构 |
|------|---------|---------|---------|---------|
| Rust Weekly | `this-week-in-rust.org` | `/rss.xml` | 每周三 | 1 item/期，内嵌 HTML 全文含 50+ 链接 |
| ByteByteGo | `blog.bytebytego.com` (Substack) | `/feed` | ~2-3 次/周 | 多 items，每篇独立文章 |
| AI Weekly | `aiweekly.co` | `/feed` | 3 次/周 (一/三/五) | 多 items，每期独立文章 |

## 数据模型

### TrendingItem（不变）

复用现有 `TrendingItem` 结构体，无新增字段：

```rust
pub struct TrendingItem {
    pub source: String,    // "rust_weekly" | "bytebytego" | "ai_weekly"
    pub id: String,        // 各源唯一标识
    pub title: String,
    pub url: String,
    pub score: Option<u64>,
    pub external_content: Option<ExternalContent>,
    pub summary: Option<String>,
}
```

### id 生成规则

| 源 | id 格式 | 示例 |
|----|--------|------|
| Rust Weekly | `twir_{issue_num}_{url_hash}` | `twir_658_a1b2c3d4e5f6a7b8` |
| ByteByteGo | `bbg_{url_slug_hash}` | `bbg_a1b2c3d4e5f6a7b8` |
| AI Weekly | `aiw_{url_slug_hash}` | `aiw_a1b2c3d4e5f6a7b8` |

`url_hash` 复用现有的 `RepoCache::compute_url_hash()` 方法（16 位 hex）。

## 模块设计

### 新增文件

```
src/
├── main.rs    # 增加 is_monday() 检测 + 周一追加 3 个源
└── rss.rs     # 新增：RSS 通用解析 + 三个 Newsletter 源的 fetch 函数
```

### rss.rs 结构

```rust
// 三个公开函数，各自实现 TrendingSource::fetch 逻辑
pub fn fetch_rust_weekly(client: &Client, count: usize) -> Result<Vec<TrendingItem>>;
pub fn fetch_bytebytego(client: &Client, count: usize) -> Result<Vec<TrendingItem>>;
pub fn fetch_ai_weekly(client: &Client, count: usize) -> Result<Vec<TrendingItem>>;
```

每个函数内部：
1. 请求 RSS URL
2. 用 `rss` crate 解析 XML
3. 按 `count` 截取
4. 映射为 `Vec<TrendingItem>`

### ByteByteGo / AI Weekly（简单映射）

```rust
// RSS item 直接映射为 TrendingItem
for item in channel.items().iter().take(count) {
    items.push(TrendingItem {
        source: "bytebytego".into(),     // 或 "ai_weekly"
        id: format!("bbg_{}", compute_hash(item.link())),
        title: item.title().unwrap_or("").to_string(),
        url: item.link().unwrap_or("").to_string(),
        score: None,
        external_content: None,
        summary: None,
    });
}
```

### Rust Weekly（需要 HTML 展开）

```rust
// 1. 取 RSS 最新一期
let latest = channel.items().first()?;

// 2. 提取期号
let issue_num = extract_issue_number(latest.title()); // "This Week in Rust 658" → 658

// 3. 从 description HTML 中用 scraper 提取所有 <a href>
let html = latest.description();
let doc = Html::parse_document(html);
let sel = Selector::parse("a[href]")?;

// 4. 每个链接 → TrendingItem
for link in doc.select(&sel).take(count) {
    let href = link.value().attr("href")?;
    let text: String = link.text().collect();
    items.push(TrendingItem {
        source: "rust_weekly".into(),
        id: format!("twir_{}_{}", issue_num, compute_hash(href)),
        title: text.trim().to_string(),
        url: resolve_url(href),     // 处理相对路径
        score: None,
        ...
    });
}
```

## 周一检测

```rust
fn is_monday() -> bool {
    chrono::Local::now().format("%u").to_string() == "1"
    // %u = ISO weekday, 1=Monday .. 7=Sunday
}
```

## CLI 变化

**无新增 CLI 参数**。周一自动追加，非周一自动跳过。

但 `--source` 仍可手动指定，用于调试：

```bash
# 仅测试 Rust Weekly 解析
cargo run --release -- --source rust_weekly

# 周一调试：只看 Newsletter 源
cargo run --release -- --source rust_weekly,bytebytego,ai_weekly

# 非周一强制跑周一源（通过 --source 显式指定）
cargo run --release -- --source rust_weekly --json
```

## `--count` 的作用范围

- GitHub/HN/Lobsters：每个源取 `--count` 条（和现在一样）
- Rust Weekly：取最新一期，HTML 展开链接后按 `--count` 截取
- ByteByteGo：RSS feed 取前 `--count` 条
- AI Weekly：RSS feed 取前 `--count` 条

## 错误处理

- 单个 RSS 源超时/解析失败 → 跳过该源，不影响其他 5 个源
- Rust Weekly HTML 展开无 `<a>` 链接 → 提示"本期无外部链接"
- 所有非致命错误 via `eprintln!` 输出到 stderr

## 缓存策略

### 缓存文件（不变）

缓存格式为 `data_v2.json`，新增三个 source key：

```json
{
  "version": "2",
  "sources": {
    "github_trending": { ... },
    "hacker_news": { ... },
    "lobsters": { ... },
    "rust_weekly": { "date": "2026-07-06", "items": [...] },
    "bytebytego": { "date": "2026-07-06", "items": [...] },
    "ai_weekly": { "date": "2026-07-06", "items": [...] }
  }
}
```

### 重复检测

按 `(source, id)` 键对比缓存。同一链接在 Rust Weekly 不同期中出现时的 id 不同（因 issue_num 变了），
**不跨期去重**——同一链接很少连续出现在多期周刊中。

## 输出 JSON 变化

周一示例：

```json
{
  "tool": "trending-bot",
  "version": "0.1.0",
  "fetched_at": "2026-07-06T09:00:00+08:00",
  "count": 49,
  "items": [
    { "source": "github_trending", "id": "owner/repo", ... },
    { "source": "rust_weekly", "id": "twir_658_a1b2...", "title": "Announcing Rust 1.96.1", ... },
    { "source": "bytebytego", "id": "bbg_c3d4...", "title": "AI Routing", ... },
    { "source": "ai_weekly", "id": "aiw_e5f6...", "title": "Anthropic Files IPO", ... },
    ...
  ],
  "cache": {
    "by_source": {
      "github_trending": { "new": 3, "old": 2 },
      "hacker_news": { "new": 4, "old": 1 },
      "lobsters": { "new": 2, "old": 3 },
      "rust_weekly": { "new": 24, "old": 0 },
      "bytebytego": { "new": 5, "old": 0 },
      "ai_weekly": { "new": 5, "old": 0 }
    }
  }
}
```

## 控制台输出

周一运行示意：

```
✓ github_trending 获取到 5 条
✓ hacker_news 获取到 5 条
✓ lobsters 获取到 5 条
📬 周一加餐: 追加 3 个 Newsletter 源...
✓ rust_weekly 展开为 24 条
✓ bytebytego 获取到 5 条
✓ ai_weekly 获取到 5 条
```

## 新增依赖

```toml
rss = "2"    # RSS 2.0 解析
```

## 测试计划

- `test_fetch_rust_weekly_rss_parse` — 用 RSS XML 样本测试 Rust Weekly RSS 解析
- `test_fetch_rust_weekly_html_expand` — 用 HTML 片段测试链接展开
- `test_fetch_bytebytego_mapping` — 测试 ByteByteGo RSS → TrendingItem 映射
- `test_fetch_ai_weekly_mapping` — 测试 AI Weekly RSS → TrendingItem 映射
- `test_is_monday` — 测试周一检测函数
- `test_rust_weekly_no_links` — HTML 无链接时的空结果处理
- `test_monday_source_inclusion` — 集成测试周一追加逻辑

## 不包含的范围

- **不引入调度层**（无 cron 配置/定时器），保持 CLI 工具简单
- **不跨期去重**（同一链接在不同期中出现视为不同条目）
- **不为 Newsletter 源抓取外部内容**（跳过 `fetcher.rs` 的内容抓取，因为 Rust Weekly 已经包含了摘要）
