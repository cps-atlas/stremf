use std::path::PathBuf;

use crate::schema::SchemaKind;

pub struct Configuration {
    /// The path to the input file or directory.
    pub infile: Option<PathBuf>,

    /// The path to the output file.
    pub outfile: PathBuf,

    /// The data schema of the [`self::file`].
    pub schema: SchemaKind,

    /// Print debug statements (when appropriate).
    pub debug: bool,
}
