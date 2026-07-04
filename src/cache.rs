use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::item::TrendingItem;

/// 缓存条目（每个源每个项目）
#[derive(Debug, Serialize, Deserialize, Clone)]
struct CacheEntry {
    id: String,
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_hash: Option<String>,
}

/// 单个源缓存
#[derive(Debug, Serialize, Deserialize)]
struct SourceCache {
    date: String,
    items: Vec<CacheEntry>,
}

/// 缓存文件格式
#[derive(Debug, Serialize, Deserialize)]
struct CacheData {
    version: String,
    sources: HashMap<String, SourceCache>,
}

/// 旧格式（用于自动迁移）
#[derive(Debug, Deserialize)]
struct OldCacheData {
    date: String,
    names: Vec<String>,
}

pub struct RepoCache {
    cache_dir: PathBuf,
}

impl RepoCache {
    pub fn new() -> Self {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        let cache_dir = PathBuf::from(home)
            .join("Library")
            .join("Caches")
            .join("trending-bot");
        RepoCache { cache_dir }
    }

    /// 仅用于测试 — 使用自定义目录
    pub fn new_with_dir(dir: PathBuf) -> Self {
        RepoCache { cache_dir: dir }
    }

    pub(crate) fn cache_path(&self) -> PathBuf {
        self.cache_dir.join("data_v2.json")
    }

    /// 加载全部缓存，自动迁移旧格式
    pub fn load_all(&self) -> Result<HashMap<String, HashSet<String>>> {
        let path = self.cache_path();
        if !path.exists() {
            return self.try_migrate_old_format();
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("读取缓存文件失败: {}", path.display()))?;
        let data: CacheData = serde_json::from_str(&content)
            .with_context(|| format!("解析缓存文件失败: {}", path.display()))?;

        Ok(data.sources.into_iter().map(|(k, v)| {
            (k, v.items.into_iter().map(|e| e.id).collect())
        }).collect())
    }

    /// 尝试加载并迁移旧格式缓存
    fn try_migrate_old_format(&self) -> Result<HashMap<String, HashSet<String>>> {
        let old_path = self.cache_dir.join("last_repos.json");
        if !old_path.exists() {
            return Ok(HashMap::new());
        }

        let content = fs::read_to_string(&old_path)?;
        let old: OldCacheData = serde_json::from_str(&content)?;
        let names: HashSet<String> = old.names.into_iter().collect();

        let today = old.date;
        let mut sources = HashMap::new();
        sources.insert("github_trending".to_string(), SourceCache {
            date: today,
            items: names.iter().map(|n| CacheEntry {
                id: n.clone(),
                url: format!("https://github.com/{}", n),
                content_hash: None,
            }).collect(),
        });

        let new_data = CacheData {
            version: "2".to_string(),
            sources,
        };

        fs::create_dir_all(&self.cache_dir)?;
        let path = self.cache_path();
        fs::write(&path, serde_json::to_string_pretty(&new_data)?)?;

        // 删除旧文件
        let _ = fs::remove_file(&old_path);

        let mut result = HashMap::new();
        result.insert("github_trending".to_string(), names);
        Ok(result)
    }

    /// 加载缓存中已抓取的内容 URL hash 集合
    pub fn load_content_hashes(&self) -> Result<HashMap<String, String>> {
        let path = self.cache_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = fs::read_to_string(&path)?;
        let data: CacheData = serde_json::from_str(&content)?;

        let mut hashes = HashMap::new();
        for (_source, sc) in data.sources {
            for entry in sc.items {
                if let Some(h) = entry.content_hash {
                    hashes.insert(entry.url, h);
                }
            }
        }
        Ok(hashes)
    }

