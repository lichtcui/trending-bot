use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::item::{ContentType, ExternalContent, TrendingItem};

/// 单次 API 调用中累计的内容字符上限
/// DeepSeek V4 Flash 支持 ~128K tokens ≈ 300K chars
/// 设为 50K chars ≈ ~20K tokens，留出充足余量
const MAX_BATCH_CONTENT_CHARS: usize = 50_000;

/// DeepSeek API 总结器
#[derive(Debug)]
pub struct Summarizer {
    api_key: String,
    client: reqwest::blocking::Client,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    type_: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

impl Summarizer {
    /// 创建总结器，自动从 Keychain 或环境变量读取 API key
    pub fn new() -> Result<Self> {
        let api_key = get_deepseek_api_key()?;
        let client = reqwest::blocking::Client::builder()
            .user_agent("trending-bot/0.1.0")
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .context("创建 HTTP 客户端失败")?;
        Ok(Summarizer { api_key, client })
    }

    /// 使用自定义 key（方便测试）
    pub fn with_key(api_key: String) -> Self {
        let client = reqwest::blocking::Client::builder()
            .user_agent("trending-bot/0.1.0")
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("创建 HTTP 客户端失败");
        Summarizer { api_key, client }
    }

    // ───────────────────────────────
    // 公开入口
    // ───────────────────────────────

    /// 对一批条目进行总结（自动分组、批量调 API、失败单条回退）
    /// 返回成功总结的条数
    pub fn summarize_items(&self, items: &mut [TrendingItem]) -> usize {
        // 先按索引建立分组
        let batch_indices = group_indices(items);
        let total_batches = batch_indices.len();
        let mut summarized = 0usize;

        for (batch_idx, indices) in batch_indices.iter().enumerate() {
            if indices.len() == 1 {
                let idx = indices[0];
                eprintln!("  └─ [{}/{}] 单条总结: {}", batch_idx + 1, total_batches, items[idx].title);
                match self.summarize_single(&items[idx]) {
                    Ok(s) => {
                        items[idx].summary = Some(s);
                        summarized += 1;
                    }
                    Err(e) => eprintln!("  ⚠️ 总结失败 [{}]: {}", items[idx].id, e),
                }
            } else {
                // 多条批量 — 临时收集引用用于调用
                let batch_refs: Vec<&TrendingItem> = indices.iter().map(|&i| &items[i]).collect();
                eprintln!("  └─ [{}/{}] 批量总结 {} 条...", batch_idx + 1, total_batches, batch_refs.len());
                match self.summarize_batch(&batch_refs) {
                    Ok(results) => {
                        for (id, summary) in &results {
                            if let Some(target) = items.iter_mut().find(|i| i.id == *id) {
                                target.summary = Some(summary.clone());
                                summarized += 1;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("  ⚠️ 批量总结失败 ({}), 逐条回退: {}", e, indices.len());
                        for &idx in indices {
                            match self.summarize_single(&items[idx]) {
                                Ok(s) => {
                                    items[idx].summary = Some(s);
                                    summarized += 1;
                                }
                                Err(e2) => eprintln!("  ⚠️ 回退总结失败 [{}]: {}", items[idx].id, e2),
                            }
                        }
                    }
                }
            }
        }

        summarized
    }

    // ───────────────────────────────
    // 单条总结（保留给回退路径和单条场景）
    // ───────────────────────────────

    fn summarize_single(&self, item: &TrendingItem) -> Result<String> {
        let content = item.external_content.as_ref()
            .context("条目没有外部内容可总结")?;
        let prompt = build_prompt(content);
        let system_prompt = "你是一个技术热点摘要助手。请用简洁的中文总结以下内容的核心要点，限 3-5 句话。\
            聚焦于：这个项目/文章解决什么问题、有什么亮点、为什么值得关注。";

        let request = ChatRequest {
            model: "deepseek-chat".to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: prompt,
                },
            ],
            max_tokens: 300,
            temperature: 0.3,
            response_format: None,
        };

        let chat_resp = self.send_request(&request)?;
        let summary = chat_resp.choices
            .first()
            .map(|c| c.message.content.trim().to_string())
            .context("DeepSeek 返回空响应")?;
        Ok(summary)
    }

