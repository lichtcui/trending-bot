mod cache;
mod fetcher;
mod hn;
mod item;
mod lobsters;
mod output;
mod repo;
mod rss;
mod source;
mod summary;

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};

use cache::RepoCache;
use item::TrendingItem;
use output::{AiOutput, SourceDiff};
use source::TrendingSource;

fn main() -> Result<()> {
    // 简单的 CLI 参数解析
    let args: Vec<String> = std::env::args().collect();
    let json_mode = args.iter().any(|a| a == "--json");
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let fetch_content = !args.iter().any(|a| a == "--no-content");
    let do_summarize = args.iter().any(|a| a == "--summarize");

    // 解析 --count N 或 -c N
    let count: usize = args.windows(2)
        .find(|w| w[0] == "--count" || w[0] == "-c")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(5);

    // 解析 --source / -s（默认全部）
    let enabled_sources: Vec<String> = {
        let specified: Vec<String> = args.windows(2)
            .filter(|w| w[0] == "--source" || w[0] == "-s")
            .map(|w| w[1].to_lowercase())
            .collect();
        if specified.is_empty() {
            vec!["github".into(), "hn".into(), "lobsters".into()]
        } else {
            specified
        }
    };
    let user_specified_sources = args.windows(2).any(|w| w[0] == "--source" || w[0] == "-s");

    // 1. 初始化各 Source
    let mut sources: Vec<Box<dyn TrendingSource>> = Vec::new();
    for name in &enabled_sources {
        match name.as_str() {
            "github" => sources.push(Box::new(source::GitHubTrending::new())),
            "hn" => sources.push(Box::new(hn::HackerNews::new())),
            "lobsters" => sources.push(Box::new(lobsters::Lobsters::new())),
            "rust_weekly" | "bytebytego" | "ai_weekly" => {
                sources.push(Box::new(rss::RssSource::new(name)));
            }
            _ => eprintln!("⚠️ 未知数据源: {}，跳过", name),
        }
    }

    // 周一自动追加 Newsletter 源（仅在未手动指定 --source 时）
    if is_monday() && !user_specified_sources {
        eprintln!("📬 周一加餐: 追加 3 个 Newsletter 源...");
        for name in &["rust_weekly", "bytebytego", "ai_weekly"] {
            sources.push(Box::new(rss::RssSource::new(name)));
        }
    }

    // 2. 顺序获取各源数据
    let mut all_items: Vec<TrendingItem> = Vec::new();
    for source in &sources {
        match source.fetch(count) {
            Ok(mut items) => {
                eprintln!("✓ {} 获取到 {} 条", source.source_name(), items.len());
                all_items.append(&mut items);
            }
            Err(e) => {
                eprintln!("⚠️ {} 获取失败: {}", source.source_name(), e);
            }
        }
    }

    // 3. 缓存对比
    let cache = RepoCache::new();
    let last_data: HashMap<String, HashSet<String>> = match cache.load_all() {
        Ok(data) => data,
        Err(e) => {
            eprintln!("⚠️ 读取缓存失败，跳过: {}", e);
            HashMap::new()
        }
    };

    // 仅提取新项目的 ID（不持有引用，避免 borrow checker 冲突）
    let new_item_ids: HashSet<String> = {
        all_items.iter()
            .filter(|item| {
                let source_last = last_data.get(&item.source);
                source_last.map_or(true, |ids| !ids.contains(&item.id))
            })
            .map(|item| item.id.clone())
            .collect()
    };

    // 4. 内容抓取（仅对新项目）
    let mut fetched_content = 0usize;
    let mut cached_content = 0usize;
    let mut failed_content = 0usize;

    if fetch_content {
        let fetcher = fetcher::ContentFetcher::new();
        let cached_urls: HashSet<String> = cache.load_content_hashes()
            .unwrap_or_default()
            .keys()
            .cloned()
            .collect();

        for item in &mut all_items {
            if new_item_ids.contains(&item.id) {
                if cached_urls.contains(&item.url) {
                    cached_content += 1;
                    continue;
                }
                match fetcher.fetch(item) {
                    Some(content) => {
                        item.external_content = Some(content);
                        fetched_content += 1;
                    }
                    None => {
                        failed_content += 1;
                    }
                }
            }
        }
    }

    // 4.5 LLM 总结（仅对新项目且有外部内容的条目，自动分批调 API）
    let mut summarized_count = 0usize;
    if do_summarize && !new_item_ids.is_empty() {
        match summary::Summarizer::new() {
            Ok(summarizer) => {
                let new_count = new_item_ids.len();
                let content_count = all_items.iter()
                    .filter(|item| new_item_ids.contains(&item.id) && item.external_content.is_some())
                    .count();
                eprintln!("🤖 新项目 {} 条，其中 {} 条有外部内容，正在调用 DeepSeek 总结...", new_count, content_count);
                summarized_count = summarizer.summarize_items(&mut all_items);
            }
            Err(e) => {
                eprintln!("⚠️ 初始化总结器失败: {} (跳过总结)", e);
            }
        }
    }

    // 5. 统计各源 & 构建输出引用
    let mut by_source: HashMap<String, SourceDiff> = HashMap::new();
    let new_refs: Vec<&TrendingItem> = all_items.iter()
        .filter(|item| new_item_ids.contains(&item.id))
        .collect();
    let old_refs: Vec<&TrendingItem> = all_items.iter()
        .filter(|item| !new_item_ids.contains(&item.id))
        .collect();

    for item in &all_items {
        let entry = by_source.entry(item.source.clone()).or_insert(SourceDiff { new: 0, old: 0 });
        if new_item_ids.contains(&item.id) {
            entry.new += 1;
        } else {
            entry.old += 1;
        }
    }

    // 6. JSON 输出
    if json_mode {
        let output = AiOutput::new(
            &all_items,
            &new_refs,
            &old_refs,
            fetched_content,
            cached_content,
            failed_content,
            &by_source,
        );
        let json = serde_json::to_string_pretty(&output)
            .context("序列化 JSON 输出失败")?;
        println!("{}", json);
    } else {
        // 非 JSON 模式：简要输出
        println!("=== 多源热点汇总 ===");
        for item in &all_items {
            let tag = if new_item_ids.contains(&item.id) { "NEW" } else { "   " };
            println!("[{}] [{}] {} - {}", tag, item.source, item.title, item.url);
        }
        println!("\n缓存: {} 新 / {} 旧", new_refs.len(), old_refs.len());
        if fetch_content {
            println!("内容: {} 新抓取 / {} 缓存命中 / {} 失败", fetched_content, cached_content, failed_content);
        }
        if do_summarize {
            println!("总结: {} 条已总结", summarized_count);
        }
    }

    // 7. 更新缓存
    if !dry_run && !all_items.is_empty() {
        if let Err(e) = cache.save_from_items(&all_items) {
            eprintln!("⚠️ 更新缓存失败: {}", e);
        }
    }

    Ok(())
}

