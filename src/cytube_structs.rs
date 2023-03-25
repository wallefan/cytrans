use serde::Serialize;

pub const CYTUBE_ACCEPTABLE_QUALITY_VALUES: [u16; 8] = [240, 360, 480, 540, 720, 1080, 1440, 2160];


#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub struct CytubeVideo {
    pub title: String,
    pub duration: f32,
    pub sources: Vec<Source>,
    pub audio_tracks: Vec<AudioTrack>,
    pub text_tracks: Vec<TextTrack>,
}

#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub struct Source {
    pub url: String,
    pub content_type: &'static str,
    pub quality: u16, // cytube accepts 240, 360, 480, 540, 720, 1080, 1440, and 2160
    pub bitrate: u64,
}

#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub struct TextTrack {
    pub url: String,
    pub name: String,
    pub content_type: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub struct AudioTrack {
    pub url: String,
    pub label: String,
    pub language: String,
    pub content_type: &'static str,
}


