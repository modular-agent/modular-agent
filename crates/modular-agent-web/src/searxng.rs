use std::time::Duration;

use modular_agent_core::{
    Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    ModularAgent, async_trait, modular_agent,
};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};

static CATEGORY: &str = "Web";

static PORT_QUERY: &str = "query";
static PORT_RESULTS: &str = "results";

static CONFIG_MAX_RESULTS: &str = "max_results";
static CONFIG_CATEGORIES: &str = "categories";
static CONFIG_LANGUAGE: &str = "language";
static CONFIG_TIME_RANGE: &str = "time_range";
static CONFIG_SAFESEARCH: &str = "safesearch";
static CONFIG_SEARXNG_URL: &str = "searxng_url";

static DEFAULT_MAX_RESULTS: i64 = 10;
static DEFAULT_SAFESEARCH: i64 = -1;

static REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Search parameters accepted on the `query` port when the input is an object.
/// Every field except `query` overrides the corresponding config default.
#[derive(Debug, Default, Deserialize)]
struct SearchRequest {
    query: String,
    categories: Option<String>,
    language: Option<String>,
    time_range: Option<String>,
    safesearch: Option<i64>,
    pageno: Option<i64>,
    max_results: Option<i64>,
}

/// One entry of the SearXNG `results` array.
///
/// SearXNG spells the publication date `publishedDate`; it is accepted as an
/// alias so the emitted value keeps the snake_case name used everywhere else.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct SearchResult {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    engine: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    score: Option<f64>,
    #[serde(
        default,
        alias = "publishedDate",
        skip_serializing_if = "Option::is_none"
    )]
    published_date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    results: Vec<SearchResult>,
}

/// Parse a SearXNG JSON search response and truncate it to `max_results`.
/// A `max_results` of 0 or less means unlimited.
fn parse_search_results(body: &str, max_results: i64) -> Result<Vec<SearchResult>, AgentError> {
    let response: SearchResponse = serde_json::from_str(body)
        .map_err(|e| AgentError::IoError(format!("SearXNG response parse error: {}", e)))?;
    let mut results = response.results;
    if max_results > 0 {
        results.truncate(max_results as usize);
    }
    Ok(results)
}

/// Read the SearXNG base URL from the global configs, without a trailing slash.
fn get_searxng_url(ma: &ModularAgent) -> Result<String, AgentError> {
    ma.get_global_configs(SearxngSearchAgent::DEF_NAME)
        .and_then(|cfg| cfg.get_string(CONFIG_SEARXNG_URL).ok())
        .map(|url| url.trim().trim_end_matches('/').to_string())
        .filter(|url| !url.is_empty())
        .ok_or_else(|| {
            AgentError::InvalidConfig(
                "SearXNG URL is not set. Configure it in global settings.".to_string(),
            )
        })
}

/// Search the web through a SearXNG instance's JSON API.
///
/// Sends `GET {searxng_url}/search?q=...&format=json` and emits the parsed
/// `results` array. The instance must have `json` enabled in `search.formats`
/// (`settings.yml`); otherwise it answers 403 and the agent reports that hint.
///
/// The `query` input takes either a plain string or an object. Object fields
/// override the config defaults for that single search; unset parameters are
/// omitted from the request so the instance's own defaults apply.
///
/// # Ports
/// - Input `query`: Search query string, or an object
///   `{ query, categories?, language?, time_range?, safesearch?, pageno?, max_results? }`
/// - Output `results`: Array of `{ title, url, content?, engine?, score?, published_date? }`
///
/// # Configuration
/// - `max_results`: Maximum number of results to emit, applied client-side
///   (default: 10, 0 = unlimited)
/// - `categories`: Comma-separated SearXNG categories, e.g. `general,news`
///   (default: "", instance default)
/// - `language`: Language code, e.g. `en` or `ja` (default: "", instance default)
/// - `time_range`: `day`, `month`, or `year` (default: "", no time filter)
/// - `safesearch`: 0 (off), 1 (moderate), or 2 (strict)
///   (default: -1, instance default)
///
/// # Global Configuration
/// - `searxng_url`: Base URL of the SearXNG instance, e.g.
///   `http://localhost:8080`. Required; searching without it fails.
///
/// # Example
/// Input `"rust async"` searches with the configured defaults. Input
/// `{"query": "rust async", "language": "ja", "pageno": 2, "max_results": 3}`
/// searches Japanese results, second page, and emits at most 3 results.
#[modular_agent(
    title = "SearXNG Search",
    category = CATEGORY,
    inputs = [PORT_QUERY],
    outputs = [PORT_RESULTS],
    integer_config(
        name = CONFIG_MAX_RESULTS,
        default = DEFAULT_MAX_RESULTS,
        description = "Maximum number of results to emit (0 = unlimited)",
    ),
    string_config(
        name = CONFIG_CATEGORIES,
        default = "",
        description = "Comma-separated categories, e.g. general,news (empty = instance default)",
    ),
    string_config(
        name = CONFIG_LANGUAGE,
        default = "",
        description = "Language code, e.g. en or ja (empty = instance default)",
    ),
    string_config(
        name = CONFIG_TIME_RANGE,
        default = "",
        description = "Time filter: day, month, or year (empty = no filter)",
    ),
    integer_config(
        name = CONFIG_SAFESEARCH,
        default = DEFAULT_SAFESEARCH,
        description = "SafeSearch level: 0 off, 1 moderate, 2 strict (-1 = instance default)",
    ),
    string_global_config(
        name = CONFIG_SEARXNG_URL,
        default = "",
        title = "SearXNG URL",
        description = "Base URL of the SearXNG instance, e.g. http://localhost:8080",
    ),
    hint(width = 1, height = 2),
)]
struct SearxngSearchAgent {
    data: AgentData,
    client: Client,
}

