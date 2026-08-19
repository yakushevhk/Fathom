use thiserror::Error;

#[derive(Debug, Error)]
pub enum PrError {
    #[error("LLM error: {0}")]
    Llm(String),
    #[error("Tool error: {0}")]
    Tool(String),
    #[error("Agent error: {0}")]
    Agent(String),
    #[error("Persistence error: {0}")]
    Persistence(String),
    #[error("Config error: {0}")]
    Config(String),
    #[error("Timeout after {0}s")]
    Timeout(u64),
    #[error("Max depth reached ({0})")]
    MaxDepthReached(u32),
    #[error("Max agents reached ({0})")]
    MaxAgentsReached(u32),
    #[error("Max iterations reached ({0})")]
    MaxIterationsReached(u32),
    #[error("Cancelled")]
    Cancelled,
    /// HTTP error from an LLM API: carries the status so retry logic can
    /// distinguish retryable (408/429/5xx) from permanent (4xx) failures.
    #[error("API error {status}: {message}")]
    Http {
        status: u16,
        message: String,
        /// Seconds from the Retry-After header, when the server sent one.
        retry_after: Option<u64>,
    },
    /// Response exceeded the buffering threshold; the caller should retry
    /// via streaming. Never retried as a normal request.
    #[error("response too large, streaming required: {0}")]
    ResponseTooLarge(String),
}

impl PrError {
    /// Whether the operation that produced this error is worth retrying.
    /// Permanent failures (auth, bad request, oversized response) are not.
    pub fn is_retryable(&self) -> bool {
        match self {
            PrError::Http { status, .. } => {
                *status == 408 || *status == 429 || *status >= 500
            }
            // Network-ish failures surface as PrError::Llm from the transport.
            PrError::Llm(_) | PrError::Timeout(_) => true,
            _ => false,
        }
    }

    /// Server-requested backoff (Retry-After), if present.
    pub fn retry_after_secs(&self) -> Option<u64> {
        match self {
            PrError::Http { retry_after, .. } => *retry_after,
            _ => None,
        }
    }
}

pub type PrResult<T> = Result<T, PrError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_llm() {
        let e = PrError::Llm("bad response".into());
        assert_eq!(e.to_string(), "LLM error: bad response");
    }

    #[test]
    fn display_http() {
        let e = PrError::Http { status: 429, message: "Too Many Requests".into(), retry_after: Some(60) };
        assert_eq!(e.to_string(), "API error 429: Too Many Requests");
    }

    #[test]
    fn display_timeout() {
        assert_eq!(PrError::Timeout(30).to_string(), "Timeout after 30s");
    }

    #[test]
    fn display_max_depth() {
        assert_eq!(PrError::MaxDepthReached(5).to_string(), "Max depth reached (5)");
    }

    #[test]
    fn display_cancelled() {
        assert_eq!(PrError::Cancelled.to_string(), "Cancelled");
    }

    #[test]
    fn is_retryable_http_429() {
        assert!(PrError::Http { status: 429, message: "".into(), retry_after: None }.is_retryable());
    }

    #[test]
    fn is_retryable_http_500() {
        assert!(PrError::Http { status: 500, message: "".into(), retry_after: None }.is_retryable());
    }

    #[test]
    fn is_retryable_http_408() {
        assert!(PrError::Http { status: 408, message: "".into(), retry_after: None }.is_retryable());
    }

    #[test]
    fn not_retryable_http_400() {
        assert!(!PrError::Http { status: 400, message: "".into(), retry_after: None }.is_retryable());
    }

    #[test]
    fn not_retryable_http_403() {
        assert!(!PrError::Http { status: 403, message: "".into(), retry_after: None }.is_retryable());
    }

    #[test]
    fn is_retryable_llm() {
        assert!(PrError::Llm("network error".into()).is_retryable());
    }

    #[test]
    fn is_retryable_timeout() {
        assert!(PrError::Timeout(60).is_retryable());
    }

    #[test]
    fn not_retryable_tool() {
        assert!(!PrError::Tool("bad input".into()).is_retryable());
    }

    #[test]
    fn not_retryable_agent() {
        assert!(!PrError::Agent("crash".into()).is_retryable());
    }

    #[test]
    fn not_retryable_cancelled() {
        assert!(!PrError::Cancelled.is_retryable());
    }

    #[test]
    fn not_retryable_max_depth() {
        assert!(!PrError::MaxDepthReached(3).is_retryable());
    }

    #[test]
    fn not_retryable_response_too_large() {
        assert!(!PrError::ResponseTooLarge("10MB".into()).is_retryable());
    }

    #[test]
    fn retry_after_http() {
        let e = PrError::Http { status: 429, message: "".into(), retry_after: Some(120) };
        assert_eq!(e.retry_after_secs(), Some(120));
    }

    #[test]
    fn retry_after_none_for_non_http() {
        assert_eq!(PrError::Llm("x".into()).retry_after_secs(), None);
        assert_eq!(PrError::Timeout(10).retry_after_secs(), None);
        assert_eq!(PrError::Cancelled.retry_after_secs(), None);
    }

    #[test]
    fn pr_result_type() {
        let ok: PrResult<i32> = Ok(42);
        assert!(ok.is_ok());
        let err: PrResult<i32> = Err(PrError::Cancelled);
        assert!(err.is_err());
    }
}
