use anyhow::{Context, Result};
use std::process::Command;

/// 从 macOS Keychain 读取飞书凭证
pub struct Keychain;

impl Keychain {
    /// 读取 App ID 和 App Secret
    ///
    /// Keychain 存储方式:
    ///   名称: FEISHU_APP, 帐户: <App ID>, 密码: <App Secret>
    pub fn get_app_credentials() -> Result<(String, String)> {
        // 读取完整属性（含帐户名）
        let output = Command::new("security")
            .args(["find-generic-password", "-s", "FEISHU_APP"])
            .output()
            .context("执行 security 命令失败")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "读取 FEISHU_APP 失败: {}\n\
                 请运行: security add-generic-password -s FEISHU_APP \\\n  \
                   -a \"<你的 App ID>\" -w \"<你的 App Secret>\" -U",
                stderr.trim()
            );
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // 从属性中提取帐户名: "acct"<blob> = "cli_xxxx"
        let app_id = stdout
            .lines()
            .find_map(|line| {
                let line = line.trim();
                // 匹配 "acct"<blob>="cli_xxx" 或 "acct"<blob> = "cli_xxx"
                if let Some(start) = line.find("\"acct\"<blob>") {
                    let after = &line[start + 12..]; // 跳过 "acct"<blob>
                    let value = after.trim_start_matches('=').trim().trim_matches('"');
                    if !value.is_empty() {
                        return Some(value.to_string());
                    }
                }
                None
            })
            .unwrap_or_default();

        // 用 -w 单独拿密码
        let secret_output = Command::new("security")
            .args(["find-generic-password", "-s", "FEISHU_APP", "-w"])
            .output()
            .context("读取 FEISHU_APP 密码失败")?;

        let app_secret = String::from_utf8(secret_output.stdout)
            .context("security 输出非 UTF-8")?
            .trim()
            .to_string();

        anyhow::ensure!(!app_secret.is_empty(), "App Secret 为空");
        anyhow::ensure!(!app_id.is_empty(),
            "未找到 App ID，请确保 Keychain 中 FEISHU_APP 的帐户名是你的 App ID");

        Ok((app_id, app_secret))
    }

    /// 读取用户 open_id
    ///
    /// Keychain 存储方式:
    ///   名称: FEISHU_OPEN_ID, 密码: <open_id>
    pub fn get_open_id() -> Result<String> {
        let output = Command::new("security")
            .args(["find-generic-password", "-s", "FEISHU_OPEN_ID", "-w"])
            .output()
            .context("执行 security 命令失败")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "读取 FEISHU_OPEN_ID 失败: {}\n\
                 请运行: security add-generic-password -s FEISHU_OPEN_ID \\\n  \
                   -a \"<你的 open_id>\" -w \"<你的 open_id>\" -U",
                stderr.trim()
            );
        }

        let open_id = String::from_utf8(output.stdout)
            .context("security 输出非 UTF-8")?
            .trim()
            .to_string();
        Ok(open_id)
    }
}
