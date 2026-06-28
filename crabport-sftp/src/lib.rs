use anyhow::Result;
use russh::client::Handle;
use russh_sftp::client::SftpSession;

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// SFTP operations over an existing SSH connection.
#[allow(async_fn_in_trait)]
pub trait CrabPortSftp: Send + Sync {
    /// Upload a file: create or overwrite `remote_path` with `data`.
    async fn write_file(&self, remote_path: &str, data: &[u8]) -> Result<()>;

    /// Download a file: read the entire contents of `remote_path`.
    async fn read_file(&self, remote_path: &str) -> Result<Vec<u8>>;

    /// Delete a remote file.
    async fn remove_file(&self, remote_path: &str) -> Result<()>;

    /// List directory entries. Returns a vec of (name, is_dir).
    async fn read_dir(&self, remote_path: &str) -> Result<Vec<(String, bool)>>;

    /// Create a directory on the remote host.
    async fn create_dir(&self, remote_path: &str) -> Result<()>;

    /// Remove a directory on the remote host.
    async fn remove_dir(&self, remote_path: &str) -> Result<()>;

    /// Check if a file or directory exists.
    async fn exists(&self, remote_path: &str) -> Result<bool>;

    /// Canonicalize (resolve) a path on the remote host.
    async fn canonicalize(&self, remote_path: &str) -> Result<String>;

    /// Close the SFTP session.
    async fn close(&self) -> Result<()>;
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

/// SFTP backend that runs over an existing SSH `client::Handle`.
///
/// Generic over the handler type so it works with any russh client
/// implementation (e.g. `crabport-ssh::backend::SshHandler`).
pub struct SftpBackend {
    session: SftpSession,
}

impl SftpBackend {
    /// Open an SFTP subsystem channel on an existing SSH connection.
    ///
    /// The `handle` is the russh `client::Handle` obtained after
    /// authenticating. This function requests the "sftp" subsystem,
    /// then initialises the SFTP protocol on that channel.
    pub async fn connect<H>(handle: &Handle<H>) -> Result<Self>
    where
        H: russh::client::Handler + Send,
    {
        let channel = handle.channel_open_session().await?;
        channel.request_subsystem(true, "sftp").await?;

        let sftp = SftpSession::new(channel.into_stream()).await?;
        Ok(Self { session: sftp })
    }
}

impl CrabPortSftp for SftpBackend {
    async fn write_file(&self, remote_path: &str, data: &[u8]) -> Result<()> {
        self.session.write(remote_path, data).await?;
        Ok(())
    }

    async fn read_file(&self, remote_path: &str) -> Result<Vec<u8>> {
        let data = self.session.read(remote_path).await?;
        Ok(data)
    }

    async fn remove_file(&self, remote_path: &str) -> Result<()> {
        self.session.remove_file(remote_path).await?;
        Ok(())
    }

    async fn read_dir(&self, remote_path: &str) -> Result<Vec<(String, bool)>> {
        let entries = self.session.read_dir(remote_path).await?;
        let mut result = Vec::new();
        for entry in entries {
            let name = entry.file_name();
            let is_dir = entry.file_type().is_dir();
            result.push((name, is_dir));
        }
        Ok(result)
    }

    async fn create_dir(&self, remote_path: &str) -> Result<()> {
        self.session.create_dir(remote_path).await?;
        Ok(())
    }

    async fn remove_dir(&self, remote_path: &str) -> Result<()> {
        self.session.remove_dir(remote_path).await?;
        Ok(())
    }

    async fn exists(&self, remote_path: &str) -> Result<bool> {
        let exists = self.session.try_exists(remote_path).await?;
        Ok(exists)
    }

    async fn canonicalize(&self, remote_path: &str) -> Result<String> {
        let resolved = self.session.canonicalize(remote_path).await?;
        Ok(resolved)
    }

    async fn close(&self) -> Result<()> {
        self.session.close().await?;
        Ok(())
    }
}
