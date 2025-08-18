//! # Base File Part
//!
//! This module defines the `BaseFilePart` struct, which is responsible for
//! managing and combining downloaded file parts into a single base file.

use std::fs::File;
use std::io::{self, BufWriter};
use std::path::{Path, PathBuf};

use anyhow::Result;
use log::info;

use super::{FileMetaData, FileSystemIO};

/// Manages the base file and combines downloaded parts into it.
///
/// `BaseFilePart` provides methods to create a new base file and append
/// content from other file parts to it. It also handles the cleanup of
/// temporary part files after they have been added.
pub struct BaseFilePart<F: FileSystemIO> {
    path:PathBuf,
    pub path_handle: BufWriter<File>,
    completed_bytes:usize,
    fs: F,
}

impl<F: FileSystemIO> BaseFilePart<F> {
    /// Creates a new `BaseFilePart` instance.
    ///
    /// Initializes a `BaseFilePart` with a given path, the number of bytes
    /// already completed (typically 0 for a new file), and a file system
    /// implementation.
    ///
    /// # Arguments
    ///
    /// * `dir` - The path to the base file directory.
    /// * `file_name` - The file name.
    /// * `completed_bytes` - The number of bytes already written to the base file.
    /// * `fs` - An implementation of the `FileSystemIO` trait.
    /// 
    ///
    /// # Returns
    ///
    /// A `Result` containing the `BaseFilePart` instance on success.
    pub fn new(dir: PathBuf,file_name:String, completed_bytes: usize, fs: F) -> Result<BaseFilePart<F>> {
        let base_file_path=Path::new(&dir).join(&file_name);
        let file_handle = fs.open_file(&base_file_path)?;
        let self_buf_writer = std::io::BufWriter::new(file_handle);

        Ok(BaseFilePart{
            path:base_file_path,
            path_handle: self_buf_writer,
            completed_bytes:completed_bytes,
            fs,
        })
    }

    /// Adds the content of another file part to the base file.
    ///
    /// This method reads the content from the `rhs` file part and appends it
    /// to the base file. After successful addition, the `rhs` file part is
    /// removed from the file system.
    ///
    /// # Arguments
    ///
    /// * `rhs` - A reference to the `FileMetaData` of the part to add.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or failure.
    pub fn add(&mut self, rhs: &FileMetaData) -> Result<()> {
        info!("Adding {:?} to base path {:?}",rhs.path,&self.path);
        let mut rhs_handle = self.fs.open_file_for_read(&rhs.path)?;
        io::copy(&mut rhs_handle, &mut self.path_handle)?;
        self.completed_bytes += rhs.completed_bytes;
        self.fs.remove_file(&rhs.path)?;
        Ok(())
    }
}
