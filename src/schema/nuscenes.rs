use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use nalgebra::{
    Quaternion, RowSVector as StaticRowVector, SMatrix as StaticMatrix, Translation, UnitQuaternion,
};
use serde::de::DeserializeOwned;
use strem::datastream::frame::sample::detections::bbox::region::{aa, Point};
use strem::datastream::frame::sample::detections::bbox::BoundingBox;
use strem::datastream::frame::sample::detections::{
    Annotation, DetectionRecord, Image, ImageSource,
};
use strem::datastream::frame::sample::Sample;
use strem::datastream::frame::Frame;

use crate::config::Configuration;

use super::Schema;

use self::annotation::Annotation as NuAnnotation;
use self::calibration::Calibration as NuCalibration;
use self::category::Category as NuCategory;
use self::data::Data as NuData;
use self::ego::Ego as NuEgo;
use self::instance::Instance as NuInstance;
use self::sample::Sample as NuSample;
use self::scene::Scene as NuScene;
use self::sensor::Sensor as NuSensor;

mod annotation;
mod calibration;
mod category;
mod data;
mod ego;
mod instance;
mod sample;
mod scene;
mod sensor;

type SampleToken = String;
type SceneToken = String;
type InstanceToken = String;
type CategoryToken = String;
type EgoToken = String;
type CalibrationToken = String;
type SensorToken = String;

pub struct NuScenes<'a> {
    pub root: PathBuf,
    pub config: &'a Configuration,
}

impl<'a> NuScenes<'a> {
    pub fn new<P: Into<PathBuf>>(root: P, config: &'a Configuration) -> Self {
        let root = root.into();
        Self { root, config }
    }

    /// Load JSON-based data from the NuScenes formatted file.
    ///
    /// # Type Parameters
    ///
    /// - `P`: The source to read from.
    /// - `T`: The type to deserialize into.
    ///
    /// This will read from a [`BufReader`] and serialize into the appropriate
    /// data structures, accordingly.
    fn load<T>(&self, filename: &str) -> Result<Vec<T>, Box<dyn Error>>
    where
        T: DeserializeOwned,
    {
        // Set path to file.
        let mut path = PathBuf::from(&self.root);
        path.push(filename);

        // Set up reader from the provided path.
        let infile = File::open(&path).or(Err(Box::new(NuScenesError::from(format!(
            "unable to open `{}`",
            path.display()
        )))))?;

        let reader = BufReader::new(infile);
        let data = serde_json::from_reader(reader)?;

        if self.config.debug {
            println!(
                "{}",
                NuScenesDebug::from(format!(
                    "serde: deserialized data from `{}`",
                    path.display()
                ))
            );
        }

        Ok(data)
    }

    fn debug(&self, msg: &str) {
        if self.config.debug {
            println!("{}", NuScenesDebug::from(msg));
        }
    }

    fn channel(&self, channel: &str) -> Option<String> {
        match channel {
            "CAM_FRONT" => Some(String::from("cam::front")),
            "CAM_FRONT_ZOOMED" => Some(String::from("cam::front::zoomed")),
            "CAM_FRONT_LEFT" => Some(String::from("cam::front::left")),
            "CAM_FRONT_RIGHT" => Some(String::from("cam::front::right")),
            "CAM_BACK" => Some(String::from("cam::back")),
            "CAM_BACK_LEFT" => Some(String::from("cam::back::left")),
            "CAM_BACK_RIGHT" => Some(String::from("cam::back::right")),
            _ => None,
        }
    }

    fn image(&self, data: &NuData) -> Option<Image> {
        if let Some(width) = data.width {
            if let Some(height) = data.height {
                let source = ImageSource::File(PathBuf::from(&data.filename));
                return Some(Image::new(source, width as u32, height as u32));
            }
        }

        None
    }

