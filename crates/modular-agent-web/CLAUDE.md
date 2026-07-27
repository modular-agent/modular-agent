# CLAUDE.md

See root CLAUDE.md for common agent development patterns.

## Overview

Web automation agents for HTTP requests, HTML scraping, web search, and content extraction.

## Agents

| Agent | Purpose | Inputs | Outputs |
| ----- | ------- | ------ | ------- |
| FetchUrlAgent | HTTP GET request | `url` | `text` |
| HtmlScraperAgent | CSS selector extraction | `html` | `html[]` |
| HtmlToMarkdownAgent | HTML to Markdown conversion | `html` | `markdown` |
| SearxngSearchAgent | Web search via SearXNG JSON API | `query` | `results` |
| FetchYtTranscriptAgent | YouTube transcript extraction | `url` or `video_id` | `transcript`, `text` |

## Features

- `fetch-url` - HTTP requests (reqwest)
- `html-scraper` - CSS selectors (scraper crate)
- `html-to-markdown` - Markdown conversion
- `searxng` - SearXNG search (reqwest)
- `yt-transcript` - YouTube transcripts via the InnerTube player API (reqwest + quick-xml)
- All enabled by default

## Common Pipelines

```text
FetchUrl → HtmlScraper → HtmlToMarkdown
FetchUrl → HtmlToMarkdown
SearxngSearch → FetchUrl → HtmlToMarkdown
FetchYtTranscript → [Process transcript]
```

## HtmlScraperAgent

- Input: Single HTML string or array of HTML strings
- Config: `selector` - CSS selector pattern
- Output: Array of matched HTML elements

## HtmlToMarkdownAgent

Conversion options:

- Aggressive preprocessing enabled
- Removes navigation and forms
- Handles malformed HTML gracefully

## SearxngSearchAgent

Calls `GET {searxng_url}/search?q=...&format=json` on a self-hosted SearXNG
instance. The instance must list `json` under `search.formats` in its
`settings.yml`; otherwise it answers 403 and the agent reports that hint.

Global config: `searxng_url` - Base URL of the instance (default: empty, required).
A trailing slash is trimmed before the request URL is built.

Input `query`:

- String - used as the search query
- Object - `{ query, categories?, language?, time_range?, safesearch?, pageno?, max_results? }`.
  Each field overrides the matching config for that request; `pageno` is
  input-only (no config).

Configs are the defaults used when the request object omits a field:

| Config | Default | Sent when |
| ------ | ------- | --------- |
| `max_results` | 10 | never sent - results are truncated client-side (0 = unlimited) |
| `categories` | "" | non-empty |
| `language` | "" | non-empty |
| `time_range` | "" | non-empty |
| `safesearch` | -1 | 0-2 (-1 leaves it to the instance) |

Output `results` - Array of `{ title, url, content?, engine?, score?, published_date? }`.
SearXNG's `publishedDate` is renamed to `published_date`; unset optional fields
are omitted.

## FetchYtTranscriptAgent

Implemented directly against YouTube's InnerTube API: `POST
https://www.youtube.com/youtubei/v1/player` with the `ANDROID` client (the
`WEB` client is rejected without a PoToken), then a GET of the selected
caption track's `baseUrl`. The timedtext parser handles both srv3
(`<p t d>` with nested `<s>`) and the legacy `<text start dur>` format.

Track selection: the `languages` list is scanned in order; within each
language a manually created track is preferred over an auto-generated (`asr`)
one. An earlier language wins even when it only has an auto-generated track
(same order as youtube-transcript-api).

The fetch logic lives in `pub async fn yt_transcript::fetch_transcript`,
separate from the agent, and is covered by the network integration tests in
`tests/yt_transcript.rs` (`#[ignore]`d; run with
`cargo test --all-features -- --ignored`).

Supported URL formats:

- `youtu.be/<video_id>`
- `youtube.com/watch?v=<video_id>`
- `www.youtube.com/watch?v=<video_id>`

Config: `languages` - Comma-separated language codes (default: "en")

Outputs:

- `transcript` - Full transcript object with snippets
- `text` - Plain text concatenation
