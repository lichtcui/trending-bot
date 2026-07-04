use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::repo::Repo;

/// 上次推送的缓存记录
#[derive(Debug, Serialize, Deserialize)]
struct CacheData {
    date: String,
    names: Vec<String>,
}

/// 本地缓存管理器，用于对比每日 Trending 变化
pub struct RepoCache {
    cache_dir: PathBuf,
}

impl RepoCache {
    /// 创建缓存管理器。
    ///
    /// 使用 macOS 标准缓存目录: ~/Library/Caches/trending-bot/
    /// 创建缓存管理器。
    ///
    /// 使用 macOS 标准缓存目录: ~/Library/Caches/trending-bot/
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

    fn cache_path(&self) -> PathBuf {
        self.cache_dir.join("last_repos.json")
    }

    /// 加载上次缓存的 repo 名称集合。
    ///
    /// - 缓存文件不存在（首次运行）→ 返回空集合
    /// - 缓存文件损坏 → 返回明确的损坏提示
    pub fn load_last_names(&self) -> Result<HashSet<String>> {
        let path = self.cache_path();
        if !path.exists() {
            return Ok(HashSet::new());
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("读取缓存文件失败: {}", path.display()))?;
        let data: CacheData = serde_json::from_str(&content)
            .with_context(|| {
                format!(
                    "解析缓存文件失败，格式可能已损坏: {}\n  请手动删除该文件后重试: rm '{}'",
                    path.display(),
                    path.display(),
                )
            })?;
        Ok(data.names.into_iter().collect())
    }

    /// 保存本次 repo 名称列表到缓存文件。
    pub fn save_current_names(&self, repos: &[Repo]) -> Result<()> {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let data = CacheData {
            date: today,
            names: repos.iter().map(|r| r.name.clone()).collect(),
        };
        fs::create_dir_all(&self.cache_dir).context("创建缓存目录失败")?;
        let content =
            serde_json::to_string_pretty(&data).context("序列化缓存数据失败")?;
        fs::write(self.cache_path(), content).context("写入缓存文件失败")?;
        Ok(())
    }

    /// 对比本次结果与上次缓存，返回 `(旧项目列表, 新项目列表)`。
    ///
    /// - `old`：上次已经出现过的项目
    /// - `new`：本次新出现的项目（不在上次缓存中）
    pub fn diff<'a>(
        &self,
        repos: &'a [Repo],
        last_names: &HashSet<String>,
    ) -> (Vec<&'a Repo>, Vec<&'a Repo>) {
        let mut old = Vec::new();
        let mut new = Vec::new();
        for repo in repos {
            if last_names.contains(&repo.name) {
                old.push(repo);
            } else {
                new.push(repo);
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

    fn make_repo(name: &str) -> Repo {
        Repo {
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
        let last = HashSet::new();
        let (old, new) = cache.diff(&repos, &last);
        assert!(old.is_empty(), "expected no old repos");
        assert_eq!(new.len(), 2, "expected 2 new repos");
    }

    #[test]
    fn test_diff_all_old() {
        let cache = RepoCache::new();
        let repos = vec![make_repo("a/b"), make_repo("c/d")];
        let last: HashSet<String> = vec!["a/b".into(), "c/d".into()].into_iter().collect();
        let (old, new) = cache.diff(&repos, &last);
        assert_eq!(old.len(), 2, "expected 2 old repos");
        assert!(new.is_empty(), "expected no new repos");
    }

    #[test]
    fn test_diff_partial_overlap() {
        let cache = RepoCache::new();
        let repos = vec![make_repo("a/b"), make_repo("c/d"), make_repo("e/f")];
        let last: HashSet<String> = vec!["a/b".into()].into_iter().collect();
        let (old, new) = cache.diff(&repos, &last);
        assert_eq!(old.len(), 1, "expected 1 old repo");
        assert_eq!(old[0].name, "a/b");
        assert_eq!(new.len(), 2, "expected 2 new repos");
        assert_eq!(new[0].name, "c/d");
        assert_eq!(new[1].name, "e/f");
    }

    #[test]
    fn test_diff_empty_repos() {
        let cache = RepoCache::new();
        let repos: Vec<Repo> = vec![];
        let last: HashSet<String> = vec!["a/b".into()].into_iter().collect();
        let (old, new) = cache.diff(&repos, &last);
        assert!(old.is_empty(), "expected no old repos");
        assert!(new.is_empty(), "expected no new repos");
    }
}
