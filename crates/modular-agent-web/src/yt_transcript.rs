use std::time::Duration;

use modular_agent_core::{
    Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    ModularAgent, async_trait, modular_agent,
};
use quick_xml::Reader;
use quick_xml::escape::resolve_predefined_entity;
use quick_xml::events::{BytesRef, BytesStart, Event};
use reqwest::Client;
use reqwest::header::USER_AGENT;
use serde::{Deserialize, Serialize};
use serde_json::json;
use url::Url;

static CATEGORY: &str = "Web";

static PORT_URL: &str = "url";
static PORT_VIDEO_ID: &str = "video_id";
static PORT_TRANSCRIPT: &str = "transcript";
static PORT_TEXT: &str = "text";

static CONFIG_LANGUAGES: &str = "languages";

static DEFAULT_LANGUAGES: &str = "en";

static REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

static INNERTUBE_PLAYER_URL: &str = "https://www.youtube.com/youtubei/v1/player";

/// The WEB InnerTube client is rejected without a PoToken, so the ANDROID
/// client is used instead. Its version has to look plausible to YouTube, and
/// the same version is echoed in the User-Agent below.
static ANDROID_CLIENT_VERSION: &str = "20.10.38";

fn android_user_agent() -> String {
    format!(
        "com.google.android.youtube/{} (Linux; U; Android 11) gzip",
        ANDROID_CLIENT_VERSION
    )
}

/// One timed segment of a transcript.
///
/// Field names match yt-transcript-rs' `FetchedTranscriptSnippet`, which this
/// module replaced, so presets consuming the `transcript` port keep working.
///
/// `pub` only so the network integration tests can reach it; not part of the
/// crate's supported API.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TranscriptSnippet {
    pub text: String,
    /// Seconds from the start of the video.
    pub start: f64,
    /// Seconds the snippet stays on screen.
    pub duration: f64,
}

