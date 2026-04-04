pub fn to_array_string_u64(arr: Option<&[u64]>) -> String {
    let Some(arr) = arr else {
        return "NULL".to_string();
    };
    if arr.is_empty() {
        return "(0)".to_string();
    }
    let mut ret = format!("({}) {}", arr.len(), arr[0]);
    for v in &arr[1..] {
        ret.push(',');
        ret.push_str(&v.to_string());
    }
    ret
}

pub fn to_array_string_bytes(arr: Option<&[u8]>) -> String {
    let Some(arr) = arr else {
        return "NULL".to_string();
    };
    if arr.is_empty() {
        return "(0)".to_string();
    }
    let mut ret = format!("({}) {:02X}", arr.len(), arr[0]);
    for b in &arr[1..] {
        ret.push(',');
        ret.push_str(&format!("{:02X}", b));
    }
    ret
}

pub fn to_hex(arr: Option<&[u8]>) -> String {
    let Some(arr) = arr else {
        return "NULL".to_string();
    };
    let mut out = String::with_capacity(arr.len() * 2);
    for b in arr {
        out.push_str(&format!("{:02X}", b));
    }
    out
}

pub fn to_hex_n(arr: Option<&[u8]>) -> Option<String> {
    Some(to_hex(Some(arr?)))
}

pub fn to_hex_u32(v: Option<u32>) -> String {
    match v {
        None => "NULL".to_string(),
        Some(v) => format!("{:08X}", v),
    }
}

pub fn to_hex_u64(v: Option<u64>) -> String {
    match v {
        None => "NULL".to_string(),
        Some(v) => format!("{:08X}", v),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reporter_hex() {
        assert_eq!(to_hex(Some(&[0xAB, 0x00, 0x01])), "AB0001");
        assert_eq!(to_hex(None), "NULL");
        assert_eq!(to_hex_n(None), None);
        assert_eq!(to_hex_u32(Some(0x1234)), "00001234");
    }
}