    fn annotations(
        &self,
        data: &NuData,
        ego: &NuEgo,
        calibration: &NuCalibration,
        annotations: &Vec<NuAnnotation>,
        instances: &HashMap<InstanceToken, NuInstance>,
        categories: &HashMap<CategoryToken, NuCategory>,
    ) -> HashMap<String, Vec<Annotation>> {
        let mut res: HashMap<String, Vec<Annotation>> = HashMap::new();
        let viewport = StaticMatrix::<f64, 3, 3>::from_rows(&[
            StaticRowVector::<f64, 3>::from(calibration.camera_intrinsic.unwrap()[0]),
            StaticRowVector::<f64, 3>::from(calibration.camera_intrinsic.unwrap()[1]),
            StaticRowVector::<f64, 3>::from(calibration.camera_intrinsic.unwrap()[2]),
        ]);

        for annotation in annotations {
            let instance = instances.get(&annotation.instance_token).unwrap();
            let label = categories.get(&instance.category_token).unwrap();

            // Project the [`NuAnnotation`] onto the sensor.
            //
            // This is done to convert a 3D bounding box into a 2D bounding box
            // that can be used by [`strem`], accordingly.
            let a = self.translate(annotation, ego, calibration);

            if a.inside(viewport, data.width.unwrap(), data.height.unwrap()) {
                let m = a.projection(viewport);

                let xmin = m.row(0).iter().copied().fold(f64::NAN, f64::min);
                let ymin = m.row(1).iter().copied().fold(f64::NAN, f64::min);

                let xmax = m.row(0).iter().copied().fold(f64::NAN, f64::max);
                let ymax = m.row(1).iter().copied().fold(f64::NAN, f64::max);

                let width = xmax - xmin;
                let height = ymax - ymin;

                res.entry(label.name.clone())
                    .or_default()
                    .push(Annotation::new(
                        label.name.clone(),
                        Some(annotation.instance_token.clone()),
                        1.0,
                        BoundingBox::AxisAligned(aa::Region::new(
                            Point::new(xmin + (width / 2.0), ymin + (height / 2.0)),
                            width,
                            height,
                        )),
                    ));
            }
        }

        res
    }

    // Translate the [`NuAnnotation`].
    //
    // This includes: (1) translating the [`NuAnnotation`] with respect to the
    // [`NuEgo`] position, and (2) translating the [`NuAnnotation`] with respect
    // to the `[NuCalibration]` position.
    fn translate(
        &self,
        annotation: &NuAnnotation,
        ego: &NuEgo,
        calibration: &NuCalibration,
    ) -> NuAnnotation {
        annotation
            .clone()
            .transform(
                Translation::<f64, 3>::new(
                    ego.translation[0],
                    ego.translation[1],
                    ego.translation[2],
                )
                .inverse(),
                UnitQuaternion::from_quaternion(Quaternion::new(
                    ego.rotation[0],
                    ego.rotation[1],
                    ego.rotation[2],
                    ego.rotation[3],
                ))
                .inverse(),
            )
            .transform(
                Translation::<f64, 3>::new(
                    calibration.translation[0],
                    calibration.translation[1],
                    calibration.translation[2],
                )
                .inverse(),
                UnitQuaternion::from_quaternion(Quaternion::new(
                    calibration.rotation[0],
                    calibration.rotation[1],
                    calibration.rotation[2],
                    calibration.rotation[3],
                ))
                .inverse(),
            )
    }
}

