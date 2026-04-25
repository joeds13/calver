use anyhow::{anyhow, Result};
use chrono::Datelike;
use regex::Regex;
use std::fmt;

/// A AnnoVer version in the form `<year>.<int>` or `<year>.<int>-dev<int>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnoVer {
    pub year: u32,
    pub increment: u32,
    /// Present on non-main branch builds.
    pub dev: Option<u32>,
}

impl AnnoVer {
    pub fn new(year: u32, increment: u32, dev: Option<u32>) -> Self {
        Self {
            year,
            increment,
            dev,
        }
    }

    /// Parse a version string, accepting an optional leading `v`.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.strip_prefix('v').unwrap_or(s);
        let re = Regex::new(r"^(\d{4})\.(\d+)(?:-dev(\d+))?$").ok()?;
        let caps = re.captures(s)?;
        Some(Self {
            year: caps[1].parse().ok()?,
            increment: caps[2].parse().ok()?,
            dev: caps.get(3).and_then(|m| m.as_str().parse().ok()),
        })
    }

    pub fn parse_required(s: &str) -> Result<Self> {
        Self::parse(s).ok_or_else(|| anyhow!("invalid annover string: {s}"))
    }

    pub fn is_dev(&self) -> bool {
        self.dev.is_some()
    }

    /// Strip the dev suffix, returning the release base version.
    pub fn base(&self) -> Self {
        Self::new(self.year, self.increment, None)
    }

    pub fn current_year() -> u32 {
        chrono::Local::now().year() as u32
    }

    /// Next release version given the latest release tag on main.
    pub fn next_main(latest: Option<&AnnoVer>) -> Self {
        let year = Self::current_year();
        match latest {
            None => Self::new(year, 1, None),
            Some(v) if v.year < year => Self::new(year, 1, None),
            Some(v) => Self::new(year, v.increment + 1, None),
        }
    }

    /// Next dev version for a feature branch.
    ///
    /// `base` is what the next main release will be.
    /// `latest_dev` is the highest dev tag already existing for that base.
    pub fn next_dev(base: &AnnoVer, latest_dev: Option<&AnnoVer>) -> Self {
        let dev_n = latest_dev.and_then(|v| v.dev).map(|n| n + 1).unwrap_or(1);
        Self::new(base.year, base.increment, Some(dev_n))
    }
}

impl fmt::Display for AnnoVer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.dev {
            None => write!(f, "{}.{}", self.year, self.increment),
            Some(d) => write!(f, "{}.{}-dev{}", self.year, self.increment, d),
        }
    }
}

// release > any dev of same base; dev versions ordered by dev number
impl Ord for AnnoVer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering::*;
        self.year
            .cmp(&other.year)
            .then(self.increment.cmp(&other.increment))
            .then_with(|| match (self.dev, other.dev) {
                (None, None) => Equal,
                (None, Some(_)) => Greater,
                (Some(_), None) => Less,
                (Some(a), Some(b)) => a.cmp(&b),
            })
    }
}

impl PartialOrd for AnnoVer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_release() {
        let v = AnnoVer::parse("2026.3").unwrap();
        assert_eq!(v.year, 2026);
        assert_eq!(v.increment, 3);
        assert_eq!(v.dev, None);
    }

    #[test]
    fn parse_dev() {
        let v = AnnoVer::parse("2026.3-dev2").unwrap();
        assert_eq!(v.dev, Some(2));
    }

    #[test]
    fn parse_v_prefix() {
        assert!(AnnoVer::parse("v2026.3").is_some());
    }

    #[test]
    fn display() {
        assert_eq!(AnnoVer::new(2026, 3, None).to_string(), "2026.3");
        assert_eq!(AnnoVer::new(2026, 3, Some(1)).to_string(), "2026.3-dev1");
    }

    #[test]
    fn ordering() {
        let release = AnnoVer::new(2026, 4, None);
        let dev = AnnoVer::new(2026, 4, Some(99));
        assert!(release > dev);
    }

    #[test]
    fn next_main_new_year() {
        let latest = AnnoVer::new(2025, 10, None);
        let next = AnnoVer::next_main(Some(&latest));
        assert_eq!(next.year, AnnoVer::current_year());
        assert_eq!(next.increment, 1);
    }
}
