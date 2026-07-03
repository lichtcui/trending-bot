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
    // 加载 .env 文件（开发环境），生产环境通过环境变量设置
    dotenvy::dotenv().ok();

    let webhook_url = std::env::var("FEISHU_WEBHOOK_URL")
        .context("请设置 FEISHU_WEBHOOK_URL 环境变量")?;
    let count: usize = std::env::var("TRENDING_COUNT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);

    // 1. 获取 Trending 项目
    println!("📡 正在获取 GitHub Trending...");
    let source = GitHubTrending;
    let repos = source
        .fetch_trending(count)
        .context("获取 GitHub Trending 失败")?;
    println!("✅ 获取到 {} 个项目", repos.len());

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

    // 3. 根据重复程度选择卡片类型
    let card = if old.is_empty() && !new.is_empty() {
        // 全部是新项目
        println!("📊 全部 {} 个项目均为新上榜", repos.len());
        format::format_card(&repos, CardVariant::Full)
    } else if !new.is_empty() {
        // 部分新项目 — 只展示新项目
        println!("📊 {} 个新项目，{} 个项目与上次相同", new.len(), old.len());
        format::format_card(&new, CardVariant::Partial)
    } else {
        // 全部重复
        println!("📊 今日热门与昨日相同，无新项目");
        format::format_card(&[], CardVariant::Stale)
    };

    // 4. 推送到飞书
    let notifier = FeishuNotifier::new(&webhook_url);
    notifier.send(&card)?;

    // 5. 更新缓存（除非全部重复）
    match old.len() {
        n if n == repos.len() && !repos.is_empty() => {
            // 全部重复，不更新缓存
            println!("📌 项目无变化，跳过缓存更新");
        }
        _ => {
            if let Err(e) = cache.save_current_names(&repos) {
                eprintln!("⚠️ 更新缓存失败: {}", e);
            } else {
                println!("💾 缓存已更新");
            }
        }
    }

    println!("✨ 完成!");
    Ok(())
}
