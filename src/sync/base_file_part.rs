
use std::io::{self,BufWriter};
use std::path::PathBuf;
use std::fs::File;

use anyhow::Result;

use super::{FileSystemIO,FileMetaData};

/// BaseFilePart contains method for adding File bytes to a base path.
pub struct BaseFilePart<F: FileSystemIO> {
    path_handle: BufWriter<File>,
    file_data: FileMetaData,
    fs: F,
}

impl<F: FileSystemIO> BaseFilePart<F> {
    pub fn new(path: PathBuf, completed_bytes: usize, fs: F) -> Result<BaseFilePart<F>> {
        let file_handle = fs.open_file(&path)?;
        let self_buf_writer = std::io::BufWriter::new(file_handle);

        Ok(BaseFilePart {
            path_handle: self_buf_writer,
            file_data: FileMetaData {
                completed_bytes: completed_bytes,
                path: path,
            },
            fs,
        })
    }

    pub fn add(&mut self, rhs: &FileMetaData) -> Result<()> {
        let mut rhs_handle = self.fs.open_file(&rhs.path)?;
        io::copy(&mut rhs_handle, &mut self.path_handle)?;
        self.file_data.completed_bytes += rhs.completed_bytes;
        self.fs.remove_file(&rhs.path)?;
        Ok(())
    }
}