    // ───────────────────────────────
    // 批量总结（一组条目一次 API 调用）
    // ───────────────────────────────

    fn summarize_batch(&self, items: &[&TrendingItem]) -> Result<Vec<(String, String)>> {
        let batch_prompt = build_batch_prompt(items);
        let system_prompt = "你是一个技术热点摘要助手。用户会提供若干条目及其内容。\
            请用简洁的中文逐条总结，每条 2-3 句话，聚焦于解决什么问题、有什么亮点。\
            必须返回合法的 JSON 对象，键为 \"0\", \"1\", \"2\"... 对应条目的序号，值为总结文本。\
            示例：{\"0\": \"总结内容...\", \"1\": \"总结内容...\"}";

        // max_tokens: 每条约 300 tokens + prompt 开销
        let max_tokens = (items.len() as u32) * 300 + 500;

        let request = ChatRequest {
            model: "deepseek-chat".to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: batch_prompt,
                },
            ],
            max_tokens,
            temperature: 0.3,
            response_format: Some(ResponseFormat {
                type_: "json_object".to_string(),
            }),
        };

        let chat_resp = self.send_request(&request)?;
        let text = chat_resp.choices
            .first()
            .map(|c| c.message.content.trim().to_string())
            .context("DeepSeek 返回空响应")?;

        // 解析 JSON 响应
        let parsed: serde_json::Value = serde_json::from_str(&text)
            .context("批量总结响应不是有效 JSON")?;

        let obj = parsed.as_object()
            .context("批量总结响应不是 JSON 对象")?;

        let mut results = Vec::new();
        for (i, item) in items.iter().enumerate() {
            let key = format!("{}", i);
            if let Some(summary) = obj.get(&key).and_then(|v| v.as_str()) {
                results.push((item.id.clone(), summary.to_string()));
            } else {
                anyhow::bail!("批量总结响应缺少条目 {} 的总结 (key={})", item.id, key);
            }
        }

        Ok(results)
    }

    // ───────────────────────────────
    // 底层 HTTP 请求
    // ───────────────────────────────

    fn send_request(&self, request: &ChatRequest) -> Result<ChatResponse> {
        let resp = self.client
            .post("https://api.deepseek.com/chat/completions")
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(request)
            .send()
            .context("请求 DeepSeek API 失败")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            anyhow::bail!("DeepSeek API 返回 {}: {}", status, body);
        }

        let chat_resp: ChatResponse = resp
            .json()
            .context("解析 DeepSeek API 响应失败")?;

        Ok(chat_resp)
    }
}

impl Default for Summarizer {
    fn default() -> Self {
        Self::new().expect("Failed to create Summarizer; ensure DEEPSEEK_API_KEY is set")
    }
}

// ───────────────────────────────
// 分组逻辑
// ───────────────────────────────

/// 将有外部内容的条目按内容长度分组，返回每组在原切片中的索引
/// 每组累计内容字符不超过 MAX_BATCH_CONTENT_CHARS
fn group_indices(items: &[TrendingItem]) -> Vec<Vec<usize>> {
    let mut batches: Vec<Vec<usize>> = Vec::new();
    let mut current_batch: Vec<usize> = Vec::new();
    let mut current_len = 0usize;

    for (i, item) in items.iter().enumerate() {
        let content_len = item.external_content
            .as_ref()
            .map(|c| c.text.len())
            .unwrap_or(0);

        if content_len == 0 {
            continue; // 跳过无内容的条目
        }

        // 如果这个条目本身已经超过上限，或者加上后就超了，另开一批
        if content_len >= MAX_BATCH_CONTENT_CHARS
            || (!current_batch.is_empty() && current_len + content_len > MAX_BATCH_CONTENT_CHARS)
        {
            if !current_batch.is_empty() {
                batches.push(std::mem::take(&mut current_batch));
                current_len = 0;
            }
        }

        current_batch.push(i);
        current_len += content_len;
    }

    if !current_batch.is_empty() {
        batches.push(current_batch);
    }

    batches
}

