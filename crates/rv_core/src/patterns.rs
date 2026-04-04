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

