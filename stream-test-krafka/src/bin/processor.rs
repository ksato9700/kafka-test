#[inline(always)]
fn read_varint(reader: &mut &[u8]) -> i64 {
    let mut val: u64 = 0;
    let mut shift = 0;
    loop {
        let b = reader[0];
        *reader = &(*reader)[1..];
        val |= ((b & 0x7F) as u64) << shift;
        if b & 0x80 == 0 { break; }
        shift += 7;
    }
    ((val >> 1) as i64) ^ -((val & 1) as i64)
}

#[inline(always)]
fn write_varint(buf: &mut Vec<u8>, n: i64) {
    let mut val = (n << 1) ^ (n >> 63);
    while val >= 0x80 {
        buf.push((val as u8) | 0x80);
        val >>= 7;
    }
    buf.push(val as u8);
}

fn main() {}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(n: i64) -> i64 {
        let mut buf = Vec::new();
        write_varint(&mut buf, n);
        let mut slice: &[u8] = &buf;
        read_varint(&mut slice)
    }

    #[test]
    fn test_zero() {
        assert_eq!(roundtrip(0), 0);
    }

    #[test]
    fn test_positive() {
        assert_eq!(roundtrip(1), 1);
        assert_eq!(roundtrip(100), 100);
        assert_eq!(roundtrip(127), 127);
        assert_eq!(roundtrip(128), 128);
        assert_eq!(roundtrip(300), 300);
    }

    #[test]
    fn test_negative() {
        assert_eq!(roundtrip(-1), -1);
        assert_eq!(roundtrip(-64), -64);
        assert_eq!(roundtrip(-1000), -1000);
    }

    #[test]
    fn test_multi_value_sequence() {
        let mut buf = Vec::new();
        write_varint(&mut buf, 3);   // count
        write_varint(&mut buf, 10);
        write_varint(&mut buf, 20);
        write_varint(&mut buf, 30);
        write_varint(&mut buf, 0);   // terminator

        let mut slice: &[u8] = &buf;
        let count = read_varint(&mut slice);
        assert_eq!(count, 3);
        let mut sum = 0i64;
        for _ in 0..count { sum += read_varint(&mut slice); }
        let terminator = read_varint(&mut slice);
        assert_eq!(sum, 60);
        assert_eq!(terminator, 0);
    }
}
