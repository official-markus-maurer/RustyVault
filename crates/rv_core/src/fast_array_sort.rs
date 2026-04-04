pub fn sort_with_filter<T: Clone>(
    input: &[T],
    find: impl Fn(&T) -> bool,
    sort: impl Fn(&T, &T) -> i32,
) -> Vec<T> {
    let mut out: Vec<T> = input.iter().filter(|v| find(v)).cloned().collect();
    out.sort_by(|a, b| sort(a, b).cmp(&0));
    out
}

pub fn sort_array<T: Clone>(input: &[T], sort: impl Fn(&T, &T) -> i32) -> Vec<T> {
    let mut out: Vec<T> = input.to_vec();
    out.sort_by(|a, b| sort(a, b).cmp(&0));
    out
}
