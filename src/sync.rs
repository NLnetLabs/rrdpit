use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::str::from_utf8_unchecked;
use std::{fmt, fs, io};

use bytes::Bytes;
use ring::digest;

//------------ RsyncUri -----------------------------------------------------

#[derive(Clone, Debug, Display, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[display(fmt = "{}", _0)]
pub struct RsyncUri(String);

impl RsyncUri {
    pub fn base_uri(s: &str) -> Result<Self, Error> {
        if s.starts_with("rsync://") && s.ends_with('/') {
            Ok(RsyncUri(s.to_string()))
        } else {
            Err(Error::InvalidRsyncBase)
        }
    }

    fn resolve(&self, s: &str) -> Self {
        RsyncUri(format!("{}{}", self.0, s))
    }
}

impl From<&str> for RsyncUri {
    fn from(s: &str) -> Self {
        RsyncUri(s.to_string())
    }
}

//------------ HttpsUri -----------------------------------------------------

#[derive(Clone, Debug, Display, Eq, Hash, PartialEq)]
#[display(fmt = "{}", _0)]
pub struct HttpsUri(String);

impl HttpsUri {
    pub fn base_uri(s: &str) -> Result<Self, Error> {
        if s.starts_with("https://") && s.ends_with('/') {
            Ok(HttpsUri(s.to_string()))
        } else {
            Err(Error::InvalidHttpsBase)
        }
    }

    pub fn resolve(&self, s: &str) -> Self {
        HttpsUri(format!("{}{}", self.0, s))
    }

    pub fn relative_to(&self, mut uri: String) -> Option<String> {
        if uri.starts_with(&self.0) {
            Some(uri.split_off(self.0.len()))
        } else {
            None
        }
    }
}

impl From<&str> for HttpsUri {
    fn from(s: &str) -> Self {
        HttpsUri(s.to_string())
    }
}

//------------ Base64 --------------------------------------------------------

/// This type contains a base64 encoded structure. The publication protocol
/// deals with objects in their base64 encoded form.
///
/// Note that we store this in a Bytes to make it cheap to clone this.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Base64(Bytes);

impl Base64 {
    pub fn from_content(content: &[u8]) -> Self {
        let base64 = base64::encode(content);
        Base64(Bytes::from(base64))
    }

    pub fn from_b64_str(s: &str) -> Self {
        Base64(Bytes::from(s))
    }
}

impl fmt::Display for Base64 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = unsafe { from_utf8_unchecked(self.0.as_ref()) };
        s.fmt(f)
    }
}

//------------ EncodedHash ---------------------------------------------------

/// This type contains a hex encoded sha256 hash.
///
/// Note that we store this in a Bytes for cheap cloning.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct EncodedHash(Bytes);

impl EncodedHash {
    pub fn from_content(content: &[u8]) -> Self {
        let sha256 = Self::sha256(content);
        let hex = hex::encode(sha256);
        EncodedHash(Bytes::from(hex))
    }

    pub fn sha256(object: &[u8]) -> Bytes {
        Bytes::from(digest::digest(&digest::SHA256, object).as_ref())
    }
}

impl fmt::Display for EncodedHash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = unsafe { from_utf8_unchecked(self.0.as_ref()) };
        s.fmt(f)
    }
}

//------------ CurrentFile ---------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentFile {
    /// The full uri for this file.
    uri: RsyncUri,

    /// The base64 encoded content of a file.
    base64: Base64,

    /// The hex encoded sha-256 hash of the file.
    hash: EncodedHash,
}

impl CurrentFile {
    pub fn new(uri: RsyncUri, content: &[u8]) -> Self {
        let base64 = Base64::from_content(content);
        let hash = EncodedHash::from_content(content);
        CurrentFile { uri, base64, hash }
    }

    pub fn uri(&self) -> &RsyncUri {
        &self.uri
    }
    pub fn base64(&self) -> &Base64 {
        &self.base64
    }
    pub fn hash(&self) -> &EncodedHash {
        &self.hash
    }
}

//------------ CurrentFile ---------------------------------------------------

/// Reads a file to Bytes
pub fn read(path: &PathBuf) -> Result<Bytes, io::Error> {
    let mut f = File::open(path).map_err(|_| Error::cannot_read(path))?;
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes)?;
    Ok(Bytes::from(bytes))
}

fn create_file_with_path(path: &PathBuf) -> Result<File, io::Error> {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
    }
    File::create(path)
}

/// Derive the path for this file.
pub fn file_path(base_path: &PathBuf, file_name: &str) -> PathBuf {
    let mut path = base_path.clone();
    path.push(file_name);
    path
}

/// Saves a file, creating parent dirs as needed
pub fn save(content: &[u8], full_path: &PathBuf) -> Result<(), io::Error> {
    let mut f = create_file_with_path(full_path)?;
    f.write_all(content)?;
    Ok(())
}

