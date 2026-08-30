use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use sha2::{Digest, Sha256};

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
        let path = self.object_path(hash);
        if !path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Object {} not found in store", hash),
            ));
        }
        fs::read(path)
    }

    pub fn copy_to_file(&self, hash: &str, dest_path: &Path) -> io::Result<()> {
        let src_path = self.object_path(hash);
        if !src_path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Object {} not found in store", hash),
            ));
        }

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::copy(src_path, dest_path)?;
        Ok(())
    }
}
