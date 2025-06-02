//! Data objects used in the (RRDP) repository. I.e. the publish, update, and
//! withdraw elements, as well as the notification, snapshot and delta file
//! definitions.
use std::collections::{HashMap, VecDeque};
use std::num::ParseIntError;
use std::path::PathBuf;
use std::str::FromStr;
use std::{fmt, io};

use base64::Engine;
use bytes::Bytes;
use uuid::Uuid;

use crate::sync::{self, Base64, CurrentFile, EncodedHash, HttpsUri, RsyncUri};
use crate::xml::{AttributesError, XmlReader, XmlReaderErr, XmlWriter};

const VERSION: &str = "1";
const NS: &str = "http://www.ripe.net/rpki/rrdp";

//------------ PublishElement ------------------------------------------------

/// The publishes as used in the RRDP protocol.
///
/// Note that the difference with the publication protocol is the absence of
/// the tag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishElement {
    base64: Base64,
    uri: RsyncUri,
}

impl PublishElement {
    pub fn new(base64: Base64, uri: RsyncUri) -> Self {
        PublishElement { base64, uri }
    }

    pub fn base64(&self) -> &Base64 {
        &self.base64
    }
    pub fn uri(&self) -> &RsyncUri {
        &self.uri
    }
}

//------------ UpdateElement -------------------------------------------------

/// The updates as used in the RRDP protocol.
///
/// Note that the difference with the publication protocol is the absence of
/// the tag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateElement {
    uri: RsyncUri,
    hash: EncodedHash,
    base64: Base64,
}

impl UpdateElement {
    pub fn uri(&self) -> &RsyncUri {
        &self.uri
    }
    pub fn hash(&self) -> &EncodedHash {
        &self.hash
    }
    pub fn base64(&self) -> &Base64 {
        &self.base64
    }
}

//------------ WithdrawElement -----------------------------------------------

/// The withdraws as used in the RRDP protocol.
///
/// Note that the difference with the publication protocol is the absence of
/// the tag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawElement {
    uri: RsyncUri,
    hash: EncodedHash,
}

impl WithdrawElement {
    pub fn uri(&self) -> &RsyncUri {
        &self.uri
    }
    pub fn hash(&self) -> &EncodedHash {
        &self.hash
    }
}

//------------ Notification --------------------------------------------------

#[derive(Clone, Debug)]
pub struct Notification {
    session: Uuid,
    serial: u64,
    snapshot: SnapshotRef,
    deltas: VecDeque<DeltaRef>,
}

impl Notification {
    pub fn new(
        session: Uuid,
        serial: u64,
        snapshot: SnapshotRef,
        deltas: VecDeque<DeltaRef>,
    ) -> Self {
        Notification {
            session,
            serial,
            snapshot,
            deltas,
        }
    }

    pub fn write_xml(&self) -> Bytes {
        Bytes::from(XmlWriter::encode_vec(|w| {
            let a = [
                ("xmlns", NS),
                ("version", VERSION),
                ("session_id", &format!("{}", self.session)),
                ("serial", &format!("{}", self.serial)),
            ];

            w.put_element("notification", Some(&a), |w| {
                {
                    // snapshot ref
                    let uri = self.snapshot.uri.to_string();
                    let hash = self.snapshot.hash.to_string();
                    let a = [("uri", uri.as_str()), ("hash", hash.as_str())];
                    w.put_element("snapshot", Some(&a), |w| w.empty())?;
                }

                {
                    // delta refs
                    for delta in &self.deltas {
                        let serial = format!("{}", delta.serial);
                        let uri = delta.file_ref.uri.to_string();
                        let hash = delta.file_ref.hash.to_string();
                        let a = [
                            ("serial", serial.as_ref()),
                            ("uri", uri.as_str()),
                            ("hash", hash.as_str()),
                        ];
                        w.put_element("delta", Some(&a), |w| w.empty())?;
                    }
                }

                Ok(())
            })
        }))
    }
}

//------------ RepoState ------------------------------------------------------

/// This type defines the state of the RRDP repository. It can be saved to disk
/// to save the new notification file, snapshot and delta. It can also purge any
/// deprecated delta files and/or files for deprecated sessions.
///
/// It can be reconstituted by reading the current state from disk starting with
/// a notification file, and ensuring that the included snapshot and deltas all
/// exist and are not tempered with.
///
/// In case the current state cannot be reconstituted this way, a new RepoState,
/// using a new session id will be used.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepoState {
    session: Uuid,
    serial: u64,
    snapshot: Snapshot,
    new_delta: Option<Delta>,
    deltas: VecDeque<DeltaRef>,
    base_uri: HttpsUri,
    base_dir: PathBuf,
}