    /// 计算 URL 的短 hash
    pub fn compute_url_hash(&self, url: &str) -> String {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        url.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    /// 保存当前项目列表到缓存
    pub fn save_from_items(&self, items: &[TrendingItem]) -> Result<()> {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();

        let mut sources: HashMap<String, Vec<CacheEntry>> = HashMap::new();
        for item in items {
            let entries = sources.entry(item.source.clone()).or_default();
            let content_hash = item.external_content.as_ref().map(|_| {
                self.compute_url_hash(&item.url)
            });
            entries.push(CacheEntry {
                id: item.id.clone(),
                url: item.url.clone(),
                content_hash,
            });
        }

        let data = CacheData {
            version: "2".to_string(),
            sources: sources.into_iter().map(|(k, v)| {
                (k, SourceCache { date: today.clone(), items: v })
            }).collect(),
        };

        fs::create_dir_all(&self.cache_dir)?;
        let content = serde_json::to_string_pretty(&data)?;
        fs::write(self.cache_path(), content)?;
        Ok(())
    }

    /// 多源差异对比
    pub fn diff_multi<'a>(
        &self,
        items: &'a [TrendingItem],
        last_by_source: &HashMap<String, HashSet<String>>,
    ) -> (Vec<&'a TrendingItem>, Vec<&'a TrendingItem>) {
        let mut old = Vec::new();
        let mut new = Vec::new();
        for item in items {
            let last_ids = last_by_source.get(&item.source);
            let is_old = last_ids.map_or(false, |ids| ids.contains(&item.id));
            if is_old {
                old.push(item);
            } else {
                new.push(item);
            }
        }
        (old, new)
    }
}

