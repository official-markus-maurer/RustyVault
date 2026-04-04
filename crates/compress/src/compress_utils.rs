use std::path::Path;

pub fn set_thread_count(thread_count: Option<i32>) -> usize {
    let cpu = std::thread::available_parallelism()
        .map(|v| v.get() as i32)
        .unwrap_or(1);
    let fallback = std::cmp::max(cpu - 2, 1);
    match thread_count {
        None => fallback as usize,
        Some(v) if v <= 0 => fallback as usize,
        Some(v) => v as usize,
    }
}

pub fn create_dir_for_file(filename: &str) -> std::io::Result<()> {
    let path = Path::new(filename);
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    if parent.as_os_str().is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(parent)
}

pub fn compare_string(s1: &str, s2: &str) -> bool {
    s1.as_bytes() == s2.as_bytes()
}

pub fn compare_string_slash(s1: &str, s2: &str) -> bool {
    let b1 = s1.as_bytes();
    let b2 = s2.as_bytes();
    if b1.len() != b2.len() {
        return false;
    }
    for i in 0..b1.len() {
        let c1 = if b1[i] == b'/' { b'\\' } else { b1[i] };
        let c2 = if b2[i] == b'/' { b'\\' } else { b2[i] };
        if c1 != c2 {
            return false;
        }
    }
    true
}

pub fn byte_arr_compare(b0: &[u8], b1: &[u8]) -> bool {
    b0 == b1
}

pub fn uint_to_bytes(crc: Option<u32>) -> Option<[u8; 4]> {
    let c = crc?;
    Some([(c >> 24) as u8, (c >> 16) as u8, (c >> 8) as u8, c as u8])
}

pub fn bytes_to_uint(crc: Option<&[u8]>) -> Option<u32> {
    let b = crc?;
    if b.len() != 4 {
        return None;
    }
    Some(((b[0] as u32) << 24) | ((b[1] as u32) << 16) | ((b[2] as u32) << 8) | (b[3] as u32))
}

pub fn utc_ticks_to_dos_date_time(ticks: i64) -> (u16, u16) {
    if ticks <= 0xFFFF_FFFF {
        let dos_file_date = ((ticks >> 16) & 0xFFFF) as u16;
        let dos_file_time = (ticks & 0xFFFF) as u16;
        return (dos_file_date, dos_file_time);
    }

    let (year, month, day, hour, minute, second) = ticks_to_ymdhms(ticks);
    let dos_file_date =
        ((day & 0x1F) | ((month & 0x0F) << 5) | (((year - 1980) & 0x7F) << 9)) as u16;
    let dos_file_time =
        (((second >> 1) & 0x1F) | ((minute & 0x3F) << 5) | ((hour & 0x1F) << 11)) as u16;
    (dos_file_date, dos_file_time)
}

pub fn zip_date_time_to_string(zip_file_date_time: Option<i64>) -> String {
    let Some(t) = zip_file_date_time else {
        return String::new();
    };
    if t == 0 || t == i64::MIN {
        return String::new();
    }

    if t > 0xFFFF_FFFF {
        let (year, month, day, hour, minute, second) = ticks_to_ymdhms(t);
        if !(1..=9999).contains(&year) {
            return String::new();
        }
        return format!(
            "{:04}/{:02}/{:02} {:02}:{:02}:{:02}",
            year, month, day, hour, minute, second
        );
    }

    let dos_file_date = ((t >> 16) & 0xFFFF) as u16;
    let dos_file_time = (t & 0xFFFF) as u16;

    let second = ((dos_file_time & 0x1F) as i32) << 1;
    let minute = ((dos_file_time >> 5) & 0x3F) as i32;
    let hour = ((dos_file_time >> 11) & 0x1F) as i32;

    let day = (dos_file_date & 0x1F) as i32;
    let month = ((dos_file_date >> 5) & 0x0F) as i32;
    let year = (((dos_file_date >> 9) & 0x7F) as i32) + 1980;

    format!(
        "{:04}/{:02}/{:02} {:02}:{:02}:{:02}",
        year, month, day, hour, minute, second
    )
}

