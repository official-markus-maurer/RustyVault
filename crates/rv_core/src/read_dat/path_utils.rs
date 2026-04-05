impl DatUpdate {
    fn normalize_dat_path(path: &str) -> String {
        path.replace('/', "\\").trim_matches('\\').to_string()
    }

    fn normalized_path_eq(left: &str, right: &str) -> bool {
        #[cfg(windows)]
        {
            left.eq_ignore_ascii_case(right)
        }
        #[cfg(not(windows))]
        {
            left == right
        }
    }

    fn normalized_path_has_prefix(full: &str, prefix: &str) -> bool {
        if Self::normalized_path_eq(full, prefix) {
            return true;
        }

        #[cfg(windows)]
        {
            let full_lower = full.to_ascii_lowercase();
            let prefix_lower = prefix.to_ascii_lowercase();
            full_lower
                .strip_prefix(&prefix_lower)
                .is_some_and(|suffix| suffix.starts_with('\\'))
        }
        #[cfg(not(windows))]
        {
            full.strip_prefix(prefix)
                .is_some_and(|suffix| suffix.starts_with('\\'))
        }
    }

    fn dat_path_matches_filter(dat_full_name: &str, dat_path: &str) -> bool {
        let normalized_full_name = Self::normalize_dat_path(dat_full_name);
        let normalized_filter = Self::normalize_dat_path(dat_path);

        if normalized_filter.is_empty() {
            return true;
        }
        Self::normalized_path_has_prefix(&normalized_full_name, &normalized_filter)
    }
}