/// # Data Access
///
impl RepoState {
    pub fn session(&self) -> Uuid {
        self.session
    }
    pub fn serial(&self) -> u64 {
        self.serial
    }
}

impl RepoState {
    /// Creates a new repo state, with a new session id, and serial starting at 1.
    pub fn new(snapshot: Snapshot, base_uri: HttpsUri, base_dir: PathBuf) -> Self {
        let session = snapshot.session;
        let serial = 1;

        let new_delta = None;
        let deltas = VecDeque::new();

        RepoState {
            session,
            serial,
            snapshot,
            new_delta,
            deltas,
            base_uri,
            base_dir,
        }
    }

    /// Saves a notification file, the snapshot, and the optional new delta to disk.
    ///
    /// If clean is true, this will also delete old sessions and delta/snapshot dirs for
    /// old versions which are no longer referenced in the notification file.
    pub fn save(mut self, max_deltas: usize, clean: bool) -> Result<(), io::Error> {
        let serial = self.serial;
        let session = self.session;

        // Save new snapshot
        let snapshot_xml = self.snapshot.write_xml();
        let snapshot_ref = SnapshotRef::new(self.snapshot_uri(serial), &snapshot_xml);
        let snapshot_path = self.snapshot_path(serial);
        sync::save(snapshot_xml.as_ref(), &snapshot_path)?;

        // If there is a new delta, save it and add it to top of the list of delta references
        if let Some(delta) = &self.new_delta {
            let delta_xml = delta.write_xml();
            let delta_file_ref = FileRef::new(self.delta_uri(serial), &delta_xml);
            let delta_ref = DeltaRef::new(serial, delta_file_ref);
            let delta_path = self.delta_path(serial);

            sync::save(delta_xml.as_ref(), &delta_path)?;
            self.deltas.push_front(delta_ref);
        }

        // First purge deltas in excess of snapshot size
        let snapshot_size = snapshot_ref.size();
        let mut deltas_size = 0;
        self.deltas.retain(|d| {
            let add = snapshot_size > deltas_size;
            deltas_size += d.size();
            add
        });

        // Truncate any deltas that exceed the max_deltas number
        self.deltas.truncate(max_deltas);

        let last_serial = self.deltas.back().map(|d| d.serial);

        let notification_path = self.notification_path();
        let notification = Notification::new(self.session, self.serial, snapshot_ref, self.deltas);
        let notification_xml = notification.write_xml();

        sync::save(notification_xml.as_ref(), &notification_path)?;

        if clean {
            // Clean up disk: unused session uuid dirs and unused delta dirs
            sync::retain_disk(&self.base_dir, |name| name == session.to_string())?;

            if let Some(last_serial) = last_serial {
                let session_dir = self.base_dir.join(format!("{}/", self.session));
                sync::retain_disk(&session_dir, |name| {
                    if let Ok(dir_serial) = u64::from_str(&name) {
                        dir_serial >= last_serial
                    } else {
                        eprintln!("Found dir: {}", &name);
                        true // keep any other things the user might have added
                    }
                })?;
            }
        }

        Ok(())
    }

    fn notification_path(&self) -> PathBuf {
        self.base_dir.join(PathBuf::from("notification.xml"))
    }

    fn snapshot_uri(&self, serial: u64) -> HttpsUri {
        self.base_uri.resolve(&self.snapshot_rel(serial))
    }

    fn snapshot_path(&self, serial: u64) -> PathBuf {
        self.base_dir.join(PathBuf::from(self.snapshot_rel(serial)))
    }

    fn snapshot_rel(&self, serial: u64) -> String {
        format!("{}/{}/snapshot.xml", &self.session, serial)
    }

    fn delta_uri(&self, serial: u64) -> HttpsUri {
        self.base_uri.resolve(&self.delta_rel(serial))
    }

    fn delta_path(&self, serial: u64) -> PathBuf {
        self.base_dir.join(PathBuf::from(self.delta_rel(serial)))
    }

    fn delta_rel(&self, serial: u64) -> String {
        format!("{}/{}/delta.xml", &self.session, serial)
    }

