use anyhow::{Context, Result};
use serde_json::{json, Value};

/// 推送器 trait — 为今后扩展到 Slack/Discord 等渠道做准备
pub trait Notifier {
    fn send(&self, payload: &Value) -> Result<()>;
}

/// 飞书应用推送器（使用 App ID + App Secret 认证）
pub struct FeishuAppNotifier {
    app_id: String,
    app_secret: String,
    chat_id: String,
    client: reqwest::blocking::Client,
}

impl FeishuAppNotifier {
    /// 创建飞书应用推送器
    ///
    /// 需要飞书企业自建应用的 App ID 和 App Secret，
    /// 以及目标群的 chat_id（格式: oc_xxxxxxxxxx）
    pub fn new(app_id: &str, app_secret: &str, chat_id: &str) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("创建 HTTP 客户端失败");
        FeishuAppNotifier {
            app_id: app_id.to_string(),
            app_secret: app_secret.to_string(),
            chat_id: chat_id.to_string(),
            client,
        }
    }

    /// 获取 tenant_access_token（自动处理 2 小时过期）
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

    /// 发送消息到指定群聊
    fn send_message(&self, token: &str, content: &str) -> Result<()> {
        let resp = self
            .client
            .post("https://open.feishu.cn/open-apis/im/v1/messages?receive_id_type=chat_id")
            .header("Authorization", format!("Bearer {}", token))
            .json(&json!({
                "receive_id": self.chat_id,
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
        // interactive 卡片的 content 字段是 JSON 字符串
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
        let notifier = FeishuAppNotifier::new("app_id", "app_secret", "oc_test");
        assert_eq!(notifier.app_id, "app_id");
        assert_eq!(notifier.chat_id, "oc_test");
    }

    #[test]
    fn test_send_with_invalid_credentials() {
        let notifier = FeishuAppNotifier::new("invalid", "invalid", "oc_test");
        let payload = serde_json::json!({"msg_type": "text", "content": {"text": "test"}});
        let result = notifier.send(&payload);
        assert!(result.is_err(), "无效凭证应返回错误");
    }

    #[test]
    fn test_trait_object() {
        let notifier = FeishuAppNotifier::new("app_id", "app_secret", "oc_test");
        let notifier_ref: &dyn Notifier = &notifier;
        let _ = notifier_ref; // 验证 trait 对象可以创建
    }
}
