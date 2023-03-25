use cytube_generator::ffprobe::ffprobe;
use cytube_generator::transcode::remux;
use std::path::Path;
use std::os::unix::process::CommandExt;
use serde_json::to_writer;
use std::fs::{OpenOptions, create_dir};

fn main() {
    let file = Path::new("test.mkv");
    let ffprobe = ffprobe(file).expect("ffprobe error");
    let outputdir = Path::new("extracted");
    let (mut command, cytube_data) = remux(file, &ffprobe, outputdir, "https://red.baka.haus/panzer/", Some("eng".into()));

    let _ = create_dir(outputdir);

    {
        let f = OpenOptions::new().write(true).create(true).truncate(true).open(outputdir.join("manifest.json")).expect("could not open JSON file for writing");
        to_writer(f, &cytube_data).expect("error serializing data");
    }

    command.exec();
}
