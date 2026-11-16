//! Steam roots and libraries.

use std::io::Read;
use std::path::{Path, PathBuf};

use smd_fs::{Origin, VDF_FILE_MAX, parse_library_folders};

use super::{Run, contains_dir, push_unique};
use crate::codes::{self, IngestDiagnostic};

/// Read a text file of at most [`VDF_FILE_MAX`] bytes (one more is read so that an
/// oversize file is noticed, not truncated). `Ok(None)` when it is not there.
pub(super) fn read_capped(path: &Path) -> Result<Option<String>, String> {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let mut text = String::new();
    file.take(VDF_FILE_MAX as u64 + 1).read_to_string(&mut text).map_err(|e| e.to_string())?;
    Ok(Some(text))
}

impl Run<'_> {
    /// Every existing Steam library: each existing Steam root, and every library its
    /// index files name. Every root and library tried is recorded.
    pub(crate) fn steam_libraries(&mut self) -> Vec<PathBuf> {
        let mut libs: Vec<PathBuf> = Vec::new();
        for (root, origin) in smd_fs::steam_roots(self.env) {
            let exists = self.try_dir("steam_root", root.clone(), origin);
            if !exists || contains_dir(&libs, &root) {
                continue;
            }
            push_unique(&mut libs, root.clone());
            for rel in self.profile.locations().steam_library_index.clone() {
                for lib in self.library_index(&root.join(rel)) {
                    let lib = PathBuf::from(lib);
                    if self.try_dir("steam_library", lib.clone(), Origin::Steam) {
                        push_unique(&mut libs, lib);
                    }
                }
            }
        }
        libs
    }

    fn library_index(&mut self, file: &Path) -> Vec<String> {
        let parsed = match read_capped(file) {
            Ok(None) => return Vec::new(),
            Ok(Some(text)) => parse_library_folders(&text).map_err(|e| e.to_string()),
            Err(e) => Err(e),
        };
        parsed.unwrap_or_else(|why| {
            let subject = file.display().to_string();
            self.diags.push(IngestDiagnostic::warn(codes::LOCATE_VDF_INVALID, &subject, why));
            Vec::new()
        })
    }
}
