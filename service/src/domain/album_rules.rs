use chrono::NaiveDate;

#[derive(Debug, Clone)]
pub struct AlbumRuleMatch {
    pub rule_name: String,
    pub album_date: String,
    pub album_name: String,
}

#[derive(Debug, Clone)]
pub struct DirectoryRulePattern {
    pub key: String,
    pub delimiter: char,
}

pub fn parse_album_from_dir_name(dir_name: &str) -> Option<AlbumRuleMatch> {
    let defaults = vec![".".to_string(), "_".to_string(), "-".to_string()];
    let patterns = patterns_from_delimiters(&defaults);
    parse_album_from_dir_name_with_patterns(dir_name, &patterns)
}

pub fn parse_album_from_dir_name_with_patterns(
    dir_name: &str,
    patterns: &[DirectoryRulePattern],
) -> Option<AlbumRuleMatch> {
    for pattern in patterns {
        if let Some(matched) = parse_by_delimiter(&pattern.key, dir_name, pattern.delimiter) {
            return Some(matched);
        }
    }
    None
}

pub fn patterns_from_delimiters(delimiters: &[String]) -> Vec<DirectoryRulePattern> {
    let mut out = Vec::new();
    for delimiter in delimiters {
        let mut chars = delimiter.chars();
        let first = match chars.next() {
            Some(c) if chars.next().is_none() => c,
            _ => continue,
        };

        let key = match first {
            '.' => "date_dot".to_string(),
            '_' => "date_underscore".to_string(),
            '-' => "date_hyphen".to_string(),
            other => format!("date_delim_{}", other as u32),
        };

        out.push(DirectoryRulePattern {
            key,
            delimiter: first,
        });
    }
    out
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
