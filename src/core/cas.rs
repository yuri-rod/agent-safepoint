use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

pub struct ObjectStore {
    root: PathBuf,
}

impl ObjectStore {
    pub fn new(safepoint_dir: &Path) -> io::Result<Self> {
        let root = safepoint_dir.join("objects");
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn object_path(&self, hash: &str) -> PathBuf {
        if hash.len() < 4 {
            return self.root.join(hash);
        }
        let (prefix, rest) = hash.split_at(2);
        self.root.join(prefix).join(rest)
    }

    pub fn exists(&self, hash: &str) -> bool {
        self.object_path(hash).is_file()
    }

    pub fn write_blob(&self, data: &[u8]) -> io::Result<String> {
        let hash = format!("{:x}", Sha256::digest(data));
        let path = self.object_path(&hash);

        if path.exists() {
            return Ok(hash);
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let tmp_path = path.with_extension(format!("tmp.{}", std::process::id()));
        {
            let mut file = File::create(&tmp_path)?;
            file.write_all(data)?;
            file.flush()?;
        }

        fs::rename(tmp_path, &path)?;
        Ok(hash)
    }

    pub fn write_file(&self, src_path: &Path) -> io::Result<(String, u64)> {
        let mut file = File::open(src_path)?;
        let mut hasher = Sha256::new();
        let mut buf = [0u8; 64 * 1024];
        let mut total_bytes = 0u64;

        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            total_bytes += n as u64;
        }

        let hash = format!("{:x}", hasher.finalize());
        let dest_path = self.object_path(&hash);

        if !dest_path.exists() {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let tmp_path = dest_path.with_extension(format!("tmp.{}", std::process::id()));
            fs::copy(src_path, &tmp_path)?;
            fs::rename(tmp_path, &dest_path)?;
        }

        Ok((hash, total_bytes))
    }

    pub fn read_blob(&self, hash: &str) -> io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        self.copy_verified(hash, &mut bytes)?;
        Ok(bytes)
    }

    pub fn verify_blob(&self, hash: &str) -> io::Result<()> {
        self.copy_verified(hash, &mut io::sink())
    }

    pub fn copy_to_file(&self, hash: &str, dest_path: &Path) -> io::Result<()> {
        let parent = dest_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent)?;
        let mut temporary = NamedTempFile::new_in(parent)?;
        self.copy_verified(hash, temporary.as_file_mut())?;
        temporary.as_file().sync_all()?;
        temporary.persist(dest_path).map_err(|err| err.error)?;
        Ok(())
    }

    fn copy_verified(&self, hash: &str, output: &mut impl Write) -> io::Result<()> {
        if hash.len() != 64
            || !hash
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid SHA-256 object ID",
            ));
        }
        let path = self.object_path(hash);
        if !fs::symlink_metadata(&path)?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Object {hash} is not a regular file"),
            ));
        }
        let mut source = File::open(path)?;
        let mut digest = Sha256::new();
        let mut buffer = [0; 64 * 1024];
        loop {
            let count = source.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            digest.update(&buffer[..count]);
            output.write_all(&buffer[..count])?;
        }
        if format!("{:x}", digest.finalize()) != hash {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Object {hash} failed SHA-256 verification"),
            ));
        }
        Ok(())
    }
}
