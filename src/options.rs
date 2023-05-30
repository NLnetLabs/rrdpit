use clap::{App, Arg};
use std::path::PathBuf;
use sync::{HttpsUri, RsyncUri};

pub struct Options {
    pub source: PathBuf,
    pub target: PathBuf,
    pub rsync: RsyncUri,
    pub https: HttpsUri,
    pub clean: bool,
    pub max_deltas: usize,
}

impl Options {
    pub fn from_strs(
        source: &str,
        target: &str,
        rsync: &str,
        https: &str,
        clean: bool,
        max_deltas: &str,
    ) -> Result<Self, Error> {
        let source = PathBuf::from(source);
        let target = PathBuf::from(target);

        let rsync =
            RsyncUri::base_uri(rsync).map_err(|_| Error::RsyncBaseUri(rsync.to_string()))?;
        let https =
            HttpsUri::base_uri(https).map_err(|_| Error::HttpsBaseUri(https.to_string()))?;

        let max_deltas = max_deltas
            .parse::<usize>()
            .map_err(|_| Error::CannotParseNumber(max_deltas.to_string()))?;

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
                max_deltas,
            })
        }
    }

    pub fn from_args() -> Result<Options, Error> {
        let matches = App::new("rrdpit")
            .version("0.0.3")
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
            .arg(
                Arg::with_name("max_deltas")
                    .short("m")
                    .long("max_deltas")
                    .value_name("number")
                    .help("Limit the maximum number of deltas kept. Default: 25")
                    .required(false),
            )
            .get_matches();

        let source = matches.value_of("source").unwrap();
        let target = matches.value_of("target").unwrap();
        let rsync = matches.value_of("rsync").unwrap();
        let https = matches.value_of("https").unwrap();
        let max_deltas = matches.value_of("max_deltas").unwrap_or("25");

        let clean = matches.is_present("clean");

        Self::from_strs(source, target, rsync, https, clean, max_deltas)
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

    #[display(fmt = "Cannot parse number: {}", _0)]
    CannotParseNumber(String),
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
            "25",
        )
        .unwrap();
    }
}
