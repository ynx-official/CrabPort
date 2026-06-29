use anyhow::Result;

/// Build a `.tar.gz` of `local_dir` at `out_path`.
///
/// The archive's top-level entry is named `archive_name`, so unpacking with
/// `tar xzf ... -C <dst>` yields `<dst>/<archive_name>/...` rather than
/// flattening the contents into `<dst>/...`.
pub(crate) fn build_tar_gz(
    local_dir: &str,
    archive_name: &str,
    out_path: &std::path::Path,
) -> Result<()> {
    let file = std::fs::File::create(out_path)?;
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    builder.append_dir_all(archive_name, local_dir)?;
    // Finish the tar header stream, then flush the gzip encoder.
    let encoder = builder.into_inner()?;
    encoder.finish()?;
    Ok(())
}

/// Build a unique local tmp path with the given suffix.
///
/// Uses a `crabport-` prefix and a per-process counter + timestamp so two
/// concurrent transfers don't collide. Lives under `std::env::temp_dir()`.
pub(crate) fn local_tmp_path(suffix: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let token = nanos ^ ((pid as u64) << 32) ^ (n << 16);
    std::env::temp_dir().join(format!("crabport-{token:016x}{suffix}"))
}

/// Validate that `path` is a complete, decodable gzip stream. Catches the
/// common SFTP-corruption case where the downloaded bytes aren't actually
/// gzip at all (e.g. a seek/read offset bug returns wrong data).
///
/// Reads the whole file through a `GzDecoder` and checks the CRC + size
/// trailer — if either mismatches, the stream was truncated or corrupt.
pub(crate) fn validate_gzip(path: &std::path::Path) -> Result<()> {
    let file = std::fs::File::open(path)?;
    let mut decoder = flate2::read::GzDecoder::new(std::io::BufReader::new(file));
    // Drain the decoder fully — `GzDecoder` verifies the CRC32 and isize
    // trailer when it reaches EOF, returning a `CorruptGzipStream` error
    // if they don't match.
    let mut sink = std::io::sink();
    std::io::copy(&mut decoder, &mut sink)?;
    Ok(())
}