impl Default for RepoCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_cache() -> (RepoCache, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let cache = RepoCache::new_with_dir(dir.path().to_path_buf());
        (cache, dir)
    }

    fn make_item(source: &str, id: &str, url: &str) -> TrendingItem {
        TrendingItem {
            source: source.to_string(),
            id: id.to_string(),
            title: "test".into(),
            url: url.to_string(),
            description: None,
            score: None,
            comments_url: None,
            external_content: None,
        }
    }

    // 仍然保留旧的测试，但用新的接口重新实现
    fn make_repo(name: &str) -> crate::repo::Repo {
        crate::repo::Repo {
            name: name.to_string(),
            url: format!("https://github.com/{}", name),
            description: None,
            language: None,
            stars_total: 0,
            stars_today: 0,
        }
    }

    #[test]
    fn test_diff_all_new() {
        let cache = RepoCache::new();
        let repos = vec![make_repo("a/b"), make_repo("c/d")];
        let last = HashMap::new();
        // 转换为 TrendingItem 调用新接口
        let items: Vec<TrendingItem> = repos.iter().map(|r| TrendingItem {
            source: "github_trending".into(),
            id: r.name.clone(),
            title: r.name.clone(),
            url: r.url.clone(),
            description: None,
            score: None,
            comments_url: None,
            external_content: None,
        }).collect();
        let (old, new) = cache.diff_multi(&items, &last);
        assert!(old.is_empty());
        assert_eq!(new.len(), 2);
    }

    #[test]
    fn test_diff_all_old() {
        let cache = RepoCache::new();
        let repos = vec![make_repo("a/b"), make_repo("c/d")];
        let mut last = HashMap::new();
        last.insert("github_trending".into(), vec!["a/b".into(), "c/d".into()].into_iter().collect());
        let items: Vec<TrendingItem> = repos.iter().map(|r| TrendingItem {
            source: "github_trending".into(),
            id: r.name.clone(),
            title: r.name.clone(),
            url: r.url.clone(),
            description: None,
            score: None,
            comments_url: None,
            external_content: None,
        }).collect();
        let (old, new) = cache.diff_multi(&items, &last);
        assert_eq!(old.len(), 2);
        assert!(new.is_empty());
    }

    #[test]
    fn test_diff_partial_overlap() {
        let cache = RepoCache::new();
        let repos = vec![make_repo("a/b"), make_repo("c/d"), make_repo("e/f")];
        let mut last = HashMap::new();
        last.insert("github_trending".into(), vec!["a/b".into()].into_iter().collect());
        let items: Vec<TrendingItem> = repos.iter().map(|r| TrendingItem {
            source: "github_trending".into(),
            id: r.name.clone(),
            title: r.name.clone(),
            url: r.url.clone(),
            description: None,
            score: None,
            comments_url: None,
            external_content: None,
        }).collect();
        let (old, new) = cache.diff_multi(&items, &last);
        assert_eq!(old.len(), 1);
        assert_eq!(old[0].id, "a/b");
        assert_eq!(new.len(), 2);
    }

    #[test]
    fn test_diff_empty_repos() {
        let cache = RepoCache::new();
        let items: Vec<TrendingItem> = vec![];
        let mut last = HashMap::new();
        last.insert("github_trending".into(), vec!["a/b".into()].into_iter().collect());
        let (old, new) = cache.diff_multi(&items, &last);
        assert!(old.is_empty());
        assert!(new.is_empty());
    }

    #[test]
    fn test_save_and_load_empty() {
        let (cache, _dir) = make_cache();
        let loaded = cache.load_all().unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_save_and_load_items() {
        let (cache, _dir) = make_cache();
        let items = vec![
            make_item("github_trending", "a/b", "https://github.com/a/b"),
            make_item("hacker_news", "story_1", "https://example.com/1"),
        ];
        cache.save_from_items(&items).unwrap();
        let loaded = cache.load_all().unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded["github_trending"].len(), 1);
        assert!(loaded["github_trending"].contains("a/b"));
        assert_eq!(loaded["hacker_news"].len(), 1);
        assert!(loaded["hacker_news"].contains("story_1"));
    }

    #[test]
    fn test_diff_multi_all_new() {
        let (cache, _dir) = make_cache();
        let items = vec![
            make_item("github_trending", "a/b", ""),
            make_item("hacker_news", "story_1", ""),
        ];
        let last = HashMap::new();
        let (old, new) = cache.diff_multi(&items, &last);
        assert_eq!(old.len(), 0);
        assert_eq!(new.len(), 2);
    }

    #[test]
    fn test_diff_multi_partial() {
        let (cache, _dir) = make_cache();
        let items = vec![
            make_item("github_trending", "a/b", ""),
            make_item("github_trending", "c/d", ""),
            make_item("hacker_news", "story_1", ""),
        ];
        let mut last = HashMap::new();
        last.insert("github_trending".into(), vec!["a/b".into()].into_iter().collect());
        let (old, new) = cache.diff_multi(&items, &last);
        assert_eq!(old.len(), 1);
        assert_eq!(old[0].id, "a/b");
        assert_eq!(new.len(), 2);
    }

    #[test]
    fn test_old_format_migration() {
        let (cache, dir) = make_cache();
        let old_path = dir.path().join("last_repos.json");
        let old_data = r#"{"date":"2026-07-04","names":["a/b","c/d"]}"#;
        fs::write(&old_path, old_data).unwrap();

        let loaded = cache.load_all().unwrap();
        assert_eq!(loaded.len(), 1);
        assert!(loaded.contains_key("github_trending"));
        assert_eq!(loaded["github_trending"].len(), 2);
        assert!(loaded["github_trending"].contains("a/b"));

        // 旧文件应已被删除
        assert!(!old_path.exists());
        // 新文件应存在
        assert!(cache.cache_path().exists());
    }

    #[test]
    fn test_content_hash() {
        let (cache, _dir) = make_cache();
        let hash1 = cache.compute_url_hash("https://github.com/rust-lang/rust");
        let hash2 = cache.compute_url_hash("https://github.com/rust-lang/rust");
        assert_eq!(hash1, hash2);
        let hash3 = cache.compute_url_hash("https://example.com");
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_content_hashes_storage() {
        let (cache, _dir) = make_cache();
        // Save items with external content
        let content = crate::item::ExternalContent {
            url: "https://github.com/rust-lang/rust".into(),
            content_type: crate::item::ContentType::GitHubReadme,
            text: "content".into(),
            word_count: 1,
        };
        let items = vec![TrendingItem {
            source: "github_trending".into(),
            id: "rust-lang/rust".into(),
            title: "rust".into(),
            url: "https://github.com/rust-lang/rust".into(),
            description: None,
            score: None,
            comments_url: None,
            external_content: Some(content),
        }];
        cache.save_from_items(&items).unwrap();

        // Load content hashes
        let hashes = cache.load_content_hashes().unwrap();
        assert_eq!(hashes.len(), 1);
        assert!(hashes.contains_key("https://github.com/rust-lang/rust"));
    }

    #[test]
    fn test_content_hash_consistency() {
        let (cache, _dir) = make_cache();
        let hash = cache.compute_url_hash("hello");
        // DefaultHasher is deterministic per process, just verify it's a 16-char hex
        assert_eq!(hash.len(), 16);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
