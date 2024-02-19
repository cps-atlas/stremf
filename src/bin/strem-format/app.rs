use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use clap::ArgMatches;
use strem::datastream::exporter::stremf::DataExporter;
use strem::datastream::DataStream;
use strem_format::config::Configuration;
use strem_format::schema::nuscenes::NuScenes;
use strem_format::schema::{Schema, SchemaKind};

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

        let datastream = if let Some(infile) = &config.infile {
            let formatter = NuScenes::new(infile, &config);
            formatter.import()?
        } else {
            DataStream::new()
        };

        // Export the [`DataStream`].
        //
        // We first set the [`DataExporter`]. This is necessary to export to the
        // STREM format, accordingly.
        datastream
            .exporter(Box::new(DataExporter::new()))
            .export(&config.outfile)?;

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
                "coco" => SchemaKind::Coco,
                "nuscenes" => SchemaKind::NuScenes,
                "strem" => SchemaKind::Strem,
                "yolo" => SchemaKind::Yolo,
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
