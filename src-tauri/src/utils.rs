//! 小工具函数。

/// 缺陷等级名称（0~3）
pub fn level_name(level: u8) -> &'static str {
    match level {
        1 => "一般",
        2 => "重大",
        3 => "紧急",
        _ => "其他",
    }
}

/// 标准 base64 编码（无第三方依赖，温度矩阵 IPC 使用）
pub fn to_base64(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_names() {
        assert_eq!(level_name(0), "其他");
        assert_eq!(level_name(1), "一般");
        assert_eq!(level_name(2), "重大");
        assert_eq!(level_name(3), "紧急");
        assert_eq!(level_name(4), "其他");
        assert_eq!(level_name(u8::MAX), "其他");
    }

    #[test]
    fn base64_encoding() {
        assert_eq!(to_base64(b""), "");
        assert_eq!(to_base64(b"f"), "Zg==");
        assert_eq!(to_base64(b"fo"), "Zm8=");
        assert_eq!(to_base64(b"foo"), "Zm9v");
        assert_eq!(to_base64(b"foobar"), "Zm9vYmFy");
        // 二进制字节与填充边界
        assert_eq!(to_base64(&[0, 0, 0]), "AAAA");
        assert_eq!(to_base64(&[255, 255, 255]), "////");
    }
}
