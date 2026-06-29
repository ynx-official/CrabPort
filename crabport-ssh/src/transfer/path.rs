// ---------------------------------------------------------------------------
// Remote path helpers
// ---------------------------------------------------------------------------

/// Build a unique remote tmp path for a single transfer.
///
/// Uses a `crabport-` prefix and a 16-hex-digit token derived from the
/// current time + a per-process counter. The token only needs to be unique
/// among concurrent transfers from this process; the prefix keeps us from
/// colliding with other tools that might write to `/tmp`.
pub(crate) fn remote_tmp_path() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    // Mix in the PID + a coarse timestamp so two Crabport processes running
    // at the same time don't collide.
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let token = nanos ^ ((pid as u64) << 32) ^ (n << 16);
    format!("/tmp/crabport-{token:016x}.gz")
}

/// Quote a path for inclusion in a shell command on the remote.
///
/// Wraps the path in single quotes and escapes any embedded single quotes
/// via the standard `'\''` idiom. This is the only fully-safe way to embed
/// an arbitrary path in a POSIX shell command without relying on the remote
/// shell being bash.
pub(crate) fn shell_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// Split a remote POSIX path into `(parent, basename)`. The parent of a
/// top-level path like `/foo` is `/`.
pub(crate) fn split_parent_basename(path: &str) -> anyhow::Result<(String, String)> {
    let p = std::path::Path::new(path);
    let basename = p
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("remote path has no basename: {path}"))?
        .to_string_lossy()
        .into_owned();
    let parent = p
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "/".to_string());
    let parent = if parent.is_empty() {
        "/".to_string()
    } else {
        parent
    };
    Ok((parent, basename))
}

/// Join a remote directory path with an entry name, inserting a `/` separator
/// only when needed (i.e. not when the parent already ends with one).
pub(crate) fn join_remote_path(parent: &str, name: &str) -> String {
    if parent.ends_with('/') {
        format!("{parent}{name}")
    } else {
        format!("{parent}/{name}")
    }
}
