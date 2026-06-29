mod dir;
mod file;
mod handle;
mod path;

use crabport_sftp::CrabPortSftp;

pub(crate) use dir::{sftp_download_dir_impl, sftp_upload_dir_impl};
pub(crate) use file::{sftp_download_file_impl, sftp_upload_file_impl};
pub(crate) use handle::SftpTransferHandle;
use path::join_remote_path;

/// Download `remote_path` into `local_path`.
///
/// Dispatches based on what `remote_path` is:
///   - regular file → single-file gzip staging ([`sftp_download_file_impl`])
///   - directory → tar.gz staging with recursive SFTP fallback
///     ([`sftp_download_dir_impl`])
pub(crate) async fn sftp_download_impl(
    backend: &SftpTransferHandle,
    remote_path: &str,
    local_path: &str,
) -> anyhow::Result<()> {
    #[cfg(debug_assertions)]
    tracing::info!("SFTP download impl: remote={remote_path} local={local_path}");
    let (is_dir, original_size) = {
        let s = backend.take_or_open_sftp().await?;
        let meta_res = s.metadata(remote_path).await;
        let (is_dir, size) = match meta_res {
            Ok(m) => {
                let is_dir = m.file_type().is_dir();
                let size = m.size.unwrap_or(0);
                (is_dir, size)
            }
            Err(e) => {
                let msg = format!("remote stat failed: {e}");
                backend
                    .return_sftp(s, Err(anyhow::anyhow!(msg.clone())))
                    .await
                    .ok();
                return Err(anyhow::anyhow!(msg));
            }
        };
        backend.return_sftp(s, Ok(())).await?;
        (is_dir, size)
    };

    if is_dir {
        sftp_download_dir_impl(backend, remote_path, local_path).await
    } else {
        sftp_download_file_impl(backend, remote_path, local_path, original_size).await
    }
}

/// Upload `local_path` to `remote_path`.
///
/// Dispatches based on what `local_path` is:
///   - regular file → single-file gzip staging ([`sftp_upload_file_impl`])
///   - directory → tar.gz staging with recursive SFTP fallback
///     ([`sftp_upload_dir_impl`])
pub(crate) async fn sftp_upload_impl(
    backend: &SftpTransferHandle,
    local_path: &str,
    remote_path: &str,
) -> anyhow::Result<()> {
    let meta = std::fs::metadata(local_path)?;
    if meta.is_dir() {
        sftp_upload_dir_impl(backend, local_path, remote_path).await
    } else {
        sftp_upload_file_impl(backend, local_path, remote_path).await
    }
}

/// Delete a remote file or directory. Stats the path first to choose
/// `remove_file` vs `remove_dir` — SFTP's `remove_dir` only works on empty
/// directories, so for non-empty dirs we fall back to a recursive walk that
/// deletes contents depth-first.
pub(crate) async fn sftp_delete_impl(
    backend: &SftpTransferHandle,
    remote_path: &str,
) -> anyhow::Result<()> {
    #[cfg(debug_assertions)]
    tracing::info!("SFTP delete: remote={remote_path}");
    let s = backend.take_or_open_sftp().await?;
    let meta_res = s.metadata(remote_path).await;
    let is_dir = match &meta_res {
        Ok(m) => m.file_type().is_dir(),
        Err(e) => {
            let msg = format!("remote stat failed: {e}");
            backend
                .return_sftp(s, Err(anyhow::anyhow!(msg.clone())))
                .await
                .ok();
            return Err(anyhow::anyhow!(msg));
        }
    };
    backend.return_sftp(s, meta_res.map(|_| ())).await?;

    if !is_dir {
        let s = backend.take_or_open_sftp().await?;
        let res = s.remove_file(remote_path).await;
        backend.return_sftp(s, res).await?;
        return Ok(());
    }

    let s = backend.take_or_open_sftp().await?;
    let direct = s.remove_dir(remote_path).await;
    let direct_ok = direct.is_ok();
    backend.return_sftp(s, direct).await.ok();
    if direct_ok {
        return Ok(());
    }

    sftp_delete_dir_recursive(backend, remote_path).await
}

/// Recursively delete a non-empty remote directory: list entries, delete
/// each child (files directly, subdirs recursively), then remove the now-
/// empty directory itself. Depth-first so the final `remove_dir` succeeds.
async fn sftp_delete_dir_recursive(
    backend: &SftpTransferHandle,
    remote_path: &str,
) -> anyhow::Result<()> {
    let s = backend.take_or_open_sftp().await?;
    let entries_res = s.read_dir(remote_path).await;
    let entries = match entries_res {
        Ok(e) => {
            backend.return_sftp(s, Ok(())).await?;
            e
        }
        Err(e) => {
            let msg = format!("read_dir failed: {e}");
            backend.return_sftp(s, Err(e)).await.ok();
            return Err(anyhow::anyhow!(msg));
        }
    };

    for (name, is_dir) in entries {
        if name == "." || name == ".." {
            continue;
        }
        let child = join_remote_path(remote_path, &name);
        if is_dir {
            Box::pin(sftp_delete_dir_recursive(backend, &child)).await?;
        } else {
            let s = backend.take_or_open_sftp().await?;
            let res = s.remove_file(&child).await;
            backend.return_sftp(s, res).await?;
        }
    }

    // Now the directory should be empty — remove it.
    let s = backend.take_or_open_sftp().await?;
    let res = s.remove_dir(remote_path).await;
    backend.return_sftp(s, res).await?;
    Ok(())
}
