use std::path::Path;
use std::process::{Command, Stdio};
use fixedstr::str4;

#[derive(Debug)]
#[derive(strum::EnumString)]
#[strum(serialize_all="snake_case")]
pub enum TrackType {
    Video,
    Audio,
    Subtitle,
}

#[derive(Debug)]
pub struct Track {
    pub index: u16,
    pub kind: TrackType,
    pub codec: String,
    pub scanline_count: Option<u16>,
    pub language: Option<str4>,
    pub title: Option<String>,
}

#[derive(Debug)]
pub struct FFprobeResult {
    pub tracks: Vec<Track>,
    pub title: Option<String>,
    pub duration: f32,
    pub bitrate: u64, // in kbps
}

fn parse_ffmpeg_line<'a>(line: &'a str) -> (&'a str, impl Iterator<Item=(&'a str, &'a str)>) {
    let mut it = line.split("|");
    let kind = it.next().unwrap();
    return (kind, it.map(|token| token.split_once("=").unwrap()));
}

pub fn ffprobe(filename: &Path) -> std::io::Result<FFprobeResult> {
    filename.metadata()?; // to make sure we can read the path before invoking ffmpeg
                          // you could remove this but it would make error messages less
                          // informative
    let res = Command::new("ffprobe")
        .arg(filename.as_os_str())
        .arg("-of").arg("compact")
        .arg("-hide_banner")
        .arg("-show_streams").arg("-show_format")
        .arg("-show_entries")
        .arg("stream_tags=title,language:stream=index,codec_type,codec_name,coded_height,bitrate:stream_disposition=:format=duration,bit_rate:format_tags=title")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?
        .wait_with_output()?;
    if !res.status.success() {
        return Err(std::io::Error::new(std::io::ErrorKind::Other, "FFprobe returned error"));
    }
    let output = std::str::from_utf8(&res.stdout).unwrap();
    let mut tracks = Vec::<Track>::new();
    let mut title: Option<String> = None;
    let mut duration = 0.0f32;
    let mut bitrate = 0u64;

    'a: for line in output.split("\n") {
        let (kind, params) = parse_ffmpeg_line(line);
        match kind {
            "format" => {
                for (k,v) in params {
                    match k {
                        "duration" => {duration = v.parse().unwrap();}
                        "bit_rate" => {bitrate = v.parse().unwrap();}
                        "tag:title" => {title = Some(v.to_owned());}
                        x => {println!("uncrecognized tag {}", x);},
                    }
                }
            },
            "stream" => {
                let mut kind: Option<TrackType> = None;
                let mut codec: Option<String> = None;
                let mut scanline_count: Option<u16> = None;
                let mut language: Option<str4> = None;
                let mut title: Option<String> = None;
                let mut index: Option<u16> = None;
                for (k,v) in params {
                    match k {
                        "codec_type" => {
                            kind = Some(match v.parse() {
                                Ok(x) => x,
                                Err(_) => continue 'a, // not a track type we're interested in
                            });
                        },
                        "index" => index = Some(v.parse().unwrap()),
                        "codec_name" => codec = Some(v.to_string()),
                        "coded_height" => scanline_count = Some(v.parse().unwrap()),
                        "tag:language" => {language = Some(v.into())},
                        "tag:title" => title = Some(v.to_string()),
                        x => {println!("uncrecognized tag {}", x);},
                    }
                }
                dbg!(line);
                let index = index.expect("no index");
                let kind = kind.expect("no codec_type");
                let codec = codec.expect("no codec_name");
                tracks.push(Track {index, kind, codec, scanline_count, language, title});
            },
            _ => {},
        }
    }
    Ok(FFprobeResult {tracks, title, duration, bitrate})
}

