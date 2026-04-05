fn trrntzip_name_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    let len = std::cmp::min(ab.len(), bb.len());
    for i in 0..len {
        let ca = if ab[i].is_ascii_uppercase() { ab[i] + 0x20 } else { ab[i] };
        let cb = if bb[i].is_ascii_uppercase() { bb[i] + 0x20 } else { bb[i] };
        if ca < cb {
            return std::cmp::Ordering::Less;
        }
        if ca > cb {
            return std::cmp::Ordering::Greater;
        }
    }
    match ab.len().cmp(&bb.len()) {
        std::cmp::Ordering::Equal => {
            for i in 0..len {
                if ab[i] < bb[i] {
                    return std::cmp::Ordering::Less;
                }
                if ab[i] > bb[i] {
                    return std::cmp::Ordering::Greater;
                }
            }
            std::cmp::Ordering::Equal
        }
        other => other,
    }
}