impl Schema for NuScenes<'_> {
    fn import(&self) -> Result<Vec<(String, Vec<Frame>)>, Box<dyn Error>> {
        self.debug(&format!("root directory at `{}`", self.root.display()));

        // Set up internal database.
        //
        // Because NuScenes uses a foreign key-based system, the keys and
        // associated values must first be set up in order to import the scenes
        // linearly.
        self.debug("building internal database");

        let scenes: HashMap<SceneToken, NuScene> = self
            .load::<NuScene>("scene.json")?
            .into_iter()
            .map(|x| (x.token.clone(), x))
            .collect();

        let samples: HashMap<SampleToken, NuSample> = self
            .load::<NuSample>("sample.json")?
            .into_iter()
            .map(|x| (x.token.clone(), x))
            .collect();

        // There are multiple [`NuAnnotation`] per sample.
        //
        // Therefore, a mapping between a sample and it associated set of
        // [`NuAnnotation`] must be created.
        let mut annotations: HashMap<SampleToken, Vec<NuAnnotation>> = HashMap::new();

        for a in self.load::<NuAnnotation>("sample_annotation.json")? {
            let token = a.sample_token.clone();
            annotations.entry(token).or_default().push(a);
        }

        // There are multiple [`NuData`] per sample.
        //
        // Therefore, a mapping between a sample and it associated set of
        // [`NuData`] must be created.
        let mut datas: HashMap<SampleToken, Vec<NuData>> = HashMap::new();

        for d in self.load::<NuData>("sample_data.json")? {
            let token = d.sample_token.clone();
            datas.entry(token).or_default().push(d);
        }

        let instances: HashMap<InstanceToken, NuInstance> = self
            .load::<NuInstance>("instance.json")?
            .into_iter()
            .map(|x| (x.token.clone(), x))
            .collect();

        let categories: HashMap<CategoryToken, NuCategory> = self
            .load::<NuCategory>("category.json")?
            .into_iter()
            .map(|x| (x.token.clone(), x))
            .collect();

        let egos: HashMap<EgoToken, NuEgo> = self
            .load::<NuEgo>("ego_pose.json")?
            .into_iter()
            .map(|x| (x.token.clone(), x))
            .collect();

        let calibrations: HashMap<CalibrationToken, NuCalibration> = self
            .load::<NuCalibration>("calibrated_sensor.json")?
            .into_iter()
            .map(|x| (x.token.clone(), x))
            .collect();

        let sensors: HashMap<SensorToken, NuSensor> = self
            .load::<NuSensor>("sensor.json")?
            .into_iter()
            .map(|x| (x.token.clone(), x))
            .collect();

        // Construct the set of [`Frame`].
        //
        // This will loop through each scene and collect the samples and
        // associated data into a linear stream.
        let mut datastreams = Vec::new();

        for scene in scenes.values() {
            let mut frames = Vec::new();
            let mut index = 0;

            let mut current = &scene.first_sample_token;

            while let Some(sample) = samples.get(current) {
                // Insert [`Frame`] into the [`DataStream`], accordingly.
                //
                // The index and associated timestamp of the [`Frame`] must be
                // provided when constructing the [`Frame`].
                let mut frame = Frame::new(index);

                for data in datas.get(&sample.token).unwrap() {
                    let calibration = calibrations.get(&data.calibrated_sensor_token).unwrap();
                    let sensor = sensors.get(&calibration.sensor_token).unwrap();

                    // If [`Some`] mapping exists, proceed.
                    //
                    // The [`self::channel`] function is used to filter out
                    // sensor/data that we do not want to consider.
                    if let Some(channel) = self.channel(&sensor.channel) {
                        let mut record = DetectionRecord::new(channel, self.image(data));

                        // Add the set of annotations to the [`DetectionRecord`].
                        //
                        // The annotations added ONLY contain those that are
                        // within the FOV (Field of View) of the data sensor,
                        // accordingly.
                        if let Some(annotations) = annotations.get(&sample.token) {
                            record.annotations = self.annotations(
                                data,
                                egos.get(&data.ego_pose_token).unwrap(),
                                calibration,
                                annotations,
                                &instances,
                                &categories,
                            );
                        }

                        // INSERT
                        frame.samples.push(Sample::ObjectDetection(record));
                    }
                }

                index += 1;
                current = &sample.next;

                // INSERT
                frames.push(frame);
            }

            datastreams.push((scene.token.clone(), frames));
        }

        Ok(datastreams)
    }
}

#[derive(Debug, Clone)]
struct NuScenesDebug {
    msg: String,
}

impl From<&str> for NuScenesDebug {
    fn from(msg: &str) -> Self {
        NuScenesDebug {
            msg: msg.to_string(),
        }
    }
}

impl From<String> for NuScenesDebug {
    fn from(msg: String) -> Self {
        NuScenesDebug { msg }
    }
}

impl fmt::Display for NuScenesDebug {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();

        write!(
            f,
            "DEBUG({:020}s): stremf: nuscenes: {}",
            timestamp, self.msg
        )
    }
}

#[derive(Debug, Clone)]
struct NuScenesError {
    msg: String,
}

impl From<&str> for NuScenesError {
    fn from(msg: &str) -> Self {
        NuScenesError {
            msg: msg.to_string(),
        }
    }
}

impl From<String> for NuScenesError {
    fn from(msg: String) -> Self {
        NuScenesError { msg }
    }
}

impl fmt::Display for NuScenesError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "nuscenes: {}", self.msg)
    }
}

impl Error for NuScenesError {}
