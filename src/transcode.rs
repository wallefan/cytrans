use crate::ffprobe::{FFprobeResult, Track, TrackType};
use crate::cytube_structs::{CytubeVideo, Source, TextTrack as CTTextTrack, AudioTrack as CTAudioTrack};
use crate::ffmpeg_languages::*;
use std::path::Path;
use std::process::Command;
use fixedstr::str4;

const BITMAP_SUBTITLE_CODECS: [&'static str; 4] = [
    "dvb_subtitle",
    "dvd_subtitle",
    "hdmv_pgs_subtitle",
    "xsub",
];

enum VideoContainer {
    MP4, WEBM, OGG
}

fn find_video_container(video_codec: &str) -> Option<VideoContainer> {
    use VideoContainer::*;
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

impl VideoContainer {
    fn get_acceptable_audio_codecs(&self) -> &'static [&'static str] {
        use VideoContainer::*;
        match self {
            MP4  => &["aac", "alac", "flac", "opus", "mp3"],
            WEBM => &["opus", "vorbis"],
            OGG  => &["opus", "vorbis", "flac"],
        }
    }
    fn preferred_audio_encoder(&self) -> &'static str {
        use VideoContainer::*;
        match self {
            MP4 => "aac",
            WEBM | OGG => "libopus",
        }
    }
    fn extension(&self) -> &'static str {
        use VideoContainer::*;
        match self {
            MP4  => "mp4",
            WEBM => "webm",
            OGG  => "ogv",
        }
    }
    fn mimetype(&self) -> &'static str {
        use VideoContainer::*;
        match self {
            MP4  => "video/mp4",
            WEBM => "video/webm",
            OGG  => "video/ogg",
        }
    }
}

enum AudioContainer {
    M4A, OGG,
    // Every source I can find on the internet says that M4A files are just renamed MP4 files that
    // only contain audio tracks.  However, when I ask ffmpeg to create an M4A file and an MP4 file
    // with the exact same contents, I get two different files.  It puts the sequence "M4A_" in the
    // file subtype of one but not the other.  Also it will refuse to put any codec besides AAC or
    // ALAC in an M4A file.  To work around this, I'm producing "pseudo-M4A" files which are
    // actually literally renamed MP4 files that only contain audio tracks.  My testing says
    // browsers will still play them despite the header saying it's an ISO MP4 rather than an M4A.
    // This allows me to embed audio codecs like MP3 that cytube would otherwise reject.
    PseudoM4A,
}

fn find_audio_container(audio_codec: &str) -> Option<AudioContainer> {
    // Now here's where things get wacky.
    // Cytube doesn't support adding bare FLAC files, citing browser compatiblitity
    // issues with the FLAC codec.
    // Maybe the documentation is just old and Cytube hasn't been updated in a while,
    // but caniuse.com tells a very different story: green lights across the board for
    // any browser released in the last couple years, with a 95% compatibility rating.
    // I should probably see about bugging the guys at Cytube to remove that
    // restriction.
    // In the meantime, however, just because we can't use the FLAC *container*
    // doesn't mean that we can't play FLAC-encoded *audio*.
    // You see, one of the container formats that Cytube *does* accept is Ogg, and
    // there are three audio codecs (that browsers support) that can go inside an
    // Ogg file: Vorbis, Opus, and FLAC.
    // If we embed FLAC data inside an Ogg file, Cytube won't know the difference.  The
    // entire point of the custom metadata files is that Cytube doesn't have to
    // retrieve the files from the media host to run ffprobe on them.  It doesn't know
    // about the codecs, only the container.  We just tell the server we have an
    // Ogg file and it says "great" and ships it to the clients.
    // The Cytube client (webpage) doesn't do any enforcement on its end.  As long as
    // the browser can play it, it'll play ball.
    // We can play FLAC files, we just can't *tell Cytube* we're playing FLAC files.
    use AudioContainer::*;
    match audio_codec {
        "aac" | "alac" | "aac_latm" => Some(M4A),
        "opus" | "vorbis" | "flac" => Some(OGG),
        "mp3" => Some(PseudoM4A),
        _ => None,
    }
}