// ───────────────────────────────
// Prompt 构建
// ───────────────────────────────

/// 构建单条 prompt
fn build_prompt(content: &ExternalContent) -> String {
    match content.content_type {
        ContentType::GitHubReadme => {
            format!(
                "以下是 GitHub 仓库的 README 内容（{} 字）：\n\n{}",
                content.word_count, content.text
            )
        }
        ContentType::GitHubIssue => {
            format!(
                "以下是 GitHub Issue 内容（{} 字）：\n\n{}",
                content.word_count, content.text
            )
        }
        ContentType::WebArticle => {
            format!(
                "以下是网页文章内容（{} 字）：\n\n{}",
                content.word_count, content.text
            )
        }
    }
}

/// 构建批量 prompt
fn build_batch_prompt(items: &[&TrendingItem]) -> String {
    let mut parts = Vec::new();
    parts.push(format!(
        "请总结以下 {} 条内容，每条用 2-3 句中文概括核心要点。", items.len()
    ));
    parts.push("返回 JSON 格式，键为序号 \"0\", \"1\", ...，值为总结文本。".to_string());
    parts.push(String::new());

    for (i, item) in items.iter().enumerate() {
        let content = match item.external_content.as_ref() {
            Some(c) => c,
            None => continue,
        };
        let type_label = match content.content_type {
            ContentType::GitHubReadme => "GitHub 仓库",
            ContentType::GitHubIssue => "GitHub Issue",
            ContentType::WebArticle => "网页文章",
        };
        parts.push(format!(
            "--- 条目 {}: [{}] {} ---\n{}",
            i, type_label, item.title, content.text
        ));
    }

    parts.join("\n")
}

// ───────────────────────────────
// API key 读取
// ───────────────────────────────

