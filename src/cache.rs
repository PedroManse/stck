//! # Caching systems
//!
//! It is common for complex programs to import a file more than once. Not only that, but
//! [`ErrCtx`] uses [`FileCacher::get_span`] to show the faulty lines in case of error, and
//! re-reading the content is necessary for this. so caching systems have been created to ease the
//! IO load.
//!
//! They have also been created to allow for better control over the operating system from the
//! host, since some caching systems need files to be alloed by the host instead of loaded at the
//! user's every whim.
//!
//! The simplest system is [`CacheHelper`], which caches all files and allow everything to be read.
//!
//! The most controlled system is [`Isolated`], which only allows previously-read files determined
//! by the host to be accessed.
//!
//! In case of testing, [`MockFileCacher`] is sugested, since you can easly mock the existence of
//! files with [mock_file](MockFileCacher::mock_file).

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
/// Used with [Line range](LineRange) to read specific lines from files on [get span](FileCacher::get_span)
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

/// # Mocked file system
///
/// Files can be mocked with [mock_file](MockFileCacher::mock_file)
///
/// If a file wasan't mocked, [`CacheHelper`] is used as a fallback
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

/// # The Isolated cache system
///
/// Only filed specified by [add_file_cached](Isolated::add_file_cached) or
/// (force_add_file)[Isolated::force_add_file] can be read by the user.
///
/// This is recomended in case of execution of untrusted code.
#[derive(Default)]
pub struct Isolated {
    allowed: HashMap<PathBuf, String>,
}

impl Isolated {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Read a file and cache it, if it doesn't already exist
    pub fn add_file_cached(&mut self, path: PathBuf) -> Result<(), std::io::Error> {
        let entry = self.allowed.entry(path);
        if let Entry::Vacant(entry) = entry {
            let cont = std::fs::read_to_string(entry.key())?;
            entry.insert_entry(cont);
        }
        Ok(())
    }
    /// Read a file and cache it
    ///
    /// May overwrite entry of file with same path.
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
