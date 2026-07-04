# 🔥 trending-bot

**GitHub Trending 爬虫 · AI 友好的结构化 JSON 输出**

每日自动抓取 GitHub Trending 热门项目，输出结构化 JSON 数据，适合 AI Agent、CI 流水线及其他自动化工具消费。内置增量缓存——只输出新上榜的项目，重复内容自动跳过。

## 特性

- **自动抓取** — 爬取 GitHub Trending 每日热门项目，CSS 选择器解析，支持 `k`/`m` 星数后缀
- **增量缓存** — 本地缓存历史记录，只输出新上榜单的项目，避免重复消费
- **AI 友好输出** — 结构化 JSON 包含缓存对比结果，AI Agent 可直接集成
- **简洁依赖** — 仅依赖 `reqwest`、`scraper`、`serde`、`chrono`，轻量无冗余
- **跨平台** — Linux / macOS / Docker 均可运行

## 快速开始

### 前置条件

- Rust 1.60+

### 安装

```bash
# 克隆仓库
git clone https://github.com/lichtcui/trending-bot.git
cd trending-bot

# 构建
cargo build --release

# 二进制文件在 target/release/trending-bot
```

### 运行

```bash
# 默认获取 Top 5 项目，输出到控制台
cargo run --release

# 获取 Top 10 项目
cargo run --release -- --count 10

# JSON 输出（适合 AI Agent 或其他工具消费）
cargo run --release -- --json

# 预览模式（不更新本地缓存）
cargo run --release -- --dry-run

# 组合使用
cargo run --release -- --json --count 10 --dry-run
```

## CLI 参数

| 参数 | 描述 |
|------|------|
| `--count N` / `-c N` | 获取前 N 个项目（默认 5） |
| `--json` | 输出结构化 JSON（包含缓存对比结果） |
| `--dry-run` | 预览模式，不更新本地缓存 |

## 项目架构

```
src/
├── main.rs       # 入口：CLI 参数解析、编排获取→缓存→输出流程
├── source.rs     # GitHub Trending 页面请求与 HTML 解析
├── repo.rs       # Repo 数据结构与 Star 数解析
├── cache.rs      # 本地缓存管理器（增量对比 / 持久化）
└── output.rs     # AI 可消费的结构化 JSON 输出
```

### 数据流

```
GitHub Trending 页面
        ↓ (HTTP 请求 + HTML 解析)
    Repo 列表
        ↓ (与本地缓存对比)
   ├─ 旧项目（已在缓存中）→ 跳过标记
   └─ 新项目（不在缓存中）→ 新项目标记
        ↓
    结构化 JSON 输出（stdout）
        ↓
    更新本地缓存
```

## JSON 输出格式

```json
{
  "tool": "trending-bot",
  "version": "0.1.0",
  "fetched_at": "2026-07-04T12:00:00+08:00",
  "count": 5,
  "repos": [
    {
      "name": "rust-lang/rust",
      "url": "https://github.com/rust-lang/rust",
      "description": "Empowering everyone to build reliable...",
      "language": "Rust",
      "stars_total": 101234,
      "stars_today": 567
    }
  ],
  "cache": {
    "status": "partial_update",
    "new_count": 2,
    "old_count": 3,
    "new_repos": ["owner/new-a", "owner/new-b"],
    "is_duplicate": false
  }
}
```

### `cache.status` 取值

| 值 | 含义 |
|----|------|
| `all_new` | 缓存为空或全部是新项目 |
| `partial_update` | 部分项目是新的 |
| `no_change` | 所有项目与上次一致 |

## AI Agent 集成示例

```bash
# 作为 MCP 工具或自定义 Agent 工具调用
cargo run --release -- --json --count 10
```

输出可以直接注入 LLM 上下文，Agent 可根据 `cache` 字段判断是否有新项目、哪些是新项目，避免重复处理。

## 缓存策略

缓存文件位于 `~/Library/Caches/trending-bot/last_repos.json`，格式如下：

```json
{
  "date": "2026-07-04",
  "names": ["owner/repo-a", "owner/repo-b"]
}
```

- **首次运行**：缓存不存在，视为全量更新（`all_new`）
- **重复数据**：只有部分重复或全部重复时，分别触发 `partial_update` 或 `no_change`
- **缓存损坏**：自动提示手动删除

## 测试

```bash
# 运行所有测试
cargo test

# 运行不包含网络请求的测试（跳过 ignore 标记的集成测试）
cargo test -- --ignored
```

## License

MIT
