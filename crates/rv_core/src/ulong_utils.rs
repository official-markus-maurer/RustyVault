pub fn i_compare(a: Option<u64>, b: Option<u64>) -> i32 {
    match (a, b) {
        (Some(a), Some(b)) => (a.cmp(&b) as i32).signum(),
        _ => -1,
    }
}

pub fn i_compare_null(v0: Option<u64>, v1: Option<u64>) -> i32 {
    match (v0, v1) {
        (None, None) => 0,
        (Some(_), None) => 1,
        (None, Some(_)) => -1,
        (Some(a), Some(b)) => a.cmp(&b) as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_null_semantics() {
        assert_eq!(i_compare_null(None, None), 0);
        assert_eq!(i_compare_null(Some(1), None), 1);
        assert_eq!(i_compare_null(None, Some(1)), -1);
        assert_eq!(i_compare_null(Some(1), Some(2)), -1);
    }
}

