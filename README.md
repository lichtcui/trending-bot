# 🔥 trending-bot

**GitHub Trending 爬虫 + 飞书机器人推送**

每日自动抓取 GitHub Trending 热门项目，通过飞书应用推送到你的个人对话。支持增量更新——只推送新上榜的项目，重复内容自动跳过。

## 特性

- **自动抓取** — 爬取 GitHub Trending 每日热门项目（支持 `--count` 自定义数量）
- **飞书推送** — 通过飞书 App ID + App Secret 认证，以交互式卡片消息发送到指定用户
- **增量更新** — 本地缓存历史记录，只推送新上榜单的项目，避免重复打扰
- **多卡片样式** — 全量更新（完整 5 条）、部分更新（仅新项目 + 🆕 标识）、无变化（极简提示）
- **macOS Keychain** — 安全存储凭证，同时支持环境变量降级（兼容 Linux/Docker）
- **JSON 输出** — `--json` 模式输出结构化数据，方便与其他工具/Agent 集成
- **健壮解析** — CSS 选择器解析 HTML，支持 `k`/`m` 星数后缀，完善的错误处理

## 快速开始

### 前置条件

- Rust 1.60+
- macOS（Keychain 功能）或 Linux/Docker（使用环境变量）
- 飞书企业自建应用（用于推送消息）

### 安装

```bash
# 克隆仓库
git clone https://github.com/lichtcui/trending-bot.git
cd trending-bot

# 构建
cargo build --release

# 二进制文件在 target/release/trending-bot
```

### 配置凭证

在飞书开放平台创建企业自建应用后，将以下凭证存入 Keychain：

```bash
# App ID
security add-generic-password -a "$USER" -s FEISHU_APP_ID -w "<your_app_id>" -U

# App Secret
security add-generic-password -a "$USER" -s FEISHU_APP_SECRET -w "<your_app_secret>" -U

# 用户 Open ID（在飞书开放平台 → 应用 → 测试用户 中获取）
security add-generic-password -a "$USER" -s FEISHU_OPEN_ID -w "<your_open_id>" -U
```

> **Linux / Docker 环境**：无需 Keychain，直接设置环境变量即可：
> ```bash
> export FEISHU_APP_ID="your_app_id"
> export FEISHU_APP_SECRET="your_app_secret"
> export FEISHU_OPEN_ID="your_open_id"
> ```
> 环境变量优先级高于 Keychain。

### 运行

```bash
# 默认获取 Top 5 项目并推送到飞书
cargo run --release

# 获取 Top 10 项目
cargo run --release -- --count 10

# 预览模式（不推送，仅打印 JSON 到控制台）
cargo run --release -- --dry-run --json

# JSON 输出（适合被其他程序调用）
cargo run --release -- --json
```

## CLI 参数

| 参数 | 描述 |
|------|------|
| `--count N` / `-c N` | 获取前 N 个项目（默认 5） |
| `--json` | 输出结构化 JSON（包含缓存对比结果） |
| `--dry-run` | 预览模式，不推送飞书消息 |

## 项目架构

```
src/
├── main.rs       # 入口：CLI 解析、编排获取→缓存→推送流程
├── source.rs     # GitHub Trending 页面抓取与 HTML 解析
├── repo.rs       # Repo 数据结构与 Star 数解析
├── cache.rs      # 本地缓存管理器（~/Library/Caches/trending-bot/）
├── format.rs     # 飞书交互式卡片消息格式化
├── notify.rs     # 飞书推送器（App ID + App Secret 认证）
├── keychain.rs   # macOS Keychain 凭证读取（支持环境变量降级）
└── output.rs     # AI 可消费的结构化 JSON 输出
```

### 数据流

```
GitHub Trending 页面
        ↓ (HTTP 请求 + HTML 解析)
    Repo 列表
        ↓ (与本地缓存对比)
   ├─ 旧项目（已在缓存中）→ 跳过
   └─ 新项目（不在缓存中）→ 格式化卡片
        ↓
    飞书交互式卡片消息
        ↓
    更新本地缓存
```

## 缓存策略

缓存文件位于 `~/Library/Caches/trending-bot/last_repos.json`，格式如下：

```json
{
  "date": "2026-07-04",
  "names": ["owner/repo-a", "owner/repo-b"]
}
```

- **首次运行**：缓存不存在，视为全量更新（`all_new`）
- **重复数据**：只有部分重复或全部重复时，分别触发 `partial` 或 `stale` 卡片
- **缓存损坏**：自动提示手动删除

## 飞书卡片样式

当缓存为空或全部为新项目时：

![全部新项目](https://img.shields.io/badge/样式-蓝色完整版-2196F3)

标题：`🔥 GitHub 今日热门 (2026-07-04)` — 蓝色背景，展示全部项目详情

当有部分项目更新时：

![部分更新](https://img.shields.io/badge/样式-蓝色更新版-2196F3)

标题：`🔥 GitHub 今日热门 · 更新` — 仅展示新项目，每条带 🆕 标识

当无变化时：

![无变化](https://img.shields.io/badge/样式-灰色极简版-9E9E9E)

标题：`📌 GitHub 今日热门` — 灰色背景，提示无新项目

## JSON 输出（`--json`）

适合 AI Agent 或其他自动化工具消费的结构化输出：

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
  },
  "feishu_pushed": true
}
```

### `cache.status` 取值

| 值 | 含义 |
|----|------|
| `all_new` | 缓存为空或全部是新项目 |
| `partial_update` | 部分项目是新的 |
| `no_change` | 所有项目与上次一致 |

## 测试

```bash
# 运行所有测试
cargo test

# 运行特定模块测试
cargo test test_diff_all_new

# 运行不包含网络请求的测试（排除 ignore 标记的测试）
cargo test -- --ignored
```

## License

MIT
