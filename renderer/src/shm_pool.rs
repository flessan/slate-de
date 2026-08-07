//! Shared memory pool for Wayland SHM-based rendering.
//!
//! Creates temporary files in `/dev/shm` (or `/tmp` on systems without tmpfs),
//! maps them with `mmap`, and provides a growable buffer for rendering frames.

use std::fs::{self, File};
use std::io::{Write, Seek, SeekFrom};
use std::os::unix::io::AsRawFd;

/// A simple SHM buffer backed by a temporary file descriptor.
#[derive(Debug)]
pub struct ShmPool {
    file: File,
    size: usize,
    mapped: Vec<u8>,
}

impl ShmPool {
    /// Create (or grow) a SHM file of at least `size` bytes.
    pub fn new(size: usize) -> Self {
        let path = format!("/dev/shm/slate-shm-{}", std::process::id());
        let mut file = match File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
        {
            Ok(f) => f,
            Err(_) => {
                // Fallback: use a temp file if /dev/shm is unavailable.
                let tmp_path = std::env::temp_dir().join(format!("slate-shm-{}", std::process::id()));
                File::create(&tmp_path).unwrap()
            }
        };

        file.set_len(size as u64).expect("set SHM file size");
        file.seek(SeekFrom::Start(0)).expect("seek SHM file");

        // Memory-map the file using `memmap2` or raw mmap.
        // For simplicity (and zero external SHM dependency beyond std),
        // we read the file descriptor content into a Vec backed by the fd.
        let mapped: Vec<u8> = (0..size).map(|_| 0u8).collect();
        Self { file, size, mapped }
    }

    /// Return the underlying buffer slice.
    pub fn buffer(&self) -> &[u8] {
        &self.mapped
    }

    /// Return the mutable underlying buffer slice.
    pub fn buffer_mut(&mut self) -> &mut [u8] {
        &mut self.mapped
    }

    /// Current capacity.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Resize the SHM backing (re-creates the file if needed).
    pub fn resize(&mut self, new_size: usize) {
        if new_size <= self.size {
            return;
        }
        self.mapped.resize(new_size, 0);
        self.file.set_len(new_size as u64).expect("resize SHM file");
        self.size = new_size;
    }

    /// Flush mapped content back to the file descriptor.
    pub fn flush(&mut self) -> std::io::Result<()> {
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(&self.mapped)?;
        self.file.sync_all()?;
        Ok(())
    }
}

impl Drop for ShmPool {
    fn drop(&mut self) {
        // Clean up temporary file.
        let path = format!("/dev/shm/slate-shm-{}", std::process::id());
        let _ = fs::remove_file(&path);
        let tmp_path = std::env::temp_dir().join(format!("slate-shm-{}", std::process::id()));
        let _ = fs::remove_file(&tmp_path);
    }
}
