use anyhow::Result;
use reqwest::blocking::Client;

use crate::item::TrendingItem;

/// 获取 This Week in Rust 的最新一期，展开 HTML 内链为独立 TrendingItem
pub fn fetch_rust_weekly(_client: &Client, _count: usize) -> Result<Vec<TrendingItem>> {
    todo!()
}

/// 获取 ByteByteGo Newsletter 最新文章
pub fn fetch_bytebytego(_client: &Client, _count: usize) -> Result<Vec<TrendingItem>> {
    todo!()
}

/// 获取 AI Weekly 最新文章
pub fn fetch_ai_weekly(_client: &Client, _count: usize) -> Result<Vec<TrendingItem>> {
    todo!()
}

#[cfg(test)]
mod tests {
    // 后续任务添加测试
}