pub fn combine_dos_date_time(dos_file_date: u16, dos_file_time: u16) -> i64 {
    ((dos_file_date as i64) << 16) | (dos_file_time as i64)
}

pub fn utc_ticks_to_ntfs_date_time(ticks: i64) -> i64 {
    const FILE_TIME_TO_UTC_TIME: i64 = 504_911_232_000_000_000;
    ticks.saturating_sub(FILE_TIME_TO_UTC_TIME)
}

pub fn utc_ticks_from_ntfs_date_time(ntfs_ticks: i64) -> i64 {
    const FILE_TIME_TO_UTC_TIME: i64 = 504_911_232_000_000_000;
    ntfs_ticks.saturating_add(FILE_TIME_TO_UTC_TIME)
}

pub fn utc_ticks_to_unix_date_time(ticks: i64) -> i32 {
    const EPOCH_TIME_TO_UTC_TIME: i64 = 621_355_968_000_000_000;
    const TICKS_PER_SECOND: i64 = 10_000_000;
    ((ticks.saturating_sub(EPOCH_TIME_TO_UTC_TIME)) / TICKS_PER_SECOND) as i32
}

pub fn utc_ticks_from_unix_date_time(linux_seconds: i32) -> i64 {
    const EPOCH_TIME_TO_UTC_TIME: i64 = 621_355_968_000_000_000;
    const TICKS_PER_SECOND: i64 = 10_000_000;
    EPOCH_TIME_TO_UTC_TIME.saturating_add((linux_seconds as i64).saturating_mul(TICKS_PER_SECOND))
}

fn ticks_to_ymdhms(ticks: i64) -> (i32, i32, i32, i32, i32, i32) {
    const TICKS_PER_SECOND: i64 = 10_000_000;
    const TICKS_PER_MINUTE: i64 = TICKS_PER_SECOND * 60;
    const TICKS_PER_HOUR: i64 = TICKS_PER_MINUTE * 60;
    const TICKS_PER_DAY: i64 = TICKS_PER_HOUR * 24;

    let days = ticks.div_euclid(TICKS_PER_DAY);
    let mut rem = ticks.rem_euclid(TICKS_PER_DAY);
    let hour = (rem / TICKS_PER_HOUR) as i32;
    rem %= TICKS_PER_HOUR;
    let minute = (rem / TICKS_PER_MINUTE) as i32;
    rem %= TICKS_PER_MINUTE;
    let second = (rem / TICKS_PER_SECOND) as i32;

    let (year, month, day) = days_to_ymd(days);
    (year, month, day, hour, minute, second)
}

fn days_to_ymd(days: i64) -> (i32, i32, i32) {
    let mut n = days;
    let mut year = 1i32;
    let era = n.div_euclid(146097);
    n = n.rem_euclid(146097);
    year += (era * 400) as i32;

    let mut y = (n - n / 1460 + n / 36524 - n / 146096) / 365;
    if y == 400 {
        y = 399;
    }
    year += y as i32;
    n -= y * 365 + y / 4 - y / 100;

    let leap = is_leap_year(year);
    let month_lengths = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1i32;
    for ml in month_lengths {
        if n < ml {
            break;
        }
        n -= ml;
        month += 1;
    }
    let day = (n + 1) as i32;
    (year, month, day)
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dos_roundtrip_pack() {
        let packed = combine_dos_date_time(0x4A37, 0x7B12);
        let (d, t) = utc_ticks_to_dos_date_time(packed);
        assert_eq!(d, 0x4A37);
        assert_eq!(t, 0x7B12);
    }

    #[test]
    fn unix_ticks_roundtrip_epoch() {
        assert_eq!(utc_ticks_from_unix_date_time(0), 621_355_968_000_000_000);
        assert_eq!(utc_ticks_to_unix_date_time(621_355_968_000_000_000), 0);
    }

    #[test]
    fn compare_slash() {
        assert!(compare_string_slash("a/b", "a\\b"));
        assert!(!compare_string_slash("a/b", "a\\c"));
    }
}
