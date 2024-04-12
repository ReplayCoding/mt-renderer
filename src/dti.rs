include!(concat!(env!("OUT_DIR"), "/dti_generated.rs"));

pub fn from_hash(hash: u32) -> Option<&'static str> {
    DTI_MAP.get(&hash).copied()
}

#[test]
fn test_from_hash() {
    assert_eq!("bitset_prop<32>", from_hash(0x5d5af4f2).unwrap());
}

#[test]
fn test_dti_hashes() {
    use crate::crc32;

    // Make sure the hashes match up with the names
    for (hash, name) in DTI_MAP.entries() {
        let hash_computed = crc32(name.as_bytes(), u32::MAX) & 0x7fffffff;
        assert_eq!(
            *hash, hash_computed,
            "'{}' {:08x} != {:08x}",
            name, hash, hash_computed
        )
    }
}
