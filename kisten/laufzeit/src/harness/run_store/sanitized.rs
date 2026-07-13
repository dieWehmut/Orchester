use orchester_protokoll::HarnessEventKind;

use super::{EventAppend, StoreError};
use crate::harness::feedback::FeedbackEngine;

const MAX_MODEL_TEXT_BYTES: usize = 65_536;

pub(super) fn canonicalize_input(
    input: EventAppend,
    sanitizer: &FeedbackEngine,
) -> Result<EventAppend, StoreError> {
    let kind = match input.kind {
        HarnessEventKind::ModelCompleted { assistant_text } => {
            let assistant_text = sanitizer.sanitize_text(&assistant_text);
            if assistant_text.len() > MAX_MODEL_TEXT_BYTES {
                return Err(StoreError::Invariant(
                    "model completion text exceeds the durable limit".into(),
                ));
            }
            HarnessEventKind::ModelCompleted { assistant_text }
        }
        kind => kind,
    };
    Ok(EventAppend { kind, ..input })
}
