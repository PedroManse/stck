use crate::*;
use std::collections::hash_map::{Entry, HashMap, OccupiedEntry};
use std::path::{Path, PathBuf};

pub trait FileCacher {
    type FileRecord<'s>: AsRef<str>
    where
        Self: 's;
    fn read_file(&mut self, path: impl AsRef<Path>)
    -> Result<Self::FileRecord<'_>, std::io::Error>;

    fn get_span(
        &mut self,
        path: impl AsRef<Path>,
        lines: &LineRange,
    ) -> Result<String, std::io::Error> {
        let entry = self.read_file(path)?;
        let lines: Vec<&str> = entry
            .as_ref()
            .split('\n')
            .skip(lines.start - 1)
            .take(lines.delta().max(1))
            .collect();
        Ok(lines.join("\n"))
    }
}

pub trait OverwriteCache {
    fn overwrite(&mut self, path: impl AsRef<Path>, content: String);
}

/// # Caching system for files
///
/// Used with [Line range](LineRange) to read specific lines from files on [get span](ErrorHelper::get_span)
#[derive(Default)]
pub struct CacheHelper {
    files: HashMap<PathBuf, String>,
}

impl CacheHelper {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

pub struct CachedFile<'s>(OccupiedEntry<'s, PathBuf, String>);

impl AsRef<str> for CachedFile<'_> {
    fn as_ref(&self) -> &str {
        self.0.get()
    }
}

impl OverwriteCache for CacheHelper {
    fn overwrite(&mut self, path: impl AsRef<Path>, content: String) {
        self.files.insert(path.as_ref().to_path_buf(), content);
    }
}

impl FileCacher for CacheHelper {
    type FileRecord<'s> = CachedFile<'s>;
    fn read_file(
        &mut self,
        path: impl AsRef<Path>,
    ) -> Result<Self::FileRecord<'_>, std::io::Error> {
        let entry = self.files.entry(path.as_ref().to_path_buf());
        let entry = match entry {
            Entry::Occupied(entry) => entry,
            Entry::Vacant(entry) => {
                let cont = std::fs::read_to_string(path)?;
                entry.insert_entry(cont)
            }
        };
        Ok(CachedFile(entry))
    }
}

pub struct NoCache;
impl FileCacher for NoCache {
    type FileRecord<'s> = String;
    fn read_file(
        &mut self,
        path: impl AsRef<Path>,
    ) -> Result<Self::FileRecord<'_>, std::io::Error> {
        std::fs::read_to_string(path)
    }
}

#[derive(Default)]
#[deprecated]
pub struct MockFileCacher(CacheHelper);

#[allow(deprecated)]
impl MockFileCacher {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn mock_file(&mut self, path: PathBuf, content: String) {
        self.0.overwrite(path, content);
    }
}

#[allow(deprecated)]
impl OverwriteCache for MockFileCacher {
    fn overwrite(&mut self, path: impl AsRef<Path>, content: String) {
        self.mock_file(path.as_ref().to_path_buf(), content);
    }
}

#[allow(deprecated)]
impl FileCacher for MockFileCacher {
    type FileRecord<'s> = CachedFile<'s>;
    fn read_file(
        &mut self,
        path: impl AsRef<Path>,
    ) -> Result<Self::FileRecord<'_>, std::io::Error> {
        self.0.read_file(path)
    }
}

#[derive(Default)]
pub struct Isolated {
    allowed: HashMap<PathBuf, String>,
}

impl Isolated {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add_file_cached(&mut self, path: PathBuf) -> Result<(), std::io::Error> {
        let entry = self.allowed.entry(path);
        if let Entry::Vacant(entry) = entry {
            let cont = std::fs::read_to_string(entry.key())?;
            entry.insert_entry(cont);
        }
        Ok(())
    }
    pub fn force_add_file(&mut self, path: PathBuf) -> Result<(), std::io::Error> {
        let cont = std::fs::read_to_string(&path)?;
        self.allowed.insert(path, cont);
        Ok(())
    }
}

impl OverwriteCache for Isolated {
    fn overwrite(&mut self, path: impl AsRef<Path>, content: String) {
        self.allowed.insert(path.as_ref().to_path_buf(), content);
    }
}

impl FileCacher for Isolated {
    type FileRecord<'s> = &'s String;
    fn read_file(
        &mut self,
        path: impl AsRef<Path>,
    ) -> Result<Self::FileRecord<'_>, std::io::Error> {
        self.allowed.get(path.as_ref()).ok_or(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Can't read file with Isolated cache system",
        ))
    }
}