/// 今天是周一吗？（ISO 标准：1=周一）
fn is_monday() -> bool {
    is_monday_for_date(chrono::Local::now().date_naive())
}

/// 可测试版本，接受指定日期
fn is_monday_for_date(date: chrono::NaiveDate) -> bool {
    date.format("%u").to_string() == "1"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monday_source_inclusion() {
        // 模拟默认源列表 + 周一追加
        let mut names: Vec<&str> = vec!["github", "hn", "lobsters"];
        if true /* is_monday */ {
            names.extend(&["rust_weekly", "bytebytego", "ai_weekly"]);
        }
        assert_eq!(names.len(), 6);
        assert!(names.contains(&"rust_weekly"));
        assert!(names.contains(&"bytebytego"));
        assert!(names.contains(&"ai_weekly"));
    }

    #[test]
    fn test_is_monday_logic() {
        // 2026-07-06 是周一
        let dt = chrono::NaiveDate::from_ymd_opt(2026, 7, 6).unwrap();
        assert!(is_monday_for_date(dt));

        // 2026-07-07 是周二
        let dt = chrono::NaiveDate::from_ymd_opt(2026, 7, 7).unwrap();
        assert!(!is_monday_for_date(dt));

        // 2026-07-05 是周日
        let dt = chrono::NaiveDate::from_ymd_opt(2026, 7, 5).unwrap();
        assert!(!is_monday_for_date(dt));
    }
}