/// 从 Keychain 或环境变量读取 DeepSeek API key
///
/// 优先级：
/// 1. `DEEPSEEK_API_KEY` 环境变量
/// 2. macOS Keychain（service name: `DEEPSEEK_API_KEY`）
fn get_deepseek_api_key() -> Result<String> {
    // 优先环境变量
    if let Ok(key) = std::env::var("DEEPSEEK_API_KEY") {
        if !key.is_empty() {
            return Ok(key);
        }
    }

    // 回退到 macOS Keychain
    let output = std::process::Command::new("security")
        .args(["find-generic-password", "-s", "DEEPSEEK_API_KEY", "-w"])
        .output()
        .context("调用 security CLI 失败（需要 macOS Keychain）")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "未找到 DeepSeek API key。请通过环境变量 DEEPSEEK_API_KEY 设置，\
            或添加到 macOS Keychain: security add-generic-password -s DEEPSEEK_API_KEY -a deepseek -w 'your-key'\
            \nsecurity 错误: {}",
            stderr.trim()
        );
    }

    let key = String::from_utf8(output.stdout)
        .context("security 输出不是有效 UTF-8")?
        .trim()
        .to_string();

    if key.is_empty() {
        anyhow::bail!("Keychain 中的 DEEPSEEK_API_KEY 为空");
    }

    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_prompt_github() {
        let content = ExternalContent {
            url: "https://github.com/rust-lang/rust".into(),
            content_type: ContentType::GitHubReadme,
            text: "# Rust\n\nA safe language.".into(),
            word_count: 5,
        };
        let prompt = build_prompt(&content);
        assert!(prompt.contains("GitHub 仓库"));
        assert!(prompt.contains("Rust\n\nA safe language."));
    }

    #[test]
    fn test_build_prompt_issue() {
        let content = ExternalContent {
            url: "https://github.com/rust-lang/rust/issues/123".into(),
            content_type: ContentType::GitHubIssue,
            text: "# Bug report\n\nSomething broke.".into(),
            word_count: 5,
        };
        let prompt = build_prompt(&content);
        assert!(prompt.contains("GitHub Issue"));
        assert!(prompt.contains("Bug report"));
    }

    #[test]
    fn test_build_prompt_web() {
        let content = ExternalContent {
            url: "https://example.com/article".into(),
            content_type: ContentType::WebArticle,
            text: "Some article text.".into(),
            word_count: 3,
        };
        let prompt = build_prompt(&content);
        assert!(prompt.contains("网页文章"));
        assert!(prompt.contains("Some article text."));
    }

    #[test]
    fn test_group_indices_empty() {
        let items: Vec<TrendingItem> = vec![];
        let batches = group_indices(&items);
        assert!(batches.is_empty());
    }

    #[test]
    fn test_group_indices_all_no_content() {
        let items = vec![
            TrendingItem {
                source: "test".into(), id: "1".into(), title: "a".into(),
                url: "https://x.com".into(), score: None,
                external_content: None, summary: None,
            },
        ];
        let batches = group_indices(&items);
        assert!(batches.is_empty());
    }

    #[test]
    fn test_group_indices_small_content_single_batch() {
        let items = vec![
            TrendingItem {
                source: "test".into(), id: "1".into(), title: "a".into(),
                url: "https://x.com".into(), score: None,
                external_content: Some(ExternalContent {
                    url: "".into(), content_type: ContentType::WebArticle,
                    text: "short".into(), word_count: 1,
                }),
                summary: None,
            },
            TrendingItem {
                source: "test".into(), id: "2".into(), title: "b".into(),
                url: "https://y.com".into(), score: None,
                external_content: Some(ExternalContent {
                    url: "".into(), content_type: ContentType::WebArticle,
                    text: "also short".into(), word_count: 2,
                }),
                summary: None,
            },
        ];
        let batches = group_indices(&items);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].len(), 2);
    }

    #[test]
    fn test_group_indices_large_content_splits() {
        let items = vec![
            TrendingItem {
                source: "test".into(), id: "big".into(), title: "big".into(),
                url: "https://x.com".into(), score: None,
                external_content: Some(ExternalContent {
                    url: "".into(), content_type: ContentType::WebArticle,
                    text: "x".repeat(MAX_BATCH_CONTENT_CHARS),
                    word_count: 1,
                }),
                summary: None,
            },
            TrendingItem {
                source: "test".into(), id: "small".into(), title: "small".into(),
                url: "https://y.com".into(), score: None,
                external_content: Some(ExternalContent {
                    url: "".into(), content_type: ContentType::WebArticle,
                    text: "short text".into(), word_count: 2,
                }),
                summary: None,
            },
        ];
        let batches = group_indices(&items);
        assert_eq!(batches.len(), 2);
    }

    #[test]
    fn test_build_batch_prompt_contains_items() {
        let items = vec![
            TrendingItem {
                source: "test".into(), id: "1".into(), title: "First".into(),
                url: "https://a.com".into(), score: None,
                external_content: Some(ExternalContent {
                    url: "".into(), content_type: ContentType::GitHubReadme,
                    text: "Content A".into(), word_count: 2,
                }),
                summary: None,
            },
            TrendingItem {
                source: "test".into(), id: "2".into(), title: "Second".into(),
                url: "https://b.com".into(), score: None,
                external_content: Some(ExternalContent {
                    url: "".into(), content_type: ContentType::WebArticle,
                    text: "Content B".into(), word_count: 2,
                }),
                summary: None,
            },
        ];
        let refs: Vec<&TrendingItem> = items.iter().collect();
        let prompt = build_batch_prompt(&refs);
        assert!(prompt.contains("条目 0"));
        assert!(prompt.contains("条目 1"));
        assert!(prompt.contains("First"));
        assert!(prompt.contains("Second"));
        assert!(prompt.contains("Content A"));
        assert!(prompt.contains("Content B"));
    }

    #[test]
    fn test_summarizer_no_key_no_panic() {
        std::env::remove_var("DEEPSEEK_API_KEY");
        let result = Summarizer::new();
        if result.is_err() {
            let err = result.unwrap_err();
            let msg = format!("{}", err);
            assert!(
                msg.contains("API key") || msg.contains("Keychain") || msg.contains("security"),
                "错误信息应有帮助: {}",
                msg
            );
        }
    }
}
