use std::collections::VecDeque;

use serde::de::DeserializeOwned;
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::responses::ResponseResult;

const MAX_LINE_BYTES: usize = 1024 * 1024;

/// One event from a streamed call: text as it is generated, then one `Done` with the complete answer. A failure
/// arrives as `Error` instead of `Done`.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamEvent {
    Delta(String),
    Done(ResponseResult),
    Error { code: String, message: String },
}

/// A streamed call. Read it with [`ResponseStream::next`] until it returns `None`. Dropping it closes the connection;
/// the gateway still settles the call's usage.
pub struct ResponseStream {
    response: reqwest::Response,
    reader: SseReader,
    ready: VecDeque<StreamEvent>,
    finished: bool,
    terminal: bool,
}

impl ResponseStream {
    pub(crate) fn new(response: reqwest::Response) -> Self {
        Self {
            response,
            reader: SseReader::default(),
            ready: VecDeque::new(),
            finished: false,
            terminal: false,
        }
    }

    /// The next event, or `None` after the final `Done` or `Error`. A stream that closes before its final event
    /// yields an `Error` with the code `stream_ended`.
    pub async fn next(&mut self) -> Option<Result<StreamEvent>> {
        loop {
            if let Some(event) = self.ready.pop_front() {
                return Some(Ok(event));
            }
            if self.finished {
                return None;
            }
            match self.response.chunk().await {
                Ok(Some(bytes)) => {
                    if let Err(error) = self.feed(&bytes) {
                        self.finished = true;
                        return Some(Err(error));
                    }
                }
                Ok(None) => {
                    if !self.terminal {
                        self.ready.push_back(StreamEvent::Error {
                            code: "stream_ended".to_owned(),
                            message: "The stream ended before its final event.".to_owned(),
                        });
                        self.terminal = true;
                    }
                    self.finished = true;
                }
                Err(error) => {
                    self.finished = true;
                    return Some(Err(if error.is_timeout() {
                        Error::Timeout
                    } else {
                        Error::Network(error)
                    }));
                }
            }
        }
    }

    fn feed(&mut self, bytes: &[u8]) -> Result<()> {
        for record in self.reader.push(bytes)? {
            let Some(event) = decode(&record)? else {
                continue;
            };
            let final_event = !matches!(event, StreamEvent::Delta(_));
            self.ready.push_back(event);
            if final_event {
                self.terminal = true;
                self.finished = true;
                break;
            }
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct DeltaData {
    text: String,
}

#[derive(Deserialize)]
struct ErrorData {
    code: String,
    message: String,
}

/// Events the gateway does not send are ignored, so a newer gateway can add events without breaking this client.
fn decode(record: &Record) -> Result<Option<StreamEvent>> {
    Ok(match record.event.as_str() {
        "delta" => Some(StreamEvent::Delta(parse::<DeltaData>(&record.data)?.text)),
        "done" => Some(StreamEvent::Done(parse(&record.data)?)),
        "error" => {
            let error: ErrorData = parse(&record.data)?;
            Some(StreamEvent::Error {
                code: error.code,
                message: error.message,
            })
        }
        _ => None,
    })
}

fn parse<T: DeserializeOwned>(data: &str) -> Result<T> {
    serde_json::from_str(data).map_err(|error| Error::Decode(error.to_string()))
}

struct Record {
    event: String,
    data: String,
}

/// Splits Server-Sent Events into records. Lines may end in LF or CRLF and may arrive split across chunks; a record
/// ends at a blank line, and its `data:` lines are joined with newlines.
#[derive(Default)]
struct SseReader {
    line: Vec<u8>,
    event: Option<String>,
    data: Vec<String>,
}

impl SseReader {
    fn push(&mut self, bytes: &[u8]) -> Result<Vec<Record>> {
        let mut records = Vec::new();
        for &byte in bytes {
            if byte != b'\n' {
                self.line.push(byte);
                if self.line.len() > MAX_LINE_BYTES {
                    return Err(Error::Decode(
                        "a stream line is longer than 1 MB".to_owned(),
                    ));
                }
                continue;
            }
            let line = std::mem::take(&mut self.line);
            let line = line.strip_suffix(b"\r").unwrap_or(&line);
            if line.is_empty() {
                if self.event.is_some() || !self.data.is_empty() {
                    records.push(Record {
                        event: self.event.take().unwrap_or_else(|| "message".to_owned()),
                        data: self.data.join("\n"),
                    });
                    self.data.clear();
                }
            } else if let Some(value) = line.strip_prefix(b"event:") {
                self.event = Some(text(value)?);
            } else if let Some(value) = line.strip_prefix(b"data:") {
                self.data.push(text(value)?);
            }
        }
        Ok(records)
    }
}

fn text(value: &[u8]) -> Result<String> {
    let value = value.strip_prefix(b" ").unwrap_or(value);
    String::from_utf8(value.to_vec())
        .map_err(|_| Error::Decode("a stream line is not UTF-8".to_owned()))
}
