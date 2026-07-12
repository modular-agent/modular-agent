//! Shared helpers for mapping provider HTTP errors to `AgentError` variants.

use std::time::Duration;

/// Parse a `Retry-After` header value as integer seconds.
///
/// HTTP-date format is not supported and yields `None`.
pub(crate) fn parse_retry_after(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    headers
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
        .map(Duration::from_secs)
}

/// Throttling-related wording that must never be classified as context
/// overflow, even when the body also matches an overflow pattern.
const RATE_LIMIT_PATTERNS: &[&str] = &[
    "rate limit",
    "rate_limit",
    "too many requests",
    "throttl",
    "quota",
];

/// Returns `true` when the (lowercased) body contains throttling wording.
pub(crate) fn mentions_rate_limit(lower_body: &str) -> bool {
    RATE_LIMIT_PATTERNS.iter().any(|p| lower_body.contains(p))
}

/// Quota/billing-exhaustion wording indicating a permanent condition; a 429
/// carrying it must not be classified as retryable.
const QUOTA_EXHAUSTED_PATTERNS: &[&str] = &[
    "insufficient_quota",
    "insufficient quota",
    "exceeded your current quota",
    "billing",
    "credit balance",
];

/// Returns `true` when the (lowercased) body contains quota/billing-exhaustion
/// wording.
pub(crate) fn mentions_quota_exhausted(lower_body: &str) -> bool {
    QUOTA_EXHAUSTED_PATTERNS.iter().any(|p| lower_body.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderValue, RETRY_AFTER};

    #[test]
    fn test_parse_retry_after_integer_seconds() {
        let mut headers = HeaderMap::new();
        headers.insert(RETRY_AFTER, HeaderValue::from_static("30"));
        assert_eq!(parse_retry_after(&headers), Some(Duration::from_secs(30)));
    }

    #[test]
    fn test_parse_retry_after_with_whitespace() {
        let mut headers = HeaderMap::new();
        headers.insert(RETRY_AFTER, HeaderValue::from_static(" 5 "));
        assert_eq!(parse_retry_after(&headers), Some(Duration::from_secs(5)));
    }

    #[test]
    fn test_parse_retry_after_missing() {
        let headers = HeaderMap::new();
        assert_eq!(parse_retry_after(&headers), None);
    }

    #[test]
    fn test_parse_retry_after_http_date_is_none() {
        let mut headers = HeaderMap::new();
        headers.insert(
            RETRY_AFTER,
            HeaderValue::from_static("Wed, 21 Oct 2026 07:28:00 GMT"),
        );
        assert_eq!(parse_retry_after(&headers), None);
    }

    #[test]
    fn test_parse_retry_after_invalid_value_is_none() {
        let mut headers = HeaderMap::new();
        headers.insert(RETRY_AFTER, HeaderValue::from_static("soon"));
        assert_eq!(parse_retry_after(&headers), None);
    }

    #[test]
    fn test_mentions_rate_limit() {
        assert!(mentions_rate_limit("you hit a rate limit"));
        assert!(mentions_rate_limit("error: rate_limit_error"));
        assert!(mentions_rate_limit("too many requests"));
        assert!(mentions_rate_limit("request was throttled"));
        assert!(mentions_rate_limit("insufficient quota"));
        assert!(!mentions_rate_limit("prompt is too long"));
    }

    #[test]
    fn test_mentions_quota_exhausted() {
        assert!(mentions_quota_exhausted("error: insufficient_quota"));
        assert!(mentions_quota_exhausted(
            "you exceeded your current quota, please check your plan and billing details"
        ));
        assert!(mentions_quota_exhausted("your credit balance is too low"));
        assert!(!mentions_quota_exhausted("rate limit exceeded"));
    }
}
