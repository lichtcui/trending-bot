use serde_json::{json, Value};

use crate::repo::Repo;

/// 卡片变体：根据重复程度选择不同格式
pub enum CardVariant {
    /// 全部新项目 — 完整版 5 条
    Full,
    /// 有部分新增 — 只显示新项目，标记 🆕
    Partial,
    /// 全部重复 — 极简提示
    Stale,
}

/// 生成飞书 interactive 卡片消息 JSON
pub fn format_card(repos: &[Repo], variant: CardVariant) -> Value {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    match variant {
        CardVariant::Full => build_full_card(repos, &today),
        CardVariant::Partial => build_partial_card(repos, &today),
        CardVariant::Stale => build_stale_card(&today),
    }
}

/// 完整版卡片：蓝色标题，5 个项目
fn build_full_card(repos: &[Repo], date: &str) -> Value {
    let elements = build_repo_elements(repos, false, date);
    json!({
        "msg_type": "interactive",
        "card": {
            "header": {
                "title": {
                    "tag": "plain_text",
                    "content": format!("🔥 GitHub 今日热门 ({})", date)
                },
                "template": "blue"
            },
            "elements": elements
        }
    })
}

/// 更新版卡片：蓝色标题，新项目带 🆕 标识
fn build_partial_card(repos: &[Repo], date: &str) -> Value {
    let elements = build_repo_elements(repos, true, date);
    json!({
        "msg_type": "interactive",
        "card": {
            "header": {
                "title": {
                    "tag": "plain_text",
                    "content": "🔥 GitHub 今日热门 · 更新"
                },
                "template": "blue"
            },
            "elements": elements
        }
    })
}

/// 极简版卡片：灰色标题，提示无新项目
fn build_stale_card(date: &str) -> Value {
    json!({
        "msg_type": "interactive",
        "card": {
            "header": {
                "title": {
                    "tag": "plain_text",
                    "content": "📌 GitHub 今日热门"
                },
                "template": "grey"
            },
            "elements": [
                {
                    "tag": "div",
                    "text": {
                        "tag": "lark_md",
                        "content": format!(
                            "今日热门与昨日相同，无新项目上榜。\n\n上次更新: {}",
                            date
                        )
                    }
                }
            ]
        }
    })
}

