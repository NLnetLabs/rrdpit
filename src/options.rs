use clap::{Arg, Command};
use std::path::PathBuf;
use crate::sync::{HttpsUri, RsyncUri};

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
        let matches = Command::new("rrdpit")
            .version(env!("CARGO_PKG_VERSION"))
            .about("Dist to RPKI RRDP")
            .arg(
                Arg::new("source")
                    .long("source")
                    .value_name("dir")
                    .help("source directory")
                    .required(true),
            )
            .arg(
                Arg::new("target")
                    .long("target")
                    .value_name("dir")
                    .help("target directory")
                    .required(true),
            )
            .arg(
                Arg::new("rsync")
                    .long("rsync")
                    .value_name("uri")
                    .help("base rsync uri")
                    .required(true),
            )
            .arg(
                Arg::new("https")
                    .long("https")
                    .value_name("uri")
                    .help("base rrdp uri")
                    .required(true),
            )
            .arg(
                Arg::new("clean")
                    .help("Clean up target dir (handle with care!)")
                    .required(false),
            )
            .arg(
                Arg::new("max_deltas")
                    .long("max_deltas")
                    .value_name("number")
                    .help("Limit the maximum number of deltas kept. Default: 25. Minimum: 1")
                    .required(false),
            )
            .get_matches();

        let source = matches.get_one::<String>("source").unwrap();
        let target = matches.get_one::<String>("target").unwrap();
        let rsync = matches.get_one::<String>("rsync").unwrap();
        let https = matches.get_one::<String>("https").unwrap();
        let max_deltas_default = "25".to_string();
        let max_deltas = matches.get_one::<String>("max_deltas").unwrap_or(&max_deltas_default);

        let clean = matches.contains_id("clean");

        Self::from_strs(source, target, rsync, https, clean, max_deltas)
    }
}

//------------ Error ---------------------------------------------------------

#[derive(Debug, Display)]
pub enum Error {
    #[display("Not a directory: {}", _0)]
    CannotRead(String),

    #[display("Not a directory: {}", _0)]
    RsyncBaseUri(String),

    #[display("Not a directory: {}", _0)]
    HttpsBaseUri(String),

    #[display("Cannot parse number: {}", _0)]
    CannotParseNumber(String),

    #[display("max_deltas must be at least 1")]
    MaxDeltasMustBeOneOrHigher,
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
            &"25",
        )
        .unwrap();
    }
}
