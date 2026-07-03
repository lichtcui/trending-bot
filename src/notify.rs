use anyhow::{Context, Result};
use serde_json::{json, Value};

/// 推送器 trait — 为今后扩展到 Slack/Discord 等渠道做准备
pub trait Notifier {
    fn send(&self, payload: &Value) -> Result<()>;
}

/// 飞书应用推送器（使用 App ID + App Secret 认证，发送给指定用户 1on1）
pub struct FeishuAppNotifier {
    app_id: String,
    app_secret: String,
    open_id: String,
    client: reqwest::blocking::Client,
}

impl FeishuAppNotifier {
    /// 创建飞书应用推送器
    ///
    /// - `app_id`: 飞书企业自建应用的 App ID
    /// - `app_secret`: 飞书企业自建应用的 App Secret
    /// - `open_id`: 你的用户 open_id（在飞书开放平台 → 应用 → 测试用户 中获取）
    pub fn new(app_id: &str, app_secret: &str, open_id: &str) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("创建 HTTP 客户端失败");
        FeishuAppNotifier {
            app_id: app_id.to_string(),
            app_secret: app_secret.to_string(),
            open_id: open_id.to_string(),
            client,
        }
    }

    /// 获取 tenant_access_token
    fn get_token(&self) -> Result<String> {
        let resp = self
            .client
            .post("https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal")
            .json(&json!({
                "app_id": self.app_id,
                "app_secret": self.app_secret,
            }))
            .send()
            .context("请求 tenant_access_token 失败")?;

        let body: Value = resp.json().context("解析 token 响应 JSON 失败")?;
        let code = body["code"].as_i64().unwrap_or(-1);
        anyhow::ensure!(code == 0, "获取 token 失败 (code={}): {}", code, body["msg"]);

        let token = body["tenant_access_token"]
            .as_str()
            .context("token 响应缺少 tenant_access_token 字段")?;
        Ok(token.to_string())
    }

    /// 发送消息到用户 1on1 对话
    fn send_message(&self, token: &str, content: &str) -> Result<()> {
        let resp = self
            .client
            .post("https://open.feishu.cn/open-apis/im/v1/messages?receive_id_type=open_id")
            .header("Authorization", format!("Bearer {}", token))
            .json(&json!({
                "receive_id": self.open_id,
                "msg_type": "interactive",
                "content": content,
            }))
            .send()
            .context("发送飞书消息失败")?;

        let body: Value = resp.json().context("解析发送消息响应 JSON 失败")?;
        let code = body["code"].as_i64().unwrap_or(-1);
        anyhow::ensure!(code == 0, "发送消息失败 (code={}): {}", code, body["msg"]);

        Ok(())
    }
}

impl Notifier for FeishuAppNotifier {
    fn send(&self, payload: &Value) -> Result<()> {
        let token = self.get_token()?;
        let content = serde_json::to_string(payload.get("card").unwrap_or(payload))
            .context("序列化卡片内容失败")?;
        self.send_message(&token, &content)?;
        println!("✅ 成功推送到飞书");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_notifier() {
        let notifier = FeishuAppNotifier::new("app_id", "app_secret", "ou_test");
        assert_eq!(notifier.app_id, "app_id");
        assert_eq!(notifier.open_id, "ou_test");
    }

    #[test]
    fn test_send_with_invalid_credentials() {
        let notifier = FeishuAppNotifier::new("invalid", "invalid", "ou_test");
        let payload = serde_json::json!({"msg_type": "text", "content": {"text": "test"}});
        let result = notifier.send(&payload);
        assert!(result.is_err(), "无效凭证应返回错误");
    }

    #[test]
    fn test_trait_object() {
        let notifier = FeishuAppNotifier::new("app_id", "app_secret", "ou_test");
        let notifier_ref: &dyn Notifier = &notifier;
        let _ = notifier_ref;
    }
}
