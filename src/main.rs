mod cache;
mod item;
mod output;
mod repo;
mod source;

use std::collections::HashSet;

use anyhow::{Context, Result};
use source::{GitHubTrending, TrendingSource};

fn main() -> Result<()> {
    // 简单的 CLI 参数解析（避免引入 clap 依赖）
    let args: Vec<String> = std::env::args().collect();
    let json_mode = args.iter().any(|a| a == "--json");
    let dry_run = args.iter().any(|a| a == "--dry-run");

    // 解析 --count N 或 -c N
    let count: usize = args.windows(2)
        .find(|w| w[0] == "--count" || w[0] == "-c")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(5);

    // 1. 获取 Trending 项目
    let source = GitHubTrending::new();
    let items = source
        .fetch(count)
        .context("获取 GitHub Trending 失败")?;
    let items: Vec<crate::item::TrendingItem> = items;

    // 2. 加载缓存，对比新旧
    let cache = cache::RepoCache::new();
    let last_names: HashSet<String> = match cache.load_last_names() {
        Ok(names) => names,
        Err(e) => {
            eprintln!("⚠️ 读取缓存失败，跳过: {}", e);
            HashSet::new()
        }
    };

    // 临时适配：只保留 repo name 用于缓存对比
    let repos: Vec<crate::repo::Repo> = items.iter().map(|i| {
        crate::repo::Repo {
            name: i.id.clone(),
            url: i.url.clone(),
            description: i.description.clone(),
            language: None,
            stars_total: 0,
            stars_today: i.score.unwrap_or(0),
        }
    }).collect();

    let (old, new) = cache.diff(&repos, &last_names);

    // 3. JSON 输出（AI 调用模式）
    if json_mode {
        let new_names: Vec<_> = new.iter().map(|r| r.name.clone()).collect();
        let ai_output = output::AiOutput::new(&repos, old.len(), &new_names);
        let json = serde_json::to_string_pretty(&ai_output)
            .context("序列化 JSON 输出失败")?;
        println!("{}", json);
    }

    // 4. 更新缓存
    if !dry_run && !repos.is_empty() {
        if let Err(e) = cache.save_current_names(&repos) {
            eprintln!("⚠️ 更新缓存失败: {}", e);
        }
    }

    Ok(())
}
