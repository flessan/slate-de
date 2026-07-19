//! Percent-encoding (used by the vision framework to build provider URLs).

const HEX: &[u8; 16] = b"0123456789ABCDEF";

/// Encode `s` preserving RFC 3986 unreserved characters.
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push(HEX[(b >> 4) as usize] as char);
                out.push(HEX[(b & 0xF) as usize] as char);
            }
        }
    }
    out
}

/// Encode a host+path-safe query value (`?` and `/` become encoded too).
pub fn encode_query_value(s: &str) -> String {
    percent_encode(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_specials() {
        assert_eq!(percent_encode("a b"), "a%20b");
        assert_eq!(percent_encode("https://x/y?q=1"), "https%3A%2F%2Fx%2Fy%3Fq%3D1");
        assert_eq!(percent_encode("plain-OK_1.0~"), "plain-OK_1.0~");
        assert_eq!(percent_encode("ü"), "%C3%BC"); // multi-byte UTF-8
    }
}
