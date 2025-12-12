use agent_stream_kit::{
    ASKit, Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    askit_agent, async_trait,
};
use url::Url;
use yt_transcript_rs::api::YouTubeTranscriptApi;

static CATEGORY: &str = "Web";

static PORT_URL: &str = "url";
static PORT_VIDEO_ID: &str = "video_id";
static PORT_TRANSCRIPT: &str = "transcript";
static PORT_TEXT: &str = "text";

static CONFIG_LANGUAGES: &str = "languages";

/// Fetch YouTube video transcript from a given URL
#[askit_agent(
    title = "Fetch YouTube Transcript",
    category = CATEGORY,
    inputs = [PORT_URL, PORT_VIDEO_ID],
    outputs = [PORT_TRANSCRIPT, PORT_TEXT],
    string_config(
        name=CONFIG_LANGUAGES,
        default="en",
    ),
)]
struct FetchYtTranscriptAgent {
    data: AgentData,
}

#[async_trait]
impl AsAgent for FetchYtTranscriptAgent {
    fn new(askit: ASKit, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        pin: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let video_id;
        if pin == PORT_URL {
            let url_str = value.as_str().ok_or_else(|| {
                AgentError::InvalidValue("Input value for 'url' must be a string".to_string())
            })?;
            let url = Url::parse(url_str).map_err(|e| {
                AgentError::InvalidValue(format!("Invalid URL '{}': {}", url_str, e))
            })?;
            // check if it's a YouTube URL
            let domain = url.domain().unwrap_or("");
            if domain != "www.youtube.com" && domain != "youtube.com" && domain != "youtu.be" {
                return Err(AgentError::InvalidValue(format!(
                    "URL '{}' is not a valid YouTube URL",
                    url_str
                )));
            }
            video_id = if domain == "youtu.be" {
                url.path().trim_start_matches('/').to_string()
            } else {
                url.query_pairs()
                    .find(|(key, _)| key == "v")
                    .map(|(_, value)| value.to_string())
                    .ok_or_else(|| {
                        AgentError::InvalidValue(format!(
                            "Could not find 'v' parameter in URL '{}'",
                            url_str
                        ))
                    })?
            };
        } else if pin == PORT_VIDEO_ID {
            video_id = value
                .as_str()
                .ok_or_else(|| {
                    AgentError::InvalidValue(
                        "Input value for 'video_id' must be a string".to_string(),
                    )
                })?
                .to_string();
        } else {
            return Err(AgentError::InvalidValue(format!(
                "Unexpected input pin '{}'",
                pin
            )));
        }

        let languages: Vec<String> = self
            .configs()?
            .get_string_or(CONFIG_LANGUAGES, "en")
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
        let lang_refs: Vec<&str> = languages.iter().map(|s| s.as_str()).collect();

        let api = YouTubeTranscriptApi::new(None, None, None).map_err(|e| {
            AgentError::IoError(format!("YouTubeTranscriptApi Initialization Error: {}", e))
        })?;
        let transcript = api
            .fetch_transcript(&video_id, &lang_refs, false)
            .await
            .map_err(|e| AgentError::IoError(format!("YouTube Transcript Fetch Error: {}", e)))?;

        let mut text = String::new();
        for snippet in &transcript.snippets {
            text.push_str(&snippet.text);
        }

        self.try_output(
            ctx.clone(),
            PORT_TRANSCRIPT,
            AgentValue::from_serialize(&transcript).map_err(|e| {
                AgentError::IoError(format!("Transcript Serialization Error: {}", e))
            })?,
        )?;

        self.try_output(ctx, PORT_TEXT, text.into())
    }
}
