mod cache;
mod format;
mod keychain;
mod notify;
mod output;
mod repo;
mod source;

use std::collections::HashSet;

use anyhow::{Context, Result};
use format::CardVariant;
use source::{GitHubTrending, TrendingSource};
use notify::{FeishuAppNotifier, Notifier};
use cache::RepoCache;

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

    // 从 macOS Keychain 读取飞书凭证（和 xhs-recipe 方式一致）
    let app_id = keychain::Keychain::get_app_id()
        .unwrap_or_default();
    let app_secret = keychain::Keychain::get_app_secret()
        .unwrap_or_default();
    let open_id = keychain::Keychain::get_open_id()
        .unwrap_or_default();

    // 1. 获取 Trending 项目
    let source = GitHubTrending::new();
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

    let has_new = !new.is_empty();
    let all_new = old.is_empty() && has_new;
    let all_old = old.len() == repos.len() && !repos.is_empty();

    // 3. 飞书推送（除非 dry-run）
    if !dry_run {
        anyhow::ensure!(!app_id.is_empty() && !app_secret.is_empty(),
            "缺少飞书凭证。请运行以下命令存入 Keychain:\n  \
             security add-generic-password -a \"$USER\" -s FEISHU_APP_ID -w \"<App ID>\" -U\n  \
             security add-generic-password -a \"$USER\" -s FEISHU_APP_SECRET -w \"<App Secret>\" -U");
        anyhow::ensure!(!open_id.is_empty(),
            "缺少 FEISHU_OPEN_ID。请运行:\n  \
             security add-generic-password -a \"$USER\" -s FEISHU_OPEN_ID -w \"<open_id>\" -U");

        let card = if all_new {
            format::format_card(&repos, CardVariant::Full)
        } else if has_new {
            // 只在真正需要时克隆 new_owned（避免 --dry-run 或 all_new 时浪费）
            let new_owned: Vec<_> = new.iter().map(|r| (*r).clone()).collect();
            format::format_card(&new_owned, CardVariant::Partial)
        } else {
            format::format_card(&[], CardVariant::Stale)
        };

        let notifier = FeishuAppNotifier::new(&app_id, &app_secret, &open_id);
        notifier.send(&card)?;

        // 有新增项目时才更新缓存（全部重复则跳过）
        if !all_old {
            if let Err(e) = cache.save_current_names(&repos) {
                eprintln!("⚠️ 更新缓存失败: {}", e);
            }
        }
    }

    // 4. JSON 输出（AI 调用模式）
    if json_mode {
        // 只在 JSON 模式需要时分配 new_names
        let new_names: Vec<_> = new.iter().map(|r| r.name.clone()).collect();
        let ai_output = output::AiOutput::new(&repos, old.len(), &new_names, !dry_run);
        let json = serde_json::to_string_pretty(&ai_output)
            .context("序列化 JSON 输出失败")?;
        println!("{}", json);
    }

    Ok(())
}
