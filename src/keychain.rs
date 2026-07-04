use anyhow::{Context, Result};
use std::process::Command;

/// 从 macOS Keychain 读取飞书凭证
///
/// 使用方式和 xhs-recipe 完全一致:
///   security find-generic-password -a "$USER" -s <SERVICE> -w
pub struct Keychain;

impl Keychain {
    /// 读取凭证：优先环境变量，降级到 macOS Keychain
    ///
    /// 环境变量名与 service 名一致（如 `FEISHU_APP_ID`），便于 Linux/Docker 等无 Keychain 环境。
    fn read_entry(service: &str) -> Result<String> {
        // 优先从环境变量读取
        if let Ok(val) = std::env::var(service) {
            let trimmed = val.trim().to_string();
            if !trimmed.is_empty() {
                return Ok(trimmed);
            }
        }

        let user = std::env::var("USER").unwrap_or_default();
        let output = Command::new("security")
            .args(["find-generic-password", "-a", &user, "-s", service, "-w"])
            .output()
            .with_context(|| format!("读取 Keychain 条目 {} 失败", service))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "读取 {} 失败: {}\n请运行:\n  \
                 security add-generic-password -a \"$USER\" -s {} -w \"<value>\" -U",
                service, stderr.trim(), service
            );
        }

        let value = String::from_utf8(output.stdout)
            .context("security 输出非 UTF-8")?
            .trim()
            .to_string();

        anyhow::ensure!(!value.is_empty(), "{} 的值为空，请检查 Keychain 配置", service);
        Ok(value)
    }

    pub fn get_app_id() -> Result<String> {
        Self::read_entry("FEISHU_APP_ID")
    }

    pub fn get_app_secret() -> Result<String> {
        Self::read_entry("FEISHU_APP_SECRET")
    }

    pub fn get_open_id() -> Result<String> {
        Self::read_entry("FEISHU_OPEN_ID")
    }
}
