use clap::{App, Arg};
use std::path::PathBuf;
use sync::{HttpsUri, RsyncUri};

pub struct Options {
    pub source: PathBuf,
    pub target: PathBuf,
    pub rsync: RsyncUri,
    pub https: HttpsUri,
    pub clean: bool,
}

impl Options {
    pub fn from_strs(
        source: &str,
        target: &str,
        rsync: &str,
        https: &str,
        clean: bool,
    ) -> Result<Self, Error> {
        let source = PathBuf::from(source);
        let target = PathBuf::from(target);

        let rsync =
            RsyncUri::base_uri(rsync).map_err(|_| Error::RsyncBaseUri(rsync.to_string()))?;
        let https =
            HttpsUri::base_uri(https).map_err(|_| Error::HttpsBaseUri(https.to_string()))?;

        if !source.is_dir() {
            Err(Error::cannot_read(source))
        } else if !target.is_dir() {
            Err(Error::cannot_read(target))
        } else {
            Ok(Options {
                source,
                target,
                rsync,
                https,
                clean,
            })
        }
    }

    pub fn from_args() -> Result<Options, Error> {
        let matches = App::new("rrdpit")
            .version("0.0.2")
            .about("Dist to RPKI RRDP")
            .arg(
                Arg::with_name("source")
                    .short("s")
                    .long("source")
                    .value_name("dir")
                    .help("source directory")
                    .required(true),
            )
            .arg(
                Arg::with_name("target")
                    .short("t")
                    .long("target")
                    .value_name("dir")
                    .help("target directory")
                    .required(true),
            )
            .arg(
                Arg::with_name("rsync")
                    .short("r")
                    .long("rsync")
                    .value_name("uri")
                    .help("base rsync uri")
                    .required(true),
            )
            .arg(
                Arg::with_name("https")
                    .short("h")
                    .long("https")
                    .value_name("uri")
                    .help("base rrdp uri")
                    .required(true),
            )
            .arg(
                Arg::with_name("clean")
                    .help("Clean up target dir (handle with care!)")
                    .required(false),
            )
            .get_matches();

        let source = matches.value_of("source").unwrap();
        let target = matches.value_of("target").unwrap();
        let rsync = matches.value_of("rsync").unwrap();
        let https = matches.value_of("https").unwrap();

        let clean = matches.is_present("clean");

        Self::from_strs(source, target, rsync, https, clean)
    }
}

//------------ Error ---------------------------------------------------------

#[derive(Debug, Display)]
pub enum Error {
    #[display(fmt = "Not a directory: {}", _0)]
    CannotRead(String),

    #[display(fmt = "Not a directory: {}", _0)]
    RsyncBaseUri(String),

    #[display(fmt = "Not a directory: {}", _0)]
    HttpsBaseUri(String),
}

impl Error {
    fn cannot_read(source: PathBuf) -> Self {
        Error::CannotRead(source.to_string_lossy().to_string())
    }
}

//------------ Tests ---------------------------------------------------------

#[cfg(test)]
pub mod tests {

    use super::*;

    #[test]
    fn parse_arguments() {
        Options::from_strs(
            "./test-resources/source-1",
            "./test-work",
            "rsync://localhost/repo/",
            "https://localhost/repo/",
            false,
        )
        .unwrap();
    }
}