    pub fn reconstitute(base_uri: HttpsUri, base_dir: PathBuf) -> Result<Self, Error> {
        let notification_path = base_dir.join("notification.xml");
        let notification = sync::read(&notification_path).map_err(|_| Error::InvalidRepoState)?;

        XmlReader::decode(notification.as_ref(), |r| {
            r.take_named_element("notification", |mut a, r| {
                let version = a.take_req("version")?;
                if version != "1" {
                    return Err(Error::InvalidRepoState);
                }

                let session = a.take_req("session_id")?;
                let session = Uuid::parse_str(&session)?;

                let serial = a.take_req("serial")?;
                let serial = u64::from_str(&serial)?;

                a.exhausted().map_err(Error::invalid_xml)?;

                let snapshot = r.take_named_element("snapshot", |mut a, _r| {
                    let uri = a.take_req("uri")?;
                    let hash = a.take_req("hash")?;
                    a.exhausted()?;

                    let snapshot_rel = base_uri.relative_to(uri).ok_or(Error::InvalidRepoState)?;
                    let snapshot_path = base_dir.join(snapshot_rel);
                    let snapshot =
                        sync::read(&snapshot_path).map_err(|_| Error::InvalidRepoState)?;

                    let snapshot_hash = EncodedHash::from_content(snapshot.as_ref());

                    if snapshot_hash.to_string() != hash {
                        return Err(Error::InvalidRepoState);
                    }

                    Snapshot::from_xml(snapshot)
                })?;

                let new_delta = None;

                let mut deltas = VecDeque::new();

                while let Some(delta) =
                    r.take_opt_element(|t, mut a, _r| match t.name.as_ref() {
                        "delta" => {
                            let serial = a.take_req("serial")?;
                            let serial = u64::from_str(&serial)?;

                            let uri = a.take_req("uri")?;
                            let hash = a.take_req("hash")?;
                            a.exhausted()?;

                            let rel = base_uri.relative_to(uri).ok_or(Error::InvalidRepoState)?;

                            let uri = base_uri.resolve(&rel);
                            let path = base_dir.join(rel);

                            let file = sync::read(&path).map_err(|_| Error::InvalidRepoState)?;
                            let file_ref = FileRef::new(uri, &file);

                            if file_ref.hash().to_string() != hash {
                                return Err(Error::InvalidRepoState);
                            }

                            Ok(Some(DeltaRef::new(serial, file_ref)))
                        }
                        _ => Err(Error::InvalidXml(format!("Unexpected tag: {}", t.name))),
                    })?
                {
                    deltas.push_back(delta)
                }

                Ok(RepoState {
                    session,
                    serial,
                    snapshot,
                    new_delta,
                    deltas,
                    base_uri,
                    base_dir,
                })
            })
        })
    }

    /// Update this RepoState with new snapshot. This will derive the delta.
    /// Returns an error in case the new snapshot is not for the next serial in
    /// the current session.
    pub fn apply(&mut self, new_snapshot: Snapshot) -> Result<(), Error> {
        // Cannot have any pending stuff. One delta only!
        if self.new_delta.is_some() {
            return Err(Error::InvalidDelta);
        }

        // Must be the next snapshot for this state.
        if new_snapshot.serial != self.serial + 1 || new_snapshot.session != self.session {
            return Err(Error::InvalidDelta);
        }

        let delta = self.snapshot.to(&new_snapshot)?;

        if !delta.is_empty() {
            self.snapshot = new_snapshot;
            self.new_delta = Some(delta);
            self.serial += 1;
        }

        Ok(())
    }
}

//------------ FileRef -------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileRef {
    uri: HttpsUri,
    hash: EncodedHash,
    size: usize,
}

impl FileRef {
    pub fn new(uri: HttpsUri, bytes: &Bytes) -> Self {
        let hash = EncodedHash::from_content(bytes.as_ref());
        let size = bytes.len();

        FileRef { uri, hash, size }
    }
    pub fn uri(&self) -> &HttpsUri {
        &self.uri
    }

    pub fn hash(&self) -> &EncodedHash {
        &self.hash
    }

    pub fn size(&self) -> usize {
        self.size
    }
}

pub type SnapshotRef = FileRef;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeltaRef {
    serial: u64,
    file_ref: FileRef,
}

impl DeltaRef {
    pub fn new(serial: u64, file_ref: FileRef) -> Self {
        DeltaRef { serial, file_ref }
    }

    pub fn serial(&self) -> u64 {
        self.serial
    }

    pub fn size(&self) -> usize {
        self.file_ref.size()
    }
}

