use std::collections::HashSet;

pub fn extract_scan_pattern(raw: &str) -> Option<&str> {
    let s = raw.trim();
    if s.len() < 7 {
        return None;
    }
    if s[..7].eq_ignore_ascii_case("ignore:") {
        Some(s[7..].trim())
    } else {
        None
    }
}

pub fn extract_db_pattern(raw: &str) -> Option<&str> {
    let s = raw.trim();
    if s.len() >= 7 && s[..7].eq_ignore_ascii_case("ignore:") {
        None
    } else {
        Some(s)
    }
}

pub fn matches_pattern(name: &str, pattern: &str) -> bool {
    let p = pattern.trim();
    if p.len() >= 6 && p[..6].eq_ignore_ascii_case("regex:") {
        let expr = p[6..].trim();
        if expr.is_empty() {
            return false;
        }
        let re = regex::RegexBuilder::new(expr)
            .case_insensitive(true)
            .build();
        return re.is_ok_and(|re| re.is_match(name));
    }

    if p.contains('*') || p.contains('?') {
        wildcard_match(p, name)
    } else {
        p == name
    }
}

pub struct PatternMatcher {
    regex: Vec<regex::Regex>,
    literal: HashSet<String>,
    wildcard: Vec<String>,
}

impl PatternMatcher {
    pub fn from_scan_ignore_patterns(patterns: &[String]) -> Self {
        let mut regex = Vec::new();
        let mut literal = HashSet::new();
        let mut wildcard = Vec::new();

        for raw in patterns {
            let Some(pat) = extract_scan_pattern(raw) else {
                continue;
            };
            let p = pat.trim();
            if p.is_empty() {
                continue;
            }
            if p.len() >= 6 && p[..6].eq_ignore_ascii_case("regex:") {
                let expr = p[6..].trim();
                if expr.is_empty() {
                    continue;
                }
                if let Ok(re) = regex::RegexBuilder::new(expr)
                    .case_insensitive(true)
                    .build()
                {
                    regex.push(re);
                }
                continue;
            }

            #[cfg(windows)]
            let p = p.to_ascii_lowercase();
            #[cfg(not(windows))]
            let p = p.to_string();

            if p.contains('*') || p.contains('?') {
                wildcard.push(p);
            } else {
                literal.insert(p);
            }
        }

        Self {
            regex,
            literal,
            wildcard,
        }
    }

    pub fn is_match(&self, name: &str) -> bool {
        for re in &self.regex {
            if re.is_match(name) {
                return true;
            }
        }

        if self.literal.is_empty() && self.wildcard.is_empty() {
            return false;
        }

        #[cfg(windows)]
        let n = name.to_ascii_lowercase();
        #[cfg(not(windows))]
        let n = name;

        #[cfg(windows)]
        let is_literal_match = self.literal.contains(n.as_str());
        #[cfg(not(windows))]
        let is_literal_match = self.literal.contains(n);
        if is_literal_match {
            return true;
        }

        for pat in &self.wildcard {
            #[cfg(windows)]
            let is_match = wildcard_match(pat, n.as_str());
            #[cfg(not(windows))]
            let is_match = wildcard_match(pat, n);
            if is_match {
                return true;
            }
        }

        false
    }
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
    let p = pattern.as_bytes();
    let t = text.as_bytes();
    let mut pi = 0usize;
    let mut ti = 0usize;
    let mut star_match_pi: Option<usize> = None;
    let mut star_match_ti = 0usize;

    while ti < t.len() {
        if pi < p.len() && (p[pi] == b'?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
            continue;
        }
        if pi < p.len() && p[pi] == b'*' {
            star_match_pi = Some(pi);
            star_match_ti = ti;
            pi += 1;
            continue;
        }
        if let Some(star_pi) = star_match_pi {
            pi = star_pi + 1;
            star_match_ti += 1;
            ti = star_match_ti;
            continue;
        }
        return false;
    }

    while pi < p.len() && p[pi] == b'*' {
        pi += 1;
    }
    pi == p.len()
}
