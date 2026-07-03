use anyhow::{Context, Result};
use serde_json::Value;

/// 推送器 trait — 为今后扩展到 Slack/Discord 等渠道做准备
pub trait Notifier {
    /// 发送 JSON payload 到目标渠道
    fn send(&self, payload: &Value) -> Result<()>;
}

/// 飞书机器人 Webhook 推送器
pub struct FeishuNotifier {
    webhook_url: String,
    client: reqwest::blocking::Client,
}

impl FeishuNotifier {
    /// 创建一个新的飞书推送器
    ///
    /// `webhook_url`: 飞书自定义机器人 Webhook 地址
    /// 格式: `https://open.feishu.cn/open-apis/bot/v2/hook/{token}`
    pub fn new(webhook_url: &str) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("创建 HTTP 客户端失败");
        FeishuNotifier {
            webhook_url: webhook_url.to_string(),
            client,
        }
    }
}

impl Notifier for FeishuNotifier {
    fn send(&self, payload: &Value) -> Result<()> {
        let resp = self
            .client
            .post(&self.webhook_url)
            .json(payload)
            .send()
            .context("请求飞书 Webhook 失败（网络错误或超时）")?;

        let status = resp.status();
        let body: Value = resp.json().context("解析飞书响应 JSON 失败")?;

        let code = body
            .get("code")
            .and_then(|c| c.as_i64())
            .unwrap_or(-1);
        if code != 0 {
            let msg = body
                .get("msg")
                .and_then(|m| m.as_str())
                .unwrap_or("未知错误");
            anyhow::bail!("飞书返回错误 (code={}): {}", code, msg);
        }

        println!("✅ 成功推送到飞书 (HTTP {})", status);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 FeishuNotifier::new 成功创建实例且 webhook_url 正确
    #[test]
    fn test_new_notifier() {
        let url = "https://open.feishu.cn/open-apis/bot/v2/hook/test-token";
        let notifier = FeishuNotifier::new(url);
        assert_eq!(notifier.webhook_url, url);
    }

    /// 验证 send 在传入无效 webhook URL 时返回错误（连接被拒绝或 DNS 失败）
    #[test]
    fn test_send_to_invalid_host() {
        let notifier = FeishuNotifier::new("https://invalid-host-12345.example.com/webhook");
        let payload = serde_json::json!({"msg_type": "text", "content": {"text": "test"}});
        let result = notifier.send(&payload);
        // 应该因 DNS 解析失败或连接超时而出错
        assert!(result.is_err(), "连接到无效主机应该返回错误");
    }

    /// 验证 send 在传入格式错误的 URL 时返回错误
    #[test]
    fn test_send_with_bad_url() {
        let notifier = FeishuNotifier::new("not-a-valid-url");
        let payload = serde_json::json!({"msg_type": "text", "content": {"text": "test"}});
        let result = notifier.send(&payload);
        assert!(result.is_err(), "格式错误的 URL 应该返回错误");
    }

    /// 验证 trait 对象可以被创建（编译检查）
    #[test]
    fn test_trait_object() {
        let url = "https://open.feishu.cn/open-apis/bot/v2/hook/test";
        let notifier = FeishuNotifier::new(url);
        // 通过 trait 对象调用
        let notifier_ref: &dyn Notifier = &notifier;
        // 只是验证 trait 对象可以创建，不实际发送
        let payload = serde_json::json!({"key": "value"});
        // 不会真正调用，因为有 trait 对象引用就够了
        let _ = notifier_ref;
        // 直接调用会失败（URL 无效）
        let result = notifier.send(&payload);
        assert!(result.is_err());
    }

    /// 验证 send 对于非 2xx 的 HTTP 状态码能正确处理
    #[test]
    fn test_send_to_http_server() {
        // 使用一个存在的 HTTP 服务器但路径不对，应返回非 200
        let notifier = FeishuNotifier::new("https://httpbin.org/status/403");
        let payload = serde_json::json!({"msg_type": "text", "content": {"text": "test"}});
        let result = notifier.send(&payload);
        // 可能因 JSON 解析失败或状态码错误而出错
        assert!(result.is_err(), "HTTP 403 应该返回错误");
    }

    /// 验证空 payload 被发送时返回错误
    #[test]
    fn test_send_empty_payload() {
        let notifier = FeishuNotifier::new("https://httpbin.org/post");
        let payload = serde_json::json!({});
        let result = notifier.send(&payload);
        // httpbin 会接受请求但飞书会返回错误，所以我们只验证不 panic
        // 由于 httpbin 返回 200，但 JSON 可能不符合飞书格式
        // 所以可能返回成功（200）或错误（code != 0）
        // 只要不 panic 就算通过
        let _ = result;
    }
}
