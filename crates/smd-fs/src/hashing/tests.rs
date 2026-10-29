use super::*;

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[test]
fn blake3_truncated_known_vectors() {
    let (b, more) = hash_reader(io::empty(), 10).unwrap();
    assert_eq!(hex(&b.hash), "af1349b9f5f9a1a6a0404dea36dcc949");
    assert_eq!(b.len, 0);
    assert!(!more);

    let data: Vec<u8> = (0..3 * 1024 * 1024).map(|i| (i % 251) as u8).collect();
    let (b, more) = hash_reader(&data[..], data.len() as u64).unwrap();
    assert!(!more);
    assert_eq!(b.len, data.len() as u64);
    assert_eq!(b.hash, blake3::hash(&data).as_bytes()[..16]);
}

#[test]
fn reports_growth_past_the_limit() {
    let (b, more) = hash_reader(&b"abcdef"[..], 3).unwrap();
    assert_eq!(b.len, 3);
    assert!(more);
}
