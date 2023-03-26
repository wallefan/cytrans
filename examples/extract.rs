use cytube_generator::ffprobe::ffprobe;
use cytube_generator::transcode::remux;
use std::path::Path;
use std::os::unix::process::CommandExt;
use serde_json::to_writer;
use std::fs::{OpenOptions, create_dir};

fn main() {
    let mut args = std::env::args_os();
    let argv0 = args.next().unwrap(); // skip argv0
    if args.len() != 4 {
        eprintln!("usage: {} <input file> <output directory> <URL prefix>", argv0.to_string_lossy());
    }
    let file = args.next().unwrap();
    let outputdir = args.next().unwrap();
    let urlprefix = args.next().unwrap();
    
    let file = Path::new(&file);
    let outputdir = Path::new(&outputdir);
    let urlprefix = urlprefix.to_string_lossy();

    let ffprobe = ffprobe(file).expect("ffprobe error");
    let (mut command, cytube_data) = remux(file, &ffprobe, outputdir, &urlprefix, Some("eng".into()));

    if let Err(e) = create_dir(outputdir) {
        if e.kind() != std::io::ErrorKind::AlreadyExists {
            panic!("error creating the output directory: {}", e);
        }
    }

    {
        let f = OpenOptions::new().write(true).create(true).truncate(true).open(outputdir.join("manifest.json")).expect("could not open JSON file for writing");
        to_writer(f, &cytube_data).expect("error serializing data");
    }

    command.exec();
}
