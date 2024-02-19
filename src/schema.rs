use std::error::Error;

use strem::datastream::DataStream;

pub mod nuscenes;

pub trait Schema {
    fn import(&self) -> Result<DataStream, Box<dyn Error>>;
}

/// The set of schemas supported.
///
/// This support only includes importing and not necessarily exporting. This is
/// by design as this tool is for converting into STREM and not vice-versa.
pub enum SchemaKind {
    Coco,
    NuScenes,
    Strem,
    Yolo,
}
