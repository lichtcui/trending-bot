mod cache;
mod format;
mod notify;
mod output;
mod repo;
mod source;

use std::collections::HashSet;

use anyhow::{Context, Result};
use format::CardVariant;
use source::{GitHubTrending, TrendingSource};
use notify::{FeishuNotifier, Notifier};
use cache::RepoCache;

fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    // 简单的 CLI 参数解析（避免引入 clap 依赖）
    let args: Vec<String> = std::env::args().collect();
    let json_mode = args.contains(&"--json".to_string());
    let dry_run = args.contains(&"--dry-run".to_string());

    let count: usize = std::env::var("TRENDING_COUNT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);

    // 1. 获取 Trending 项目
    let source = GitHubTrending;
    let repos = source
        .fetch_trending(count)
        .context("获取 GitHub Trending 失败")?;

    // 2. 加载缓存，对比新旧
    let cache = RepoCache::new();
    let last_names: HashSet<String> = match cache.load_last_names() {
        Ok(names) => names,
        Err(e) => {
            eprintln!("⚠️ 读取缓存失败，跳过: {}", e);
            HashSet::new()
        }
    };

    let (old, new) = cache.diff(&repos, &last_names);
    let new_owned: Vec<repo::Repo> = new.clone().into_iter().cloned().collect();
    let new_names: Vec<String> = new.iter().map(|r| r.name.clone()).collect();

    let all_new = old.is_empty() && !new_owned.is_empty();
    let has_new = !new_owned.is_empty();
    let all_old = old.len() == repos.len() && !repos.is_empty();

    // 3. 飞书推送（除非 dry-run）
    if !dry_run {
        let webhook_url = std::env::var("FEISHU_WEBHOOK_URL")
            .context("请设置 FEISHU_WEBHOOK_URL 环境变量")?;

        let card = if all_new {
            format::format_card(&repos, CardVariant::Full)
        } else if has_new {
            format::format_card(&new_owned, CardVariant::Partial)
        } else {
            format::format_card(&[], CardVariant::Stale)
        };

        let notifier = FeishuNotifier::new(&webhook_url);
        notifier.send(&card)?;

        // 更新缓存（除非全部重复）
        if all_old {
            // 全部重复，不更新
        } else {
            if let Err(e) = cache.save_current_names(&repos) {
                eprintln!("⚠️ 更新缓存失败: {}", e);
            }
        }
    }

    // 4. JSON 输出（AI 调用模式）
    if json_mode {
        let ai_output = output::AiOutput::new(&repos, old.len(), &new_names, !dry_run);
        let json = serde_json::to_string_pretty(&ai_output)
            .context("序列化 JSON 输出失败")?;
        println!("{}", json);
    }

    Ok(())
}