/// 构建项目列表 element 数组
fn build_repo_elements(repos: &[Repo], show_new_badge: bool, date: &str) -> Vec<Value> {
    let mut elements: Vec<Value> = Vec::new();

    for (i, repo) in repos.iter().enumerate() {
        let badge = if show_new_badge { "🆕 " } else { "" };
        let rank = i + 1;
        let lang = repo.language.as_deref().unwrap_or("");
        let desc = repo.description.as_deref().unwrap_or("No description");

        let content = format!(
            "**{badge}#{rank} [{name}]({url})**\n⭐ {stars} stars · {lang}\n📈 +{today} stars today\n\n{desc}",
            badge = badge,
            rank = rank,
            name = repo.name,
            url = repo.url,
            stars = repo.stars_total,
            lang = lang,
            today = repo.stars_today,
            desc = desc,
        );

        elements.push(json!({
            "tag": "div",
            "text": {
                "tag": "lark_md",
                "content": content
            }
        }));

        // 项目间分隔线
        if i < repos.len() - 1 {
            elements.push(json!({ "tag": "hr" }));
        }
    }

    // 脚注
    elements.push(json!({
        "tag": "note",
        "elements": [
            {
                "tag": "plain_text",
                "content": format!("数据来源: GitHub Trending · {}", date)
            }
        ]
    }));

    elements
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_repo(name: &str) -> Repo {
        Repo {
            name: name.to_string(),
            url: format!("https://github.com/{}", name),
            description: Some("A test repo description.".into()),
            language: Some("Rust".into()),
            stars_total: 12345,
            stars_today: 567,
        }
    }

    #[test]
    fn test_full_card_structure() {
        let repos = vec![make_repo("owner/repo-a"), make_repo("owner/repo-b")];
        let card = format_card(&repos, CardVariant::Full);

        // 顶层字段
        assert_eq!(card["msg_type"], "interactive");
        assert!(card["card"].is_object());

        // header
        assert_eq!(card["card"]["header"]["template"], "blue");
        let title = card["card"]["header"]["title"]["content"].as_str().unwrap();
        assert!(title.starts_with("🔥 GitHub 今日热门"));

        // elements: 2 repos + 1 hr + 1 note = 4
        let elems = card["card"]["elements"].as_array().unwrap();
        assert_eq!(elems.len(), 4);

        // 第一条是 div
        assert_eq!(elems[0]["tag"], "div");
        let text = elems[0]["text"]["content"].as_str().unwrap();
        assert!(text.contains("owner/repo-a"));
        assert!(text.contains("12345")); // u64 直接格式化显示

        // 第二条是 hr
        assert_eq!(elems[1]["tag"], "hr");

        // 最后一条是 note
        let last = elems.last().unwrap();
        assert_eq!(last["tag"], "note");
    }

    #[test]
    fn test_partial_card_has_new_badge() {
        let repos = vec![make_repo("new/project")];
        let card = format_card(&repos, CardVariant::Partial);

        assert_eq!(card["card"]["header"]["template"], "blue");
        let title = card["card"]["header"]["title"]["content"].as_str().unwrap();
        assert!(title.contains("更新"));

        let content = card["card"]["elements"][0]["text"]["content"].as_str().unwrap();
        assert!(content.contains("🆕"));
    }

    #[test]
    fn test_stale_card_has_grey_template() {
        let repos: Vec<Repo> = vec![];
        let card = format_card(&repos, CardVariant::Stale);

        assert_eq!(card["card"]["header"]["template"], "grey");
        let title = card["card"]["header"]["title"]["content"].as_str().unwrap();
        assert!(title.contains("📌"));

        let text = card["card"]["elements"][0]["text"]["content"].as_str().unwrap();
        assert!(text.contains("无新项目上榜"));
    }

    #[test]
    fn test_card_has_msg_type() {
        let repos = vec![make_repo("a/b"), make_repo("c/d")];
        let full = format_card(&repos, CardVariant::Full);
        assert_eq!(full["msg_type"], "interactive");

        let partial = format_card(&repos, CardVariant::Partial);
        assert_eq!(partial["msg_type"], "interactive");

        let stale = format_card(&[], CardVariant::Stale);
        assert_eq!(stale["msg_type"], "interactive");
    }

    #[test]
    fn test_empty_repos_full_card_still_has_note() {
        let repos: Vec<Repo> = vec![];
        let card = format_card(&repos, CardVariant::Full);
        let elems = card["card"]["elements"].as_array().unwrap();
        // Only the note should be present
        assert_eq!(elems.len(), 1);
        assert_eq!(elems[0]["tag"], "note");
    }

    #[test]
    fn test_repo_with_no_description() {
        let mut repo = make_repo("no/desc");
        repo.description = None;
        let repos = vec![repo];
        let card = format_card(&repos, CardVariant::Full);
        let content = card["card"]["elements"][0]["text"]["content"].as_str().unwrap();
        assert!(content.contains("No description"));
    }

    #[test]
    fn test_repo_with_no_language() {
        let mut repo = make_repo("no/lang");
        repo.language = None;
        let repos = vec![repo];
        let card = format_card(&repos, CardVariant::Full);
        let content = card["card"]["elements"][0]["text"]["content"].as_str().unwrap();
        // Language field should be empty but not crash
        assert!(content.contains("stars"));
    }

    #[test]
    fn test_note_contains_date() {
        let repo = make_repo("a/b");
        let card = format_card(&[repo], CardVariant::Full);
        let note = card["card"]["elements"].as_array().unwrap().last().unwrap();
        let note_text = note["elements"][0]["content"].as_str().unwrap();
        assert!(note_text.contains("数据来源: GitHub Trending"));
    }
}
