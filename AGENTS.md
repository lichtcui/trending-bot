# trending-bot — Agent Instructions

## Project

Rust CLI 热点聚合工具。每天抓取多个 Trending 数据源，输出结构化 JSON，支持 LLM 内容总结。

## Commands

```bash
# 构建
cargo build --release

# 测试
cargo test                          # 单元测试
cargo test -- --ignored             # 含网络请求的集成测试

# 运行（默认 3 源：github, hn, lobsters）
cargo run --release -- --json                        # JSON 输出
cargo run --release -- --json --count 10             # Top 10
cargo run --release -- --json --summarize            # 带 LLM 总结
cargo run --release -- --json --source rust_weekly,bytebytego,ai_weekly

# 周一自动追加 3 个 RSS 源（无需 --source），非周一 = 3 源
```

## Important Rules

- **周一自动追加**：`rust_weekly`、`bytebytego`、`ai_weekly` 三个 Newsletter 源仅在周一自动加载。非周一可通过 `--source` 显式指定。
- **不跨期去重**：同一链接在不同周中视为不同条目。
- **`--summarize`** 对无总结的旧项目也会重新调用 DeepSeek 总结。
  需要设置环境变量 `DEEPSEEK_API_KEY` 或添加至 macOS Keychain。
- **测试必须全部通过**才能提交。

## Project Structure

```
src/
├── main.rs       # 入口：CLI 解析、编排、周一检测
├── source.rs     # TrendingSource trait + GitHubTrending
├── rss.rs        # RSS 源解析（Rust Weekly / ByteByteGo / AI Weekly）
├── hn.rs         # HackerNews
├── lobsters.rs   # Lobsters
├── item.rs       # TrendingItem 统一数据模型
├── fetcher.rs    # 外部链接内容抓取
├── cache.rs      # 多源统一缓存
├── output.rs     # JSON 输出
├── summary.rs    # DeepSeek LLM 总结
└── repo.rs       # 旧数据模型
```

## Key Design

- 所有源实现 `TrendingSource` trait
- 缓存按 `(source, id)` 键去重
- RSS 源 `id` 前缀：`twir_` / `bbg_` / `aiw_`
- 单个源失败不影响其他源
