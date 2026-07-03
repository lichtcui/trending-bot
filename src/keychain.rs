use anyhow::{Context, Result};
use std::process::Command;

/// 从 macOS Keychain 读取飞书凭证
///
/// 使用 `security find-generic-password` 命令读取。
///
/// # 存储方式
/// 在 Keychain Access 中添加两个通用密码项：
///
/// 1. 名称: FEISHU_APP
///    - 帐户: <你的 App ID（如 cli_xxxx）>
///    - 密码: <你的 App Secret>
///
/// 2. 名称: FEISHU_OPEN_ID
///    - 帐户: <你的 open_id（如 ou_xxxx）>
///    - 密码: <你的 open_id>
pub struct Keychain;

impl Keychain {
    /// 读取 App ID 和 App Secret
    ///
    /// 返回 (app_id, app_secret)
    pub fn get_app_credentials() -> Result<(String, String)> {
        // 获取完整属性（含帐户名，即 App ID）
        let output = Command::new("security")
            .args(["find-generic-password", "-s", "FEISHU_APP", "-g"])
            .output()
            .context("执行 security 命令失败，请确保 macOS Keychain 中已配置 FEISHU_APP")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "读取 FEISHU_APP 失败: {}\n\
                 请打开 Keychain Access → 添加通用密码项:\n  \
                 名称: FEISHU_APP, 帐户: <你的 App ID>, 密码: <你的 App Secret>",
                stderr.trim()
            );
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        // -g 模式将密码和属性输出到 stderr，格式如:
        // "acct"<blob>="cli_xxxx"
        let app_id = stderr
            .lines()
            .find_map(|line| {
                let line = line.trim();
                // 匹配 "acct"<blob>="<value>"
                if let Some(val) = line.strip_prefix("\"acct\"<blob>=\"") {
                    val.trim_end_matches('"').to_string().into()
                } else if let Some(val) = line.strip_prefix("\"acct\"<blob>=") {
                    // 不带引号的格式: "acct"<blob>=cli_xxxx
                    Some(val.trim().to_string())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        // 用 -w 单独拿密码（只输出到 stdout）
        let secret_output = Command::new("security")
            .args(["find-generic-password", "-s", "FEISHU_APP", "-w"])
            .output()
            .context("读取 FEISHU_APP 密码失败")?;

        let app_secret = String::from_utf8(secret_output.stdout)
            .context("security 命令输出非 UTF-8")?
            .trim()
            .to_string();

        anyhow::ensure!(!app_id.is_empty(), "Keychain 中 FEISHU_APP 的帐户名为空");
        anyhow::ensure!(!app_secret.is_empty(), "Keychain 中 FEISHU_APP 的密码为空");

        Ok((app_id, app_secret))
    }

    /// 读取用户 open_id
    pub fn get_open_id() -> Result<String> {
        let output = Command::new("security")
            .args(["find-generic-password", "-s", "FEISHU_OPEN_ID", "-w"])
            .output()
            .context("执行 security 命令失败，请确保 macOS Keychain 中已配置 FEISHU_OPEN_ID")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "读取 FEISHU_OPEN_ID 失败: {}\n\
                 请打开 Keychain Access → 添加通用密码项:\n  \
                 名称: FEISHU_OPEN_ID, 帐户: <你的 open_id>, 密码: <你的 open_id>",
                stderr.trim()
            );
        }

        let open_id = String::from_utf8(output.stdout)
            .context("security 命令输出非 UTF-8")?
            .trim()
            .to_string();
        Ok(open_id)
    }
}
