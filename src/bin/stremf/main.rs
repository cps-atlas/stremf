use std::process;

use crate::app::App;
mod cli;

mod app;

fn main() {
    let app = App::new(cli::build().get_matches());

    match app.run() {
        Ok(..) => process::exit(0),
        Err(e) => {
            eprintln!("stremf: error: {}", e);
            process::exit(1);
        }
    }
}
