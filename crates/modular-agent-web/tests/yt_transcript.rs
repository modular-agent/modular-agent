//! Network integration tests for the YouTube transcript fetcher.
//!
//! They talk to youtube.com and depend on a specific video keeping its
//! captions, so they are ignored by default:
//! `cargo test --all-features -- --ignored`
#![cfg(feature = "yt-transcript")]

use modular_agent_web::yt_transcript::{Transcript, fetch_transcript};
use reqwest::Client;

/// "Rick Astley - Never Gonna Give You Up": has both auto-generated English
/// captions and manually created tracks in several languages, including ja.
const VIDEO_ID: &str = "dQw4w9WgXcQ";

async fn fetch(language: &str) -> Transcript {
    let client = Client::builder()
        .build()
        .expect("failed to build HTTP client");
    let languages = vec![language.to_string()];
    fetch_transcript(&client, VIDEO_ID, &languages)
        .await
        .unwrap_or_else(|e| panic!("fetch_transcript({language}) failed: {e}"))
}

#[tokio::test]
#[ignore = "requires network access to youtube.com"]
async fn fetches_english_transcript() {
    let transcript = fetch("en").await;

    assert_eq!(transcript.video_id, VIDEO_ID);
    assert_eq!(transcript.language_code, "en");
    assert!(!transcript.snippets.is_empty());
    assert!(
        transcript
            .snippets
            .iter()
            .all(|snippet| !snippet.text.trim().is_empty())
    );
    assert!(
        transcript
            .snippets
            .iter()
            .any(|snippet| snippet.start > 0.0)
    );
    assert!(
        transcript
            .snippets
            .iter()
            .any(|snippet| snippet.duration > 0.0)
    );
}

#[tokio::test]
#[ignore = "requires network access to youtube.com"]
async fn selects_japanese_track_when_requested() {
    let transcript = fetch("ja").await;

    assert_eq!(transcript.language_code, "ja");
    assert!(!transcript.snippets.is_empty());
}
