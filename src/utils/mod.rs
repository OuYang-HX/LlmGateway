/// SHA-256 hash utility for API key hashing
pub fn sha256_hash(input: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    Digest::update(&mut hasher, input.as_bytes());
    let result = Digest::finalize(hasher);
    hex::encode(result)
}