fn recurse_disk(
    base_path: &PathBuf,
    path: &PathBuf,
    rsync_base: &RsyncUri,
) -> Result<Vec<CurrentFile>, Error> {
    let mut res = Vec::new();

    for entry in fs::read_dir(path).map_err(|_| Error::cannot_read(path))? {
        let entry = entry.map_err(|_| Error::cannot_read(path))?;
        let path = entry.path();
        if entry
            .file_name()
            .to_str()
            .map(|name| name.starts_with('.'))
            .unwrap_or(true)
        {
            // this is a hidden file / directory (by convention) so skip it
        } else if path.is_dir() {
            let mut other = recurse_disk(base_path, &path, rsync_base)?;
            res.append(&mut other);
        } else {
            let uri = derive_uri(base_path, &path, rsync_base)?;
            let content = read(&path).map_err(|_| Error::cannot_read(&path))?;
            let current_file = CurrentFile::new(uri, &content);

            res.push(current_file);
        }
    }

    Ok(res)
}

fn derive_uri(
    base_path: &PathBuf,
    path: &PathBuf,
    rsync_base: &RsyncUri,
) -> Result<RsyncUri, Error> {
    let rel_path = derive_relative_path(base_path, path)?;
    Ok(rsync_base.resolve(&rel_path))
}

fn derive_relative_path(base_path: &PathBuf, path: &PathBuf) -> Result<String, Error> {
    let base_str = base_path.to_string_lossy().to_string();
    let mut path_str = path.to_string_lossy().to_string();

    if !path_str.starts_with(&base_str) {
        Err(Error::OutsideJail(path_str, base_str))
    } else {
        let base_len = base_str.len();
        let rel = path_str.split_off(base_len);
        Ok(rel)
    }
}

pub fn crawl_disk(base_path: &PathBuf, rsync_base: &RsyncUri) -> Result<Vec<CurrentFile>, Error> {
    recurse_disk(base_path, base_path, rsync_base)
}

/// Cleans up a directory, i.e. it retains any files and/or disks for which the
/// predicate function returns 'true'
pub fn retain_disk<P>(base_path: &PathBuf, keep: P) -> Result<(), Error>
where
    P: Copy + FnOnce(String) -> bool,
{
    for entry in fs::read_dir(base_path).map_err(|_| Error::cannot_read(base_path))? {
        let entry = entry.map_err(|_| Error::cannot_read(base_path))?;
        let rel = derive_relative_path(base_path, &entry.path())?;

        if !keep(rel) {
            let _res = fs::remove_dir_all(entry.path());
        }
    }

    Ok(())
}

//------------ Error ---------------------------------------------------------
#[derive(Debug, Display)]
pub enum Error {
    #[display(fmt = "Invalid rsync uri")]
    InvalidRsyncUri,

    #[display(fmt = "rsync base uri must start with rsync:// end with slash")]
    InvalidRsyncBase,

    #[display(fmt = "https base uri must start with https:// end with slash")]
    InvalidHttpsBase,

    #[display(fmt = "Cannot read: {}", _0)]
    CannotRead(String),

    #[display(fmt = "Unsupported characters: {}", _0)]
    UnsupportedFileName(String),

    #[display(fmt = "File: {} outside of jail: {}", _0, _1)]
    OutsideJail(String, String),
}

impl Error {
    fn cannot_read(path: &PathBuf) -> Error {
        let str = path.to_string_lossy().to_string();
        Error::CannotRead(str)
    }
}

impl std::error::Error for Error {}

impl From<Error> for io::Error {
    fn from(e: Error) -> Self {
        io::Error::new(io::ErrorKind::Other, e)
    }
}

//------------ Tests ---------------------------------------------------------
//
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_scan_disk() {
        let base_dir = PathBuf::from("./test-resources/");
        let rsync_base = RsyncUri::base_uri("rsync://localhost/repo/").unwrap();

        let files = crawl_disk(&base_dir, &rsync_base).unwrap();

        let expected = vec![
            "rsync://localhost/repo/source-1/file1.txt",
            "rsync://localhost/repo/source-1/file2.txt",
            "rsync://localhost/repo/source-1/file3.txt",
            "rsync://localhost/repo/source-2/file1.txt",
            "rsync://localhost/repo/source-2/file2.txt",
            "rsync://localhost/repo/source-2/file4.txt",
            "rsync://localhost/repo/source-3/file1.txt",
            "rsync://localhost/repo/source-3/file2.txt",
            "rsync://localhost/repo/source-3/file4.txt",
            "rsync://localhost/repo/source-3/file5.txt",
        ];
        let mut expected: Vec<RsyncUri> = expected.into_iter().map(RsyncUri::from).collect();
        expected.sort();

        let mut found: Vec<RsyncUri> = files.iter().map(|f| f.uri.clone()).collect();
        found.sort();

        assert_eq!(expected, found);
    }
}
