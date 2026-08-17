//! Server-Sent Events framing shared by streaming wire protocols.
//!
//! Framing is transport concern, not provider vocabulary: both the OpenAI
//! Responses stream and the Anthropic Messages stream are `event:`/`data:`
//! frames separated by a blank line, and only the payload names differ.

/// A frame rejected before its payload reaches a wire decoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MalformedFrame;

/// One parsed frame: its optional event name and its concatenated data lines.
pub(super) struct EventFrame<'a> {
    pub(super) name: Option<&'a str>,
    pub(super) data: String,
}

/// Return the length of the prefix of `bytes` that holds one complete frame,
/// including its terminating blank line.
pub(super) fn find_event_boundary(bytes: &[u8]) -> Option<usize> {
    let lf = bytes
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|index| index + 2);
    let crlf = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4);
    match (lf, crlf) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(index), None) | (None, Some(index)) => Some(index),
        (None, None) => None,
    }
}

/// The length of the blank-line terminator at the end of a frame.
pub(super) fn boundary_length(bytes: &[u8]) -> usize {
    if bytes.ends_with(b"\r\n\r\n") {
        4
    } else {
        2
    }
}

/// Parse one frame. `Ok(None)` means the frame carried no data lines, which a
/// server may send as a comment or keep-alive.
pub(super) fn parse_event_frame(frame: &[u8]) -> Result<Option<EventFrame<'_>>, MalformedFrame> {
    let frame = std::str::from_utf8(frame).map_err(|_| MalformedFrame)?;
    let mut name = None;
    let mut data = String::new();
    for line in frame.lines() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.starts_with(':') {
            continue;
        }
        if let Some(value) = line.strip_prefix("event:") {
            name = Some(value.trim());
        } else if let Some(value) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(value.trim_start());
        } else if !line.trim().is_empty() {
            return Err(MalformedFrame);
        }
    }
    if data.is_empty() {
        return Ok(None);
    }
    Ok(Some(EventFrame { name, data }))
}
