use std::error::Error;
use std::fmt;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::ArgMatches;
use strem::datastream::io::exporter::DataExporter;
use stremf::config::Configuration;
use stremf::schema::nuscenes::NuScenes;
use stremf::schema::{Schema, SchemaKind};

pub struct App {
    matches: ArgMatches,
}

impl App {
    pub fn new(matches: ArgMatches) -> Self {
        Self { matches }
    }

    /// Run the stremf application.
    ///
    /// This method is responsible for handling the Command-Line Interface (CLI)
    /// argument configurations as well as selecting what needs to be run based
    /// on those inputs.
    pub fn run(&self) -> Result<(), Box<dyn Error>> {
        let config = self.configure()?;

        if let Some(infile) = &config.infile {
            let schema = NuScenes::new(infile, &config);
            let datastreams = schema.import()?;

            for (name, datastream) in datastreams {
                let path = PathBuf::from(&config.outfile).join(format!("{}.json", name));
                DataExporter::new().export(&datastream.frames, &path)?;

                if config.debug {
                    println!(
                        "{}",
                        AppDebug::from(format!("exported... {}", path.display()))
                    );
                }
            }
        }

        Ok(())
    }

    /// Create a new [`Configuration`] from the set of [`ArgMatches`].
    ///
    /// This function also maps possible values to typed enumerations within the
    /// crate, accordingly.
    fn configure(&self) -> Result<Configuration, Box<dyn Error>> {
        Ok(Configuration {
            infile: self.matches.get_one::<PathBuf>("input").cloned(),
            outfile: self.matches.get_one::<PathBuf>("FILE").unwrap().clone(),
            schema: match &self.matches.get_one::<String>("schema").unwrap()[..] {
                "nuscenes" => SchemaKind::NuScenes,
                x => {
                    return Err(Box::new(AppError::from(format!(
                        "unsupported schema: `{}`",
                        x
                    ))))
                }
            },
            debug: self.matches.get_flag("debug"),
        })
    }
}

#[derive(Debug, Clone)]
struct AppDebug {
    msg: String,
}

impl From<&str> for AppDebug {
    fn from(msg: &str) -> Self {
        AppDebug {
            msg: msg.to_string(),
        }
    }
}

impl From<String> for AppDebug {
    fn from(msg: String) -> Self {
        AppDebug { msg }
    }
}

impl fmt::Display for AppDebug {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();

        write!(f, "DEBUG({:020}s): stremf: app: {}", timestamp, self.msg)
    }
}

#[derive(Debug, Clone)]
struct AppError {
    msg: String,
}

impl From<&str> for AppError {
    fn from(msg: &str) -> Self {
        AppError {
            msg: msg.to_string(),
        }
    }
}

impl From<String> for AppError {
    fn from(msg: String) -> Self {
        AppError { msg }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "app: {}", self.msg)
    }
}

impl Error for AppError {}
