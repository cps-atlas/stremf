use std::path::PathBuf;

use clap::builder::PossibleValue;
use clap::{value_parser, Arg, ArgAction, ColorChoice, Command};

pub fn build() -> Command {
    Command::new(clap::crate_name!())
        .color(ColorChoice::Always)
        .about(clap::crate_description!())
        .arg(
            Arg::new("input")
                .short('i')
                .long("input")
                .action(ArgAction::Set)
                .value_parser(value_parser!(PathBuf))
                .value_name("path")
                .help("The path to the input directory or file"),
        )
        .arg(
            Arg::new("FILE")
                .required(true)
                .action(ArgAction::Set)
                .value_parser(value_parser!(PathBuf))
                .help("The path to the output file"),
        )
        .arg(
            Arg::new("schema")
                .short('s')
                .long("schema")
                .action(ArgAction::Set)
                .value_parser([
                    PossibleValue::new("coco"),
                    PossibleValue::new("nuscenes"),
                    PossibleValue::new("strem"),
                    PossibleValue::new("yolo"),
                ])
                .hide_possible_values(true)
                .default_value("nuscenes")
                .hide_default_value(true)
                .value_name("name")
                .help("The input dataset schema"),
        )
        .arg(
            Arg::new("debug")
                .short('d')
                .long("debug")
                .action(ArgAction::SetTrue)
                .help("Enable debugging"),
        )
}