/// A fetched transcript, shaped like yt-transcript-rs' `FetchedTranscript`.
///
/// `pub` only so the network integration tests can reach it; not part of the
/// crate's supported API.
#[doc(hidden)]
#[derive(Debug, Clone, Serialize)]
pub struct Transcript {
    pub snippets: Vec<TranscriptSnippet>,
    pub video_id: String,
    pub language: String,
    pub language_code: String,
    pub is_generated: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlayerResponse {
    playability_status: Option<PlayabilityStatus>,
    captions: Option<Captions>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlayabilityStatus {
    status: Option<String>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Captions {
    player_captions_tracklist_renderer: Option<CaptionsTracklist>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CaptionsTracklist {
    #[serde(default)]
    caption_tracks: Vec<CaptionTrack>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CaptionTrack {
    base_url: String,
    language_code: String,
    /// `"asr"` on auto-generated tracks, absent on manually created ones.
    kind: Option<String>,
    name: Option<TrackName>,
}

/// YouTube spells the track label either as `simpleText` or as a run list,
/// depending on the client and the video.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackName {
    simple_text: Option<String>,
    runs: Option<Vec<TrackNameRun>>,
}

#[derive(Debug, Deserialize)]
struct TrackNameRun {
    #[serde(default)]
    text: String,
}

impl CaptionTrack {
    fn is_generated(&self) -> bool {
        self.kind.as_deref() == Some("asr")
    }

    fn display_name(&self) -> String {
        let name = self.name.as_ref().and_then(|name| {
            name.simple_text.clone().or_else(|| {
                name.runs
                    .as_ref()
                    .map(|runs| runs.iter().map(|run| run.text.as_str()).collect())
            })
        });
        name.filter(|name| !name.is_empty())
            .unwrap_or_else(|| self.language_code.clone())
    }
}

/// Pick a caption track: `languages` is walked in order, and within each
/// language a manually created track beats an auto-generated one. An earlier
/// language always wins, even when it only has an auto-generated track — the
/// same order yt-transcript-rs and youtube-transcript-api use.
fn select_track<'a>(
    tracks: &'a [CaptionTrack],
    languages: &[String],
) -> Result<&'a CaptionTrack, AgentError> {
    for language in languages {
        for generated in [false, true] {
            let track = tracks.iter().find(|track| {
                track.is_generated() == generated
                    && track.language_code.eq_ignore_ascii_case(language)
            });
            if let Some(track) = track {
                return Ok(track);
            }
        }
    }

    let available: Vec<String> = tracks
        .iter()
        .map(|track| {
            if track.is_generated() {
                format!("{} (auto-generated)", track.language_code)
            } else {
                track.language_code.clone()
            }
        })
        .collect();
    Err(AgentError::InvalidValue(format!(
        "No transcript found for languages [{}]. Available: [{}]",
        languages.join(", "),
        available.join(", ")
    )))
}

/// Resolve one entity body (the text between `&` and `;`); `None` keeps it
/// verbatim.
fn resolve_entity(entity: &str) -> Option<String> {
    if let Some(predefined) = resolve_predefined_entity(entity) {
        return Some(predefined.to_string());
    }
    let digits = entity.strip_prefix('#')?;
    let code = match digits.strip_prefix(['x', 'X']) {
        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
        None => digits.parse::<u32>().ok()?,
    };
    char::from_u32(code).map(String::from)
}

/// Unescape the entities the legacy timedtext format double-escapes
/// (`&amp;#39;` for `'`, so `&#39;` survives XML parsing).
///
/// Works reference-by-reference so one bad token cannot spoil the rest of the
/// snippet: a bare `&` (as in "Tom & Jerry") or an unresolvable reference is
/// kept verbatim while the surrounding entities still resolve.
fn unescape_lenient(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let resolved = rest[1..]
            .find(';')
            .and_then(|semi| resolve_entity(&rest[1..1 + semi]).map(|value| (value, semi + 2)));
        match resolved {
            Some((value, length)) => {
                out.push_str(&value);
                rest = &rest[length..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Read a numeric attribute, scaled by `scale` (1000 for millisecond values).
/// Returns `Ok(None)` when the attribute is absent or not a number.
fn read_time_attribute(
    element: &BytesStart,
    name: &str,
    scale: f64,
) -> Result<Option<f64>, AgentError> {
    let attribute = element
        .try_get_attribute(name)
        .map_err(|e| AgentError::IoError(format!("Timedtext attribute error: {}", e)))?;
    let Some(attribute) = attribute else {
        return Ok(None);
    };
    let raw = attribute
        .unescape_value()
        .map_err(|e| AgentError::IoError(format!("Timedtext attribute error: {}", e)))?;
    Ok(raw.trim().parse::<f64>().ok().map(|value| value / scale))
}

/// Append a general reference (`&amp;`, `&#39;`, ...) to `out`.
///
/// quick-xml reports references as their own events instead of inlining them,
/// so text assembly has to resolve them. Entities outside the predefined XML
/// set and invalid character references (`&#0;`, out-of-range code points) are
/// kept verbatim rather than aborting the transcript.
fn push_general_ref(out: &mut String, reference: &BytesRef) -> Result<(), AgentError> {
    let name = reference
        .decode()
        .map_err(|e| AgentError::IoError(format!("Timedtext entity error: {}", e)))?;
    if let Ok(Some(ch)) = reference.resolve_char_ref() {
        out.push(ch);
        return Ok(());
    }
    match resolve_predefined_entity(&name) {
        Some(value) => out.push_str(value),
        None => {
            out.push('&');
            out.push_str(&name);
            out.push(';');
        }
    }
    Ok(())
}

/// Close the snippet being built and keep it unless its text is blank.
///
/// Only `legacy` (`<text>`) snippets get the second unescape pass: that format
/// double-escapes entities, while srv3 escapes them once, so a second pass
/// there would corrupt captions whose real text looks like an entity.
fn flush_snippet(
    open: &mut Option<(TranscriptSnippet, bool)>,
    snippets: &mut Vec<TranscriptSnippet>,
) {
    let Some((mut snippet, legacy)) = open.take() else {
        return;
    };
    if legacy {
        snippet.text = unescape_lenient(&snippet.text);
    }
    if !snippet.text.trim().is_empty() {
        snippets.push(snippet);
    }
}

/// Parse a timedtext document into snippets.
///
/// Two formats are served in practice: srv3 (`<p t="ms" d="ms">`, with the
/// words of auto-generated tracks split into nested `<s>` elements) and the
/// legacy one (`<text start="sec" dur="sec">`). They differ only in element
/// name, time unit, and escaping depth (legacy double-escapes entities), so
/// one pass handles both. Snippets whose text is blank are dropped; srv3
/// emits them as timing-only continuation markers. Elements without a
/// parseable start time are ignored, so a non-timedtext document yields no
/// snippets instead of one snippet at 0.0s.
fn parse_timedtext(xml: &str) -> Result<Vec<TranscriptSnippet>, AgentError> {
    let mut reader = Reader::from_str(xml);
    let config = reader.config_mut();
    // Caption text is untrusted input; a stray '&' or an unbalanced tag should
    // not throw away the whole transcript.
    config.allow_dangling_amp = true;
    config.check_end_names = false;
    config.allow_unmatched_ends = true;

    let mut snippets = Vec::new();
    let mut open: Option<(TranscriptSnippet, bool)> = None;

    loop {
        let event = reader
            .read_event()
            .map_err(|e| AgentError::IoError(format!("Timedtext parse error: {}", e)))?;
        match event {
            Event::Start(element) | Event::Empty(element) => {
                let legacy = match element.name().as_ref() {
                    b"p" => false,
                    b"text" => true,
                    // <s> and everything else only contributes text.
                    _ => continue,
                };
                let (start, duration) = if legacy {
                    (
                        read_time_attribute(&element, "start", 1.0)?,
                        read_time_attribute(&element, "dur", 1.0)?,
                    )
                } else {
                    (
                        read_time_attribute(&element, "t", 1000.0)?,
                        read_time_attribute(&element, "d", 1000.0)?,
                    )
                };
                // An element without a parseable start time is not caption
                // data (e.g. an HTML error page served with status 200); skip
                // it so the empty-transcript guard can catch such documents.
                let Some(start) = start else {
                    continue;
                };
                flush_snippet(&mut open, &mut snippets);
                open = Some((
                    TranscriptSnippet {
                        text: String::new(),
                        start,
                        duration: duration.unwrap_or(0.0),
                    },
                    legacy,
                ));
            }
            Event::End(element) => {
                if matches!(element.name().as_ref(), b"p" | b"text") {
                    flush_snippet(&mut open, &mut snippets);
                }
            }
            Event::Text(text) => {
                if let Some((snippet, _)) = open.as_mut() {
                    let decoded = text.xml_content().map_err(|e| {
                        AgentError::IoError(format!("Timedtext decode error: {}", e))
                    })?;
                    snippet.text.push_str(&decoded);
                }
            }
            Event::CData(data) => {
                if let Some((snippet, _)) = open.as_mut() {
                    let decoded = data.decode().map_err(|e| {
                        AgentError::IoError(format!("Timedtext decode error: {}", e))
                    })?;
                    snippet.text.push_str(&decoded);
                }
            }
            Event::GeneralRef(reference) => {
                if let Some((snippet, _)) = open.as_mut() {
                    push_general_ref(&mut snippet.text, &reference)?;
                }
            }
            Event::Eof => {
                flush_snippet(&mut open, &mut snippets);
                break;
            }
            _ => {}
        }
    }

    Ok(snippets)
}

/// Fetch a transcript through YouTube's InnerTube player API.
///
/// Asks the player endpoint for the caption track list, picks a track
/// according to `languages` (see [`select_track`]), then downloads and parses
/// its timedtext document. Kept out of the agent, and `pub`, only so it can be
/// exercised directly by the network integration tests; not part of the
/// crate's supported API.
#[doc(hidden)]
pub async fn fetch_transcript(
    client: &Client,
    video_id: &str,
    languages: &[String],
) -> Result<Transcript, AgentError> {
    let request_body = json!({
        "context": {
            "client": {
                "clientName": "ANDROID",
                "clientVersion": ANDROID_CLIENT_VERSION,
                "androidSdkVersion": 30,
                "hl": "en",
                "gl": "US",
            }
        },
        "videoId": video_id,
    });

    let response = client
        .post(INNERTUBE_PLAYER_URL)
        .header(USER_AGENT, android_user_agent())
        .json(&request_body)
        .send()
        .await
        .map_err(|e| AgentError::IoError(format!("YouTube player API request error: {}", e)))?
        .error_for_status()
        .map_err(|e| AgentError::IoError(format!("YouTube player API failed: {}", e)))?;

    let player: PlayerResponse = response
        .json()
        .await
        .map_err(|e| AgentError::IoError(format!("YouTube player API parse error: {}", e)))?;

    if let Some(playability) = &player.playability_status {
        let status = playability.status.as_deref().unwrap_or("UNKNOWN");
        if status != "OK" {
            let reason = playability.reason.as_deref().unwrap_or("no reason given");
            return Err(AgentError::IoError(format!(
                "YouTube will not play video '{}': {} ({})",
                video_id, status, reason
            )));
        }
    }

    let tracks = player
        .captions
        .and_then(|captions| captions.player_captions_tracklist_renderer)
        .map(|tracklist| tracklist.caption_tracks)
        .unwrap_or_default();
    if tracks.is_empty() {
        return Err(AgentError::InvalidValue(format!(
            "Video '{}' has no captions",
            video_id
        )));
    }

    let track = select_track(&tracks, languages)?;

    let timedtext = client
        .get(&track.base_url)
        .header(USER_AGENT, android_user_agent())
        .send()
        .await
        .map_err(|e| AgentError::IoError(format!("Timedtext request error: {}", e)))?
        .error_for_status()
        .map_err(|e| AgentError::IoError(format!("Timedtext request failed: {}", e)))?
        .text()
        .await
        .map_err(|e| AgentError::IoError(format!("Timedtext read error: {}", e)))?;

    let snippets = parse_timedtext(&timedtext)?;
    if snippets.is_empty() {
        return Err(AgentError::IoError(format!(
            "Transcript for video '{}' ({}) came back empty",
            video_id, track.language_code
        )));
    }

    Ok(Transcript {
        snippets,
        video_id: video_id.to_string(),
        language: track.display_name(),
        language_code: track.language_code.clone(),
        is_generated: track.is_generated(),
    })
}

/// Extract the video ID from a YouTube watch URL.
fn video_id_from_url(url_str: &str) -> Result<String, AgentError> {
    let url = Url::parse(url_str)
        .map_err(|e| AgentError::InvalidValue(format!("Invalid URL '{}': {}", url_str, e)))?;
    let domain = url.domain().unwrap_or("");
    if domain != "www.youtube.com" && domain != "youtube.com" && domain != "youtu.be" {
        return Err(AgentError::InvalidValue(format!(
            "URL '{}' is not a valid YouTube URL",
            url_str
        )));
    }
    if domain == "youtu.be" {
        return Ok(url.path().trim_start_matches('/').to_string());
    }
    url.query_pairs()
        .find(|(key, _)| key == "v")
        .map(|(_, value)| value.to_string())
        .ok_or_else(|| {
            AgentError::InvalidValue(format!("Could not find 'v' parameter in URL '{}'", url_str))
        })
}

/// Fetch the transcript of a YouTube video.
///
/// Calls YouTube's InnerTube player API with the ANDROID client to list the
/// video's caption tracks, then downloads and parses the timedtext document of
/// the best matching track. No API key, cookie, or login is required.
///
/// Track selection walks the `languages` config in order; within each language
/// manually created captions are preferred over auto-generated ones, and an
/// earlier language wins even when it only has an auto-generated track.
/// Videos without captions, and videos YouTube
/// refuses to serve (private, age-restricted, region-blocked), report the
/// reason as an error.
///
/// Accepted URL forms are `youtu.be/<video_id>`,
/// `youtube.com/watch?v=<video_id>`, and `www.youtube.com/watch?v=<video_id>`.
///
/// # Ports
/// - Input `url`: YouTube video URL
/// - Input `video_id`: Bare YouTube video ID, e.g. `dQw4w9WgXcQ`
/// - Output `transcript`: `{ snippets: [{ text, start, duration }], video_id,
///   language, language_code, is_generated }`, with times in seconds
/// - Output `text`: Snippet texts joined with spaces
///
/// # Configuration
/// - `languages`: Comma-separated language codes in preference order
///   (default: "en")
#[modular_agent(
    title = "Fetch YouTube Transcript",
    category = CATEGORY,
    inputs = [PORT_URL, PORT_VIDEO_ID],
    outputs = [PORT_TRANSCRIPT, PORT_TEXT],
    string_config(
        name = CONFIG_LANGUAGES,
        default = DEFAULT_LANGUAGES,
        description = "Comma-separated language codes in preference order, e.g. en,ja",
    ),
)]
struct FetchYtTranscriptAgent {
    data: AgentData,
    client: Client,
}

#[async_trait]
impl AsAgent for FetchYtTranscriptAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|e| AgentError::IoError(format!("YouTube client build error: {}", e)))?;
        Ok(Self {
            data: AgentData::new(ma, id, spec),
            client,
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let video_id = if port == PORT_URL {
            let url_str = value.as_str().ok_or_else(|| {
                AgentError::InvalidValue("Input value for 'url' must be a string".to_string())
            })?;
            video_id_from_url(url_str)?
        } else if port == PORT_VIDEO_ID {
            value
                .as_str()
                .ok_or_else(|| {
                    AgentError::InvalidValue(
                        "Input value for 'video_id' must be a string".to_string(),
                    )
                })?
                .to_string()
        } else {
            return Err(AgentError::InvalidValue(format!(
                "Unexpected input port '{}'",
                port
            )));
        };

        let languages: Vec<String> = self
            .configs()?
            .get_string_or(CONFIG_LANGUAGES, DEFAULT_LANGUAGES)
            .split(',')
            .map(|language| language.trim().to_string())
            .filter(|language| !language.is_empty())
            .collect();

        let transcript = fetch_transcript(&self.client, &video_id, &languages).await?;

        let mut text = String::new();
        for snippet in &transcript.snippets {
            text.push_str(&snippet.text);
            text.push(' ');
        }

        self.output(
            ctx.clone(),
            PORT_TRANSCRIPT,
            AgentValue::from_serialize(&transcript).map_err(|e| {
                AgentError::IoError(format!("Transcript Serialization Error: {}", e))
            })?,
        )
        .await?;

        self.output(ctx, PORT_TEXT, text.into()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRV3: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<timedtext format="3">
<head><wp id="0"/><ws id="0"/></head>
<body>
<p t="1200" d="2500">Never gonna give you up</p>
<p t="3700" d="1800"><s>Never </s><s t="300">gonna </s><s t="600">let you down</s></p>
<p t="5500" d="900">Tom &amp; Jerry &#39;n friends</p>
<p t="6400" d="10"></p>
</body>
</timedtext>"#;

    const LEGACY: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<transcript>
<text start="1.2" dur="2.5">Never gonna give you up</text>
<text start="3.7" dur="1.8">Never gonna &amp;#39;let you down&amp;#39;</text>
</transcript>"#;

    fn track(language_code: &str, kind: Option<&str>) -> CaptionTrack {
        CaptionTrack {
            base_url: format!("https://example.com/{}", language_code),
            language_code: language_code.to_string(),
            kind: kind.map(|kind| kind.to_string()),
            name: None,
        }
    }

    #[test]
    fn parses_srv3_with_nested_segments() {
        let snippets = parse_timedtext(SRV3).unwrap();
        assert_eq!(snippets.len(), 3);

        assert_eq!(snippets[0].text, "Never gonna give you up");
        assert_eq!(snippets[0].start, 1.2);
        assert_eq!(snippets[0].duration, 2.5);

        // Nested <s> segments are concatenated into one snippet.
        assert_eq!(snippets[1].text, "Never gonna let you down");
        assert_eq!(snippets[1].start, 3.7);
    }

    #[test]
    fn resolves_entities() {
        let snippets = parse_timedtext(SRV3).unwrap();
        assert_eq!(snippets[2].text, "Tom & Jerry 'n friends");
    }

    #[test]
    fn parses_legacy_format_in_seconds() {
        let snippets = parse_timedtext(LEGACY).unwrap();
        assert_eq!(snippets.len(), 2);
        assert_eq!(snippets[0].start, 1.2);
        assert_eq!(snippets[0].duration, 2.5);
        // Double-escaped text needs the extra unescape pass.
        assert_eq!(snippets[1].text, "Never gonna 'let you down'");
    }

    #[test]
    fn invalid_char_ref_does_not_abort_the_parse() {
        let xml = r#"<timedtext format="3"><body>
<p t="0" d="1000">a&#0;b</p>
<p t="1000" d="1000">good</p>
</body></timedtext>"#;
        let snippets = parse_timedtext(xml).unwrap();
        assert_eq!(snippets.len(), 2);
        assert_eq!(snippets[0].text, "a&#0;b");
        assert_eq!(snippets[1].text, "good");
    }

    #[test]
    fn srv3_text_is_unescaped_only_once() {
        // A caption whose real text contains entity-looking strings, e.g. a
        // programming tutorial. srv3 escapes once, so no second pass.
        let xml = r#"<timedtext format="3"><body>
<p t="0" d="1000">write &amp;lt;div&amp;gt; here</p>
</body></timedtext>"#;
        let snippets = parse_timedtext(xml).unwrap();
        assert_eq!(snippets[0].text, "write &lt;div&gt; here");
    }

    #[test]
    fn legacy_bare_ampersand_does_not_block_entity_resolution() {
        let xml = r#"<transcript>
<text start="0" dur="1">Tom &amp; Jerry isn&amp;#39;t bad</text>
</transcript>"#;
        let snippets = parse_timedtext(xml).unwrap();
        assert_eq!(snippets[0].text, "Tom & Jerry isn't bad");
    }

    #[test]
    fn elements_without_timing_are_not_snippets() {
        let html = "<html><body><p>Sorry, an error occurred</p></body></html>";
        assert!(parse_timedtext(html).unwrap().is_empty());

        let bad_time =
            r#"<timedtext format="3"><body><p t="abc" d="1000">x</p></body></timedtext>"#;
        assert!(parse_timedtext(bad_time).unwrap().is_empty());
    }

    #[test]
    fn empty_document_yields_no_snippets() {
        assert!(parse_timedtext("").unwrap().is_empty());
        assert!(
            parse_timedtext("<timedtext format=\"3\"><body/></timedtext>")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn prefers_manual_track_over_generated() {
        let tracks = vec![track("en", Some("asr")), track("en", None)];
        let languages = vec!["en".to_string()];
        let selected = select_track(&tracks, &languages).unwrap();
        assert!(!selected.is_generated());
    }

    #[test]
    fn falls_back_to_generated_track() {
        let tracks = vec![track("ja", None), track("en", Some("asr"))];
        let languages = vec!["en".to_string()];
        let selected = select_track(&tracks, &languages).unwrap();
        assert_eq!(selected.language_code, "en");
        assert!(selected.is_generated());
    }

    #[test]
    fn earlier_language_wins_over_later_manual_track() {
        let tracks = vec![track("en", Some("asr")), track("ja", None)];
        let languages = vec!["en".to_string(), "ja".to_string()];
        let selected = select_track(&tracks, &languages).unwrap();
        assert_eq!(selected.language_code, "en");
        assert!(selected.is_generated());
    }

    #[test]
    fn unknown_language_lists_available_tracks() {
        let tracks = vec![track("en", Some("asr"))];
        let languages = vec!["fr".to_string()];
        let err = select_track(&tracks, &languages).unwrap_err();
        let AgentError::InvalidValue(message) = err else {
            panic!("expected InvalidValue");
        };
        assert!(message.contains("fr"));
        assert!(message.contains("en (auto-generated)"));
    }

    #[test]
    fn display_name_falls_back_to_language_code() {
        assert_eq!(track("en", None).display_name(), "en");

        let mut named = track("en", None);
        named.name = Some(TrackName {
            simple_text: Some("English".to_string()),
            runs: None,
        });
        assert_eq!(named.display_name(), "English");

        let mut with_runs = track("en", None);
        with_runs.name = Some(TrackName {
            simple_text: None,
            runs: Some(vec![
                TrackNameRun {
                    text: "English ".to_string(),
                },
                TrackNameRun {
                    text: "(auto-generated)".to_string(),
                },
            ]),
        });
        assert_eq!(with_runs.display_name(), "English (auto-generated)");
    }

    #[test]
    fn video_id_is_extracted_from_url_forms() {
        assert_eq!(
            video_id_from_url("https://youtu.be/dQw4w9WgXcQ").unwrap(),
            "dQw4w9WgXcQ"
        );
        assert_eq!(
            video_id_from_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=10s").unwrap(),
            "dQw4w9WgXcQ"
        );
        assert!(video_id_from_url("https://example.com/watch?v=x").is_err());
        assert!(video_id_from_url("https://www.youtube.com/watch").is_err());
    }
}
