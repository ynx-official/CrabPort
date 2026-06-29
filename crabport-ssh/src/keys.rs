use russh::keys::key::KeyPair;

// ---------------------------------------------------------------------------
// Private key decoding
// ---------------------------------------------------------------------------

pub(crate) fn decode_private_key(
    key_str: &str,
    passphrase: Option<&str>,
) -> Result<KeyPair, Box<dyn std::error::Error + Send + Sync>> {
    // Try PEM-encoded key first (OpenSSH format: "-----BEGIN OPENSSH PRIVATE KEY-----")
    if key_str.contains("BEGIN") {
        let pair = russh::keys::decode_secret_key(key_str, passphrase)?;
        return Ok(pair);
    }

    // Otherwise treat as a raw key file path or content — try as file path first
    if let Ok(content) = std::fs::read_to_string(key_str) {
        let pair = russh::keys::decode_secret_key(&content, passphrase)?;
        return Ok(pair);
    }

    Err("cannot decode private key: not a valid PEM key or file path".into())
}
