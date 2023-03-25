use crate::ffprobe::{FFprobeResult, Track, TrackType};
use crate::cytube_structs::{CytubeVideo, Source, TextTrack as CTTextTrack, AudioTrack as CTAudioTrack};
use crate::ffmpeg_languages::LANGUAGES;
use std::path::Path;
use std::process::{Command, Stdio, Child};
use std::io;

enum Container {
    MP4, WEBM, OGG
}

fn find_container(video_codec: &str) -> Option<Container> {
    use Container::*;
    match video_codec {
        "av1" | "vp8" | "vp9" => Some(WEBM),
        "h264" | // H.264
        "hevc" | // H.265
        "mpeg4"| // MP4V-ES
        "mpeg2video"
                 => Some(MP4),
        "theora" => Some(OGG),
        _ => None,
    }
}

impl Container {
    fn get_acceptable_audio_codecs(&self) -> &'static [&'static str] {
        use Container::*;
        match self {
            MP4  => &["aac", "alac", "flac", "opus", "mp3"],
            WEBM => &["opus", "vorbis"],
            OGG  => &["opus", "vorbis", "flac"],
        }
    }
    fn preferred_audio_encoder(&self) -> &'static str {
        use Container::*;
        match self {
            MP4 => "aac",
            WEBM | OGG => "libopus",
        }
    }
    fn extension(&self) -> &'static str {
        use Container::*;
        match self {
            MP4  => "mp4",
            WEBM => "webm",
            OGG  => "ogv",
        }
    }
    fn mimetype(&self) -> &'static str {
        use Container::*;
        match self {
            MP4  => "video/mp4",
            WEBM => "video/webm",
            OGG  => "video/ogg",
        }
    }
}

fn strcat(first: &str, rest: &[&str]) -> String {
    let mut s = String::from(first);
    for next in rest {
        s.push_str(next);
    }
    s
}

pub fn remux(media_file: &Path, ffprobe: &FFprobeResult, outputdir: &Path, url_prefix: &str, preferred_language: &str) -> (Command, CytubeVideo) {
    let mut subtitle_tracks: Vec<&Track> = Vec::new();
    let mut audio_tracks: Vec<&Track> = Vec::new();
    let mut video_tracks: Vec<&Track> = Vec::new();
    use TrackType::*;
    for track in &ffprobe.tracks {
        match track.kind {
            Video => video_tracks.push(track),
            Audio => audio_tracks.push(track),
            Subtitle => subtitle_tracks.push(track),
        }
    }

    let mut command = Command::new("ffmpeg");
    command.arg("-hide_banner");
    command.args(["-strict", "-2"]);
    command.arg("-i").arg(media_file.as_os_str());

    let mut ct_sources = Vec::new();
    let mut ct_audio_tracks = Vec::new();
    let mut ct_text_tracks = Vec::new();
    
    if let Some(video) = video_tracks.first() {
        if let Some(audio) = audio_tracks.first() {
            command.args(["-map",
                         format!("0:{}", video.index).as_str(),
                         "-map",
                         format!("0:{}", audio.index).as_str(),
            ]);
            if let Some(video_container) = find_container(&video.codec) {
                command.args([
                             "-c:v", "copy",
                             "-c:a",
                ]);
                if video_container.get_acceptable_audio_codecs().contains(&audio.codec.as_str()) {
                    command.arg("copy");
                } else {
                    command.args([video_container.preferred_audio_encoder(),
                                  "-ac", "2"]); // downmix to stereo to make encoding faster
                }

                let filename = format!("main.{}", video_container.extension());

                command.arg(outputdir.join("main").with_extension(video_container.extension()).as_os_str());
                ct_sources.push(Source{
                    bitrate: ffprobe.bitrate,
                    content_type: video_container.mimetype(),
                    quality: video.scanline_count.unwrap(), // TODO
                    url: strcat(url_prefix, &[filename.as_str()]),
                });
            } else {
                // the codec used in the original video file isn't supported by the browser
                // AV1 transcode it is
                command.args(["-c:v", "libstvav1", "-c:a", "libopus", "-ac", "2"]);
                command.arg(outputdir.join("main.webm"));
                ct_sources.push(Source{
                    bitrate: ffprobe.bitrate, // TODO figure out the actual bitrate
                    content_type: "video/webm",
                    quality: video.scanline_count.unwrap(), // TODO
                    url: strcat(url_prefix, &["main.webm"]),
                });
            }
        }
    }



    for sub_track in subtitle_tracks {
        command.args(["-map", format!("0:{}", sub_track.index).as_str()]);
        let lang = match &sub_track.language {
            Some(x) => std::str::from_utf8(x).unwrap(),
            None => "unknown",
        };
        let filename = format!("sub_{}_{}.vtt", sub_track.index, lang);
        command.arg(outputdir.join(&filename).as_os_str());

        // TODO this is the ugliest thing ever.  FIX ME!
        let language_string = match sub_track.language {
            Some(x) => build_language_string(std::str::from_utf8(&x).unwrap(), sub_track.title.as_ref().map(|x|x.as_str())),
            None => sub_track.title.clone().unwrap_or("Unknown".to_string()),
        };

        ct_text_tracks.push(CTTextTrack {
            content_type: "text/vtt",
            url: strcat(url_prefix, &[filename.as_str()]),
            name: language_string,
        });
    }

    (command,
    CytubeVideo {
        title: ffprobe.title.clone().unwrap_or_else(|| media_file.file_stem().unwrap().to_string_lossy().to_string()),
        duration: ffprobe.duration,
        sources: ct_sources,
        audio_tracks: ct_audio_tracks,
        text_tracks: ct_text_tracks,
    })
}

fn build_language_string(language: &str, title: Option<&str>) -> String {
    let mut s = String::from(*LANGUAGES.get(language).unwrap_or(&language));
    if let Some(title) = title {
        s.push_str(" (");
        s.push_str(title);
        s.push(')');
    }
    s
}
