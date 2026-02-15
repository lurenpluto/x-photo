use chrono::NaiveDate;

#[derive(Debug, Clone)]
pub struct AlbumRuleMatch {
    pub rule_name: String,
    pub album_date: String,
    pub album_name: String,
}

pub trait AlbumRule {
    fn key(&self) -> &'static str;
    fn parse(&self, dir_name: &str) -> Option<AlbumRuleMatch>;
}

pub struct DotDateRule;
pub struct UnderscoreDateRule;
pub struct HyphenDateRule;

impl AlbumRule for DotDateRule {
    fn key(&self) -> &'static str {
        "date_dot"
    }

    fn parse(&self, dir_name: &str) -> Option<AlbumRuleMatch> {
        parse_by_delimiter(self.key(), dir_name, '.')
    }
}

impl AlbumRule for UnderscoreDateRule {
    fn key(&self) -> &'static str {
        "date_underscore"
    }

    fn parse(&self, dir_name: &str) -> Option<AlbumRuleMatch> {
        parse_by_delimiter(self.key(), dir_name, '_')
    }
}

impl AlbumRule for HyphenDateRule {
    fn key(&self) -> &'static str {
        "date_hyphen"
    }

    fn parse(&self, dir_name: &str) -> Option<AlbumRuleMatch> {
        parse_by_delimiter(self.key(), dir_name, '-')
    }
}

pub fn parse_album_from_dir_name(dir_name: &str) -> Option<AlbumRuleMatch> {
    let rules: [&dyn AlbumRule; 3] = [&DotDateRule, &UnderscoreDateRule, &HyphenDateRule];
    for rule in rules {
        if let Some(matched) = rule.parse(dir_name) {
            return Some(matched);
        }
    }
    None
}

fn parse_by_delimiter(rule_name: &str, dir_name: &str, delim: char) -> Option<AlbumRuleMatch> {
    let parts: Vec<&str> = dir_name.split(delim).collect();
    if parts.len() < 4 {
        return None;
    }

    let year: i32 = parts[0].parse().ok()?;
    let month: u32 = parts[1].parse().ok()?;
    let day: u32 = parts[2].parse().ok()?;
    let date = NaiveDate::from_ymd_opt(year, month, day)?;

    let album_name = parts[3..].join(&delim.to_string()).trim().to_string();
    if album_name.is_empty() {
        return None;
    }

    Some(AlbumRuleMatch {
        rule_name: rule_name.to_string(),
        album_date: date.format("%Y-%m-%d").to_string(),
        album_name,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_album_from_dir_name;

    #[test]
    fn parses_dot_mode() {
        let matched = parse_album_from_dir_name("2020.02.01.大沙河长廊.人才公园.流花山").unwrap();
        assert_eq!(matched.rule_name, "date_dot");
        assert_eq!(matched.album_date, "2020-02-01");
        assert_eq!(matched.album_name, "大沙河长廊.人才公园.流花山");
    }

    #[test]
    fn parses_underscore_mode() {
        let matched = parse_album_from_dir_name("2020_02_01_人才公园_流花山").unwrap();
        assert_eq!(matched.rule_name, "date_underscore");
        assert_eq!(matched.album_date, "2020-02-01");
        assert_eq!(matched.album_name, "人才公园_流花山");
    }

    #[test]
    fn parses_hyphen_mode() {
        let matched = parse_album_from_dir_name("2020-02-01-深圳湾公园").unwrap();
        assert_eq!(matched.rule_name, "date_hyphen");
        assert_eq!(matched.album_date, "2020-02-01");
        assert_eq!(matched.album_name, "深圳湾公园");
    }
}