impl AsRef<FileRef> for DeltaRef {
    fn as_ref(&self) -> &FileRef {
        &self.file_ref
    }
}

//------------ Snapshot ------------------------------------------------------

/// A structure to contain the RRDP snapshot data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Snapshot {
    session: Uuid,
    serial: u64,
    current_objects: Vec<CurrentFile>,
}

impl Snapshot {
    pub fn new(session: Uuid, serial: u64, current_objects: Vec<CurrentFile>) -> Self {
        Snapshot {
            session,
            serial,
            current_objects,
        }
    }

    pub fn to(&self, new_snapshot: &Snapshot) -> Result<Delta, Error> {
        if self.serial != new_snapshot.serial - 1 || self.session != new_snapshot.session {
            return Err(Error::InvalidDelta);
        }

        let old_files: HashMap<_, _> = self.current_objects.iter().map(|o| (o.uri(), o)).collect();

        let mut new_files: HashMap<_, _> = new_snapshot
            .current_objects
            .iter()
            .map(|o| (o.uri(), o))
            .collect();

        let mut publishes = vec![];
        let mut updates = vec![];
        let mut withdraws = vec![];

        for (uri, old_file) in old_files.into_iter() {
            match new_files.remove(uri) {
                Some(new_file) => {
                    if new_file != old_file {
                        updates.push(UpdateElement {
                            uri: uri.clone(),
                            hash: old_file.hash().clone(),
                            base64: new_file.base64().clone(),
                        })
                    }
                }
                None => withdraws.push(WithdrawElement {
                    uri: uri.clone(),
                    hash: old_file.hash().clone(),
                }),
            }
        }

        for (uri, new_file) in new_files.into_iter() {
            publishes.push(PublishElement {
                uri: uri.clone(),
                base64: new_file.base64().clone(),
            })
        }

        let elements = DeltaElements {
            publishes,
            updates,
            withdraws,
        };

        Ok(Delta {
            session: new_snapshot.session,
            serial: new_snapshot.serial,
            elements,
        })
    }

    pub fn len(&self) -> usize {
        self.current_objects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.current_objects.is_empty()
    }

    pub fn write_xml(&self) -> Bytes {
        Bytes::from(XmlWriter::encode_vec(|w| {
            let a = [
                ("xmlns", NS),
                ("version", VERSION),
                ("session_id", &format!("{}", self.session)),
                ("serial", &format!("{}", self.serial)),
            ];

            w.put_element("snapshot", Some(&a), |w| {
                for el in &self.current_objects {
                    let uri = el.uri().to_string();
                    let b64 = el.base64().to_string();
                    let atr = [("uri", uri.as_ref())];
                    w.put_element("publish", Some(&atr), |w| w.put_text(&b64))?;
                }
                Ok(())
            })
        }))
    }

    pub fn from_xml(bytes: Bytes) -> Result<Self, Error> {
        XmlReader::decode(bytes.as_ref(), |r| {
            r.take_named_element("snapshot", |mut a, r| {
                let _version = a.take_req("version")?;
                let session = a.take_req("session_id")?;
                let session = Uuid::from_str(&session)?;
                let serial = a.take_req("serial")?;
                let serial = u64::from_str(serial.as_str())?;
                a.exhausted()?;

                let mut files = vec![];
                while let Some(file) = r.take_opt_element(|t, mut a, r| match t.name.as_ref() {
                    "publish" => {
                        let uri = a.take_req("uri")?;
                        let uri = RsyncUri::from(uri.as_str());
                        a.exhausted()?;

                        let base64 = r.take_chars()?;
                        let content = base64::engine::general_purpose::STANDARD.decode(&base64)?;

                        Ok(Some(CurrentFile::new(uri, &content)))
                    }
                    _ => Err(Error::InvalidXml(format!("Unexpected tag: {}", t.name))),
                })? {
                    files.push(file);
                }

                Ok(Snapshot::new(session, serial, files))
            })
        })
    }
}

//------------ DeltaElements -------------------------------------------------

/// Defines the elements for an RRDP delta.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeltaElements {
    publishes: Vec<PublishElement>,
    updates: Vec<UpdateElement>,
    withdraws: Vec<WithdrawElement>,
}

impl DeltaElements {
    pub fn unwrap(
        self,
    ) -> (
        Vec<PublishElement>,
        Vec<UpdateElement>,
        Vec<WithdrawElement>,
    ) {
        (self.publishes, self.updates, self.withdraws)
    }