impl AudioContainer {
    fn extension(&self) -> &'static str {
        use AudioContainer::*;
        match self {
            OGG => "ogg",
            M4A | PseudoM4A => "m4a",
        }
    }
    fn mimetype(&self) -> &'static str {
        use AudioContainer::*;
        match self {
            OGG => "audio/ogg",
            M4A | PseudoM4A => "audio/mp4",
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

pub fn remux(media_file: &Path, ffprobe: &FFprobeResult, outputdir: &Path, url_prefix: &str, preferred_language: Option<str4>) -> (Command, CytubeVideo) {
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
        let video_container = find_video_container(&video.codec);
        let mut chosen_audio = audio_tracks.first();
        let mut highest_score = 0;
        for audio in audio_tracks.iter() {
            let mut score = 0;
            if video_container.as_ref().map_or(true, |container| container.get_acceptable_audio_codecs().contains(&audio.codec.as_str())) {
                score += 1;
            }
            if preferred_language.as_ref().map_or(true, |lang| audio.language == Some(*lang)) {
                score += 1;
            }
            if score > highest_score {
                chosen_audio = Some(audio);
                highest_score = score;
            }
        }
        if let Some(audio) = chosen_audio {
            command.args(["-map",
                         format!("0:{}", video.index).as_str(),
                         "-map",
                         format!("0:{}", audio.index).as_str(),
            ]);
            if let Some(video_container) = video_container {
                command.args([
                             "-c:v", "copy",
                             "-c:a",
                ]);
                if video_container.get_acceptable_audio_codecs().contains(&audio.codec.as_str()) {
                    command.arg("copy");
                    if matches!(video_container, VideoContainer::MP4) && audio.codec == "flac" {
                        // ffmpeg doesn't like putting FLAC streams inside MP4 files, considers it
                        // experimental.  we have to tell it that that's okay
                        command.args(["-strict", "experimental"]);
                    }
                } else {
                    command.args([video_container.preferred_audio_encoder(),
                                  "-ac", "2"]); // downmix to stereo to make encoding faster
                }

                let filename = format!("main.{}", video_container.extension());

                command.arg(outputdir.join(&filename));
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

            for audio_track in audio_tracks.iter() {
                if audio_track.index == audio.index {
                    // no point generating a separate file for the audio track we muxed into the
                    // video file.
                    continue;
                }

                if let Some(container) = find_audio_container(&audio_track.codec) {
                    let language = audio_track.language.as_ref().map(|x| x.as_str()).unwrap_or("unknown");

                    let filename = format!("audio_{}_{}.{}", audio_track.index, language, container.extension());
                    
                    command.arg("-map");
                    command.arg(format!("0:{}", audio_track.index));
                    command.args(["-c", "copy"]);
                    command.arg(outputdir.join(&filename));

                    ct_audio_tracks.push(CTAudioTrack {
                        content_type: container.mimetype(),
                        language: FF2CT.get(language).unwrap_or(&language).to_string(),
                        label: build_language_string(&language, audio_track.title.as_ref().map(|x|x.as_str())),
                        url: strcat(url_prefix, &[&filename]),
                    });
                }
                

            }
        }

    }

    for sub_track in subtitle_tracks {
        if BITMAP_SUBTITLE_CODECS.contains(&sub_track.codec.as_str()) {
            // ffmpeg can't do OCR
            continue;
        }
        command.args(["-map", format!("0:{}", sub_track.index).as_str()]);
        let lang = match &sub_track.language {
            Some(x) => x.as_str(),
            None => "unknown",
        };
        let filename = format!("sub_{}_{}.vtt", sub_track.index, lang);
        command.arg(outputdir.join(&filename).as_os_str());

        let language_string = match sub_track.language {
            Some(x) => build_language_string(x.as_str(), sub_track.title.as_ref().map(|x|x.as_str())),
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
