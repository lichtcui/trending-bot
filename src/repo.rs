use serde::Serialize;

/// 一个 GitHub Trending 项目
#[derive(Debug, Clone, Serialize)]
pub struct Repo {
    pub name: String,           // "owner/repo"
    pub url: String,            // "https://github.com/owner/repo"
    pub description: Option<String>,
    pub language: Option<String>,
    pub stars_total: u64,       // 总 Star 数
    pub stars_today: u64,       // 今日新增 Star
}

/// 解析 GitHub 的 Star 数字符串
/// 支持格式: "1,234" → 1234, "12.3k" → 12300, "5.2m" → 5200000
pub fn parse_star_count(s: &str) -> Option<u64> {
    let s = s.trim().replace(',', "");
    if s.is_empty() {
        return None;
    }
    let lower = s.to_lowercase();
    if lower.ends_with('k') {
        let num: f64 = lower[..lower.len() - 1].trim().parse().ok()?;
        Some((num * 1000.0) as u64)
    } else if lower.ends_with('m') {
        let num: f64 = lower[..lower.len() - 1].trim().parse().ok()?;
        Some((num * 1_000_000.0) as u64)
    } else if lower.ends_with('b') {
        let num: f64 = lower[..lower.len() - 1].trim().parse().ok()?;
        Some((num * 1_000_000_000.0) as u64)
    } else {
        lower.parse::<u64>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_plain_number() {
        assert_eq!(parse_star_count("1,234"), Some(1234));
        assert_eq!(parse_star_count("0"), Some(0));
        assert_eq!(parse_star_count("99999"), Some(99999));
    }

    #[test]
    fn test_parse_k_suffix() {
        assert_eq!(parse_star_count("12.3k"), Some(12300));
        assert_eq!(parse_star_count("1k"), Some(1000));
        assert_eq!(parse_star_count("0.5k"), Some(500));
        assert_eq!(parse_star_count("100k"), Some(100000));
    }

    #[test]
    fn test_parse_m_suffix() {
        assert_eq!(parse_star_count("1.5m"), Some(1_500_000));
        assert_eq!(parse_star_count("5.2m"), Some(5_200_000));
    }

    #[test]
    fn test_parse_empty_string() {
        assert_eq!(parse_star_count(""), None);
        assert_eq!(parse_star_count("   "), None);
    }

    #[test]
    fn test_parse_whitespace() {
        assert_eq!(parse_star_count("  1,234 "), Some(1234));
    }
}