    pub fn len(&self) -> usize {
        self.publishes.len() + self.updates.len() + self.withdraws.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn publishes(&self) -> &Vec<PublishElement> {
        &self.publishes
    }

    pub fn updates(&self) -> &Vec<UpdateElement> {
        &self.updates
    }

    pub fn withdraws(&self) -> &Vec<WithdrawElement> {
        &self.withdraws
    }
}

//------------ Delta ---------------------------------------------------------

/// Defines an RRDP delta.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Delta {
    session: Uuid,
    serial: u64,
    elements: DeltaElements,
}

impl Delta {
    pub fn new(session: Uuid, serial: u64, elements: DeltaElements) -> Self {
        Delta {
            session,
            serial,
            elements,
        }
    }

    pub fn session(&self) -> &Uuid {
        &self.session
    }
    pub fn serial(&self) -> u64 {
        self.serial
    }
    pub fn elements(&self) -> &DeltaElements {
        &self.elements
    }

    /// Total number of elements
    ///
    /// This is a cheap approximation of the size of the delta that can help
    /// in determining the choice of how many deltas to include in a
    /// notification file.
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    pub fn unwrap(self) -> (Uuid, u64, DeltaElements) {
        (self.session, self.serial, self.elements)
    }

    pub fn write_xml(&self) -> Bytes {
        Bytes::from(XmlWriter::encode_vec(|w| {
            let a = [
                ("xmlns", NS),
                ("version", VERSION),
                ("session_id", &format!("{}", self.session)),
                ("serial", &format!("{}", self.serial)),
            ];

            w.put_element("delta", Some(&a), |w| {
                for el in &self.elements.publishes {
                    let uri = el.uri.to_string();
                    let b64 = el.base64.to_string();
                    let atr = [("uri", uri.as_ref())];
                    w.put_element("publish", Some(&atr), |w| w.put_text(&b64))?;
                }

                for el in &self.elements.updates {
                    let uri = el.uri.to_string();
                    let b64 = el.base64.to_string();
                    let hash = el.hash.to_string();
                    let atr = [("uri", uri.as_ref()), ("hash", hash.as_ref())];
                    w.put_element("publish", Some(&atr), |w| w.put_text(&b64))?;
                }

                for el in &self.elements.withdraws {
                    let uri = el.uri.to_string();
                    let hash = el.hash.to_string();

                    let atr = [("uri", uri.as_ref()), ("hash", hash.as_ref())];
                    w.put_element("withdraw", Some(&atr), |w| w.empty())?;
                }

                Ok(())
            })
        }))
    }
}

//------------ Error ---------------------------------------------------------
#[derive(Debug, Display)]
pub enum Error {
    #[display("Invalid XML: {}", _0)]
    InvalidXml(String),

    #[display("Invalid delta for current session and serial")]
    InvalidDelta,

    #[display("No valid repo state found on disk")]
    InvalidRepoState,
}

impl Error {
    fn invalid_xml(e: impl fmt::Display) -> Self {
        Error::InvalidXml(e.to_string())
    }
}

impl From<XmlReaderErr> for Error {
    fn from(e: XmlReaderErr) -> Self {
        Error::invalid_xml(e)
    }
}

impl From<AttributesError> for Error {
    fn from(e: AttributesError) -> Self {
        Error::invalid_xml(e)
    }
}

impl From<base64::DecodeError> for Error {
    fn from(e: base64::DecodeError) -> Self {
        Error::invalid_xml(e)
    }
}

impl From<ParseIntError> for Error {
    fn from(e: ParseIntError) -> Self {
        Error::invalid_xml(e)
    }
}

impl From<uuid::Error> for Error {
    fn from(e: uuid::Error) -> Self {
        Error::invalid_xml(e)
    }
}

//------------ Tests ---------------------------------------------------------
//
#[cfg(test)]
mod tests {
    use super::*;
    use crate::rrdp::Snapshot;
    use crate::sync;

    const SOURCE_1: &str = "./test-resources/source-1/";
    const SOURCE_2: &str = "./test-resources/source-2/";
    const SOURCE_3: &str = "./test-resources/source-3/";

    const RSYNC_BASE: &str = "rsync://localhost/repo/";
    const RSYNC_FILE1: &str = "rsync://localhost/repo/file1.txt";
    const RSYNC_FILE3: &str = "rsync://localhost/repo/file3.txt";
    const RSYNC_FILE4: &str = "rsync://localhost/repo/file4.txt";

