//! One transient-failure policy shared by every wire adapter.
//!
//! Each wire decides what a status *means*; whether a status is worth trying
//! again is a transport property, so the policy lives here instead of being
//! copied into every model.

use std::future::Future;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use super::{HttpRequest, HttpResponse, HttpResponseStream, HttpTransportError};

const MAX_TRANSIENT_TRANSPORT_ATTEMPTS: usize = 2;
const TRANSIENT_TRANSPORT_RETRY_DELAY: Duration = Duration::from_millis(100);

/// A transport reply whose status decides whether another attempt is worthwhile.
pub(super) trait RetryableReply {
    fn reply_status(&self) -> u16;
}

impl RetryableReply for HttpResponse {
    fn reply_status(&self) -> u16 {
        self.status()
    }
}

impl RetryableReply for HttpResponseStream {
    fn reply_status(&self) -> u16 {
        self.status()
    }
}

/// Send a request, retrying only failures that a second attempt can fix.
///
/// A retryable *status* is returned rather than raised so the calling wire
/// still maps it into its own error vocabulary once the attempts are spent.
pub(super) async fn send_with_retry<R, S, F>(
    request: &HttpRequest,
    cancel: &CancellationToken,
    mut send: S,
) -> Result<R, HttpTransportError>
where
    R: RetryableReply,
    S: FnMut(HttpRequest, CancellationToken) -> F,
    F: Future<Output = Result<R, HttpTransportError>>,
{
    let mut attempt = 1;
    loop {
        if cancel.is_cancelled() {
            return Err(HttpTransportError::Cancelled);
        }
        let outcome = send(request.clone(), cancel.clone()).await;
        let retryable = match &outcome {
            Ok(reply) => retryable_status(reply.reply_status()),
            Err(error) => retryable_error(*error),
        };
        if !retryable || attempt >= MAX_TRANSIENT_TRANSPORT_ATTEMPTS {
            return outcome;
        }
        attempt += 1;
        tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(HttpTransportError::Cancelled),
            _ = tokio::time::sleep(TRANSIENT_TRANSPORT_RETRY_DELAY) => {}
        }
    }
}

fn retryable_status(status: u16) -> bool {
    matches!(status, 408 | 425 | 500..=599)
}

fn retryable_error(error: HttpTransportError) -> bool {
    matches!(
        error,
        HttpTransportError::Timeout | HttpTransportError::Transport
    )
}