#[async_trait]
impl AsAgent for SearxngSearchAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|e| AgentError::IoError(format!("SearXNG client build error: {}", e)))?;
        Ok(Self {
            data: AgentData::new(ma, id, spec),
            client,
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        value: AgentValue,
    ) -> Result<(), AgentError> {
        let request = if let Some(query) = value.as_str() {
            SearchRequest {
                query: query.to_string(),
                ..Default::default()
            }
        } else if value.is_object() {
            value.to_deserialize::<SearchRequest>()?
        } else {
            return Err(AgentError::InvalidValue(
                "Input value for 'query' must be a string or an object".to_string(),
            ));
        };

        if request.query.trim().is_empty() {
            return Err(AgentError::InvalidValue(
                "Search query is empty".to_string(),
            ));
        }

        let config = self.configs()?;
        let max_results = request
            .max_results
            .unwrap_or_else(|| config.get_integer_or(CONFIG_MAX_RESULTS, DEFAULT_MAX_RESULTS));
        let categories = request
            .categories
            .unwrap_or_else(|| config.get_string_or_default(CONFIG_CATEGORIES));
        let language = request
            .language
            .unwrap_or_else(|| config.get_string_or_default(CONFIG_LANGUAGE));
        let time_range = request
            .time_range
            .unwrap_or_else(|| config.get_string_or_default(CONFIG_TIME_RANGE));
        let safesearch = request
            .safesearch
            .unwrap_or_else(|| config.get_integer_or(CONFIG_SAFESEARCH, DEFAULT_SAFESEARCH));

        let base_url = get_searxng_url(self.ma())?;
        let search_url = format!("{}/search", base_url);

        let mut params: Vec<(&str, String)> =
            vec![("q", request.query), ("format", "json".to_string())];
        if !categories.is_empty() {
            params.push(("categories", categories));
        }
        if !language.is_empty() {
            params.push(("language", language));
        }
        if !time_range.is_empty() {
            params.push(("time_range", time_range));
        }
        if safesearch >= 0 {
            params.push(("safesearch", safesearch.to_string()));
        }
        if let Some(pageno) = request.pageno {
            params.push(("pageno", pageno.to_string()));
        }

        let response = self
            .client
            .get(&search_url)
            .query(&params)
            .send()
            .await
            .map_err(|e| {
                if e.is_connect() {
                    AgentError::IoError(format!("SearXNG is not reachable at {}: {}", base_url, e))
                } else {
                    AgentError::IoError(format!("SearXNG request error: {}", e))
                }
            })?;

        if response.status() == StatusCode::FORBIDDEN {
            return Err(AgentError::IoError(format!(
                "SearXNG at {} returned 403 Forbidden. The JSON API is likely disabled; add 'json' to 'search.formats' in the instance settings.yml",
                base_url
            )));
        }
        let response = response
            .error_for_status()
            .map_err(|e| AgentError::IoError(format!("SearXNG search failed: {}", e)))?;

        let body = response
            .text()
            .await
            .map_err(|e| AgentError::IoError(format!("SearXNG response read error: {}", e)))?;

        let results = parse_search_results(&body, max_results)?;

        self.output(ctx, PORT_RESULTS, AgentValue::from_serialize(&results)?)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trimmed-down but field-accurate response from a real SearXNG instance.
    const FIXTURE: &str = r#"{
      "query": "rust async",
      "number_of_results": 0,
      "results": [
        {
          "url": "https://rust-lang.github.io/async-book/",
          "title": "Asynchronous Programming in Rust",
          "content": "This book aims to be a comprehensive, up-to-date guide.",
          "engine": "duckduckgo",
          "parsed_url": ["https", "rust-lang.github.io", "/async-book/", "", "", ""],
          "template": "default.html",
          "engines": ["duckduckgo", "google"],
          "positions": [1, 2],
          "publishedDate": "2024-03-11T00:00:00",
          "score": 4.0,
          "category": "general"
        },
        {
          "url": "https://tokio.rs/",
          "title": "Tokio - An asynchronous Rust runtime",
          "content": null,
          "engine": "google",
          "parsed_url": ["https", "tokio.rs", "/", "", "", ""],
          "template": "default.html",
          "engines": ["google"],
          "positions": [3],
          "score": 2.0,
          "category": "general"
        },
        {
          "url": "https://docs.rs/futures/latest/futures/",
          "title": "futures - Rust",
          "content": "Abstractions for asynchronous programming.",
          "engine": "brave",
          "parsed_url": ["https", "docs.rs", "/futures/latest/futures/", "", "", ""],
          "template": "default.html",
          "engines": ["brave"],
          "positions": [4],
          "score": 1.0,
          "category": "general"
        }
      ],
      "answers": [],
      "corrections": [],
      "infoboxes": [],
      "suggestions": ["rust async book"],
      "unresponsive_engines": []
    }"#;

    #[test]
    fn parses_all_result_fields() {
        let results = parse_search_results(FIXTURE, 0).unwrap();
        assert_eq!(results.len(), 3);

        let first = &results[0];
        assert_eq!(first.title, "Asynchronous Programming in Rust");
        assert_eq!(first.url, "https://rust-lang.github.io/async-book/");
        assert_eq!(
            first.content.as_deref(),
            Some("This book aims to be a comprehensive, up-to-date guide.")
        );
        assert_eq!(first.engine.as_deref(), Some("duckduckgo"));
        assert_eq!(first.score, Some(4.0));
        assert_eq!(first.published_date.as_deref(), Some("2024-03-11T00:00:00"));

        // Missing / null optional fields stay None
        assert_eq!(results[1].content, None);
        assert_eq!(results[1].published_date, None);
    }

    #[test]
    fn truncates_to_max_results() {
        let results = parse_search_results(FIXTURE, 2).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[1].url, "https://tokio.rs/");
    }

    #[test]
    fn max_results_larger_than_available_keeps_all() {
        let results = parse_search_results(FIXTURE, 100).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn missing_results_key_yields_empty() {
        let results = parse_search_results(r#"{"query": "x", "answers": []}"#, 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn invalid_json_is_an_error() {
        let err = parse_search_results("<html>403 Forbidden</html>", 10).unwrap_err();
        assert!(matches!(err, AgentError::IoError(_)));
    }

    #[test]
    fn serializes_without_unset_optional_fields() {
        let results = parse_search_results(FIXTURE, 0).unwrap();
        let json = serde_json::to_value(&results[1]).unwrap();
        let obj = json.as_object().unwrap();
        assert_eq!(obj.get("url").unwrap(), "https://tokio.rs/");
        assert!(!obj.contains_key("content"));
        assert!(!obj.contains_key("published_date"));

        let json = serde_json::to_value(&results[0]).unwrap();
        let obj = json.as_object().unwrap();
        assert_eq!(obj.get("published_date").unwrap(), "2024-03-11T00:00:00");
    }

    #[test]
    fn request_object_parses_optional_overrides() {
        let request: SearchRequest =
            serde_json::from_str(r#"{"query": "rust", "language": "ja", "pageno": 2}"#).unwrap();
        assert_eq!(request.query, "rust");
        assert_eq!(request.language.as_deref(), Some("ja"));
        assert_eq!(request.pageno, Some(2));
        assert_eq!(request.categories, None);
        assert_eq!(request.max_results, None);
        assert_eq!(request.safesearch, None);
    }
}