    fn snapshot_source_1() -> Snapshot {
        let base_dir = PathBuf::from(SOURCE_1);
        let rsync_base = RsyncUri::base_uri(RSYNC_BASE).unwrap();

        let session = Uuid::new_v4();
        let serial = 1;
        let files = sync::crawl_disk(&base_dir, &rsync_base).unwrap();

        Snapshot::new(session, serial, files)
    }

    fn snapshot_from_src(session: Uuid, serial: u64, source: &str) -> Snapshot {
        let base_dir = PathBuf::from(source);
        let rsync_base = RsyncUri::base_uri(RSYNC_BASE).unwrap();

        let files = sync::crawl_disk(&base_dir, &rsync_base).unwrap();

        Snapshot::new(session, serial, files)
    }

    #[test]
    fn save_and_reload_snapshot() {
        let snapshot = snapshot_source_1();

        let xml = snapshot.write_xml();
        let target = PathBuf::from("./test-work/snapshot.xml");

        sync::save(xml.as_ref(), &target).unwrap();

        let bytes = sync::read(&target).unwrap();
        let loaded_snapshot = Snapshot::from_xml(bytes).unwrap();

        assert_eq!(snapshot, loaded_snapshot);
    }

    #[test]
    fn diff_snapshot() {
        let snapshot_1 = snapshot_source_1();
        let snapshot_2 = snapshot_from_src(snapshot_1.session, snapshot_1.serial + 1, SOURCE_2);

        let delta = snapshot_1.to(&snapshot_2).unwrap();

        assert_eq!(2, delta.serial);

        let elements = delta.elements;

        let (publishes, updates, withdraws) = elements.unwrap();

        assert_eq!(1, publishes.len());
        assert_eq!(
            &RsyncUri::from(RSYNC_FILE4),
            publishes.get(0).unwrap().uri()
        );

        assert_eq!(1, updates.len());
        assert_eq!(&RsyncUri::from(RSYNC_FILE1), updates.get(0).unwrap().uri());

        assert_eq!(1, withdraws.len());
        assert_eq!(
            &RsyncUri::from(RSYNC_FILE3),
            withdraws.get(0).unwrap().uri()
        );
    }

    #[test]
    fn save_and_reload_current_state() {
        let snapshot_1 = snapshot_source_1();

        let state = RepoState::new(
            snapshot_1,
            HttpsUri::from("https://localhost/rrdp/"),
            PathBuf::from("./test-work/"),
        );
        let target_dir_1 = PathBuf::from(format!("./test-work/{}/1", state.session));

        state.clone().save(25, true).unwrap();

        let mut loaded_state = RepoState::reconstitute(
            HttpsUri::from("https://localhost/rrdp/"),
            PathBuf::from("./test-work/"),
        )
        .unwrap();

        assert_eq!(state, loaded_state);

        let snapshot_2 = snapshot_from_src(loaded_state.session, loaded_state.serial + 1, SOURCE_2);
        let target_dir_2 = PathBuf::from(format!("./test-work/{}/2", state.session));

        loaded_state.apply(snapshot_2).unwrap();
        loaded_state.save(25, true).unwrap();

        let mut state = RepoState::reconstitute(
            HttpsUri::from("https://localhost/rrdp/"),
            PathBuf::from("./test-work/"),
        )
        .unwrap();
        let target_dir_3 = PathBuf::from(format!("./test-work/{}/3", state.session));

        let snapshot_3 = snapshot_from_src(state.session, state.serial + 1, SOURCE_3);
        state.apply(snapshot_3).unwrap();
        state.save(25, true).unwrap();

        assert!(!target_dir_1.exists()); // dir 1 should be cleaned up (too much space)
        assert!(target_dir_3.exists());

        // Applying a zero delta should be a no-op, so the new target dir should not exist
        // Furthermore, delta 2 should be removed if we limit the max_deltas to 1. I.e.
        // we will only have target dir 3 remaining.
        let mut state = RepoState::reconstitute(
            HttpsUri::from("https://localhost/rrdp/"),
            PathBuf::from("./test-work/"),
        )
        .unwrap();

        let target_dir_4 = PathBuf::from(format!("./test-work/{}/4", state.session));

        let snapshot_4 = snapshot_from_src(state.session, state.serial + 1, SOURCE_3);
        state.apply(snapshot_4).unwrap();
        state.save(1, true).unwrap();

        assert!(!target_dir_2.exists());
        assert!(target_dir_3.exists());
        assert!(!target_dir_4.exists());
    }
}
