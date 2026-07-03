use serde::Serialize;
use crate::repo::Repo;

/// AI 可消费的结构化输出
#[derive(Debug, Serialize)]
pub struct AiOutput {
    pub tool: String,
    pub version: &'static str,
    pub fetched_at: String,
    pub count: usize,
    pub repos: Vec<Repo>,
    pub cache: CacheContext,
    pub feishu_pushed: bool,
}

/// 缓存对比上下文
#[derive(Debug, Serialize)]
pub struct CacheContext {
    /// 状态: "all_new" | "partial_update" | "no_change"
    pub status: String,
    pub new_count: usize,
    pub old_count: usize,
    pub new_repos: Vec<String>,
    pub is_duplicate: bool,
}

impl AiOutput {
    pub fn new(repos: &[Repo], old_count: usize, new_repos: &[String], feishu_pushed: bool) -> Self {
        let (status, is_duplicate) = if old_count == 0 && !new_repos.is_empty() {
            ("all_new".to_string(), false)
        } else if !new_repos.is_empty() {
            ("partial_update".to_string(), false)
        } else {
            ("no_change".to_string(), true)
        };

        AiOutput {
            tool: "trending-bot".to_string(),
            version: env!("CARGO_PKG_VERSION"),
            fetched_at: chrono::Local::now().to_rfc3339(),
            count: repos.len(),
            repos: repos.to_vec(),
            cache: CacheContext {
                status,
                new_count: new_repos.len(),
                old_count,
                new_repos: new_repos.to_vec(),
                is_duplicate,
            },
            feishu_pushed,
        }
    }
}
