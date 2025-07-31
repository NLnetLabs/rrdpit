#[macro_use]
extern crate derive_more;
extern crate rrdpit;
extern crate uuid;

use std::fmt;
use std::path::PathBuf;
use uuid::Uuid;

use rrdpit::options::Options;
use rrdpit::rrdp::{RepoState, Snapshot};
use rrdpit::sync::crawl_disk;
use rrdpit::sync::RsyncUri;

fn main() {
    match Options::from_args() {
        Ok(options) => match sync(options) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("{e}");
                ::std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("{e}");
            ::std::process::exit(1);
        }
    }
}

fn snapshot(
    session: Uuid,
    serial: u64,
    source: &PathBuf,
    rsync: &RsyncUri,
) -> Result<Snapshot, Error> {
    let files = crawl_disk(source, rsync).map_err(Error::custom)?;
    Ok(Snapshot::new(session, serial, files))
}

fn sync(options: Options) -> Result<(), Error> {
    let state = match RepoState::reconstitute(options.https.clone(), options.target.clone()) {
        Ok(mut state) => {
            let snapshot = snapshot(
                state.session(),
                state.serial() + 1,
                &options.source,
                &options.rsync,
            )
            .map_err(Error::custom)?;
            state.apply(snapshot).map_err(Error::custom)?;
            state
        }
        Err(_) => {
            let snapshot = snapshot(Uuid::new_v4(), 1, &options.source, &options.rsync)
                .map_err(Error::custom)?;
            RepoState::new(snapshot, options.https.clone(), options.target.clone())
        }
    };

    state
        .save(options.max_deltas, options.clean)
        .map_err(Error::custom)
}

//------------ Error ---------------------------------------------------------
#[derive(Debug, Display)]
pub enum Error {
    #[display("{}", _0)]
    Custom(String),
}

impl Error {
    fn custom(e: impl fmt::Display) -> Self {
        Error::Custom(e.to_string())
    }
}
