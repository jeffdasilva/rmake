// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Owned diagnostic text with lossless transport for Makefile bytes.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    pub file: Vec<u8>,
    pub line: usize,
}

/// An output emitted synchronously while reading or expanding make syntax.
/// The receiver controls I/O; byte slices remain valid for this callback only.
#[derive(Clone, Copy, Debug)]
pub enum Output<'a> {
    Info(&'a [u8]),
    Warning {
        location: Option<&'a Location>,
        message: &'a [u8],
    },
}

/// A filesystem operation requested by expansion. Filenames and contents are
/// raw Linux bytes; the runtime owns filesystem access and error formatting.
#[derive(Clone, Copy, Debug)]
pub enum FilesystemEffect<'a> {
    Wildcard(&'a [u8]),
    /// Account/environment fallback; make's nonempty HOME is expanded in core.
    Tilde(&'a [u8]),
    Realpath(&'a [u8]),
    ReadFile(&'a [u8]),
    WriteFile {
        path: &'a [u8],
        text: Option<&'a [u8]>,
        append: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilesystemError {
    Caller(Vec<u8>),
    Unlocated(Vec<u8>),
}

impl From<Vec<u8>> for FilesystemError {
    fn from(message: Vec<u8>) -> Self {
        Self::Caller(message)
    }
}

impl FilesystemError {
    pub fn into_message(self) -> Vec<u8> {
        match self {
            Self::Caller(message) | Self::Unlocated(message) => message,
        }
    }
}

type FilesystemReceiver<'a> =
    dyn FnMut(FilesystemEffect<'_>) -> Result<Vec<u8>, FilesystemError> + 'a;

/// Explicit caller context and output receiver. This is never stored in or
/// shared by cloned variable stores; each evaluation chooses its own receiver.
pub struct ExpansionContext<'a> {
    pub location: Option<&'a Location>,
    output: Option<&'a mut dyn FnMut(Output<'_>)>,
    filesystem: Option<&'a mut FilesystemReceiver<'a>>,
}

impl<'a> ExpansionContext<'a> {
    pub fn new(location: Option<&'a Location>, output: &'a mut dyn FnMut(Output<'_>)) -> Self {
        Self {
            location,
            output: Some(output),
            filesystem: None,
        }
    }

    /// Pure evaluation rejects output functions instead of silently losing them.
    pub fn pure() -> Self {
        Self {
            location: None,
            output: None,
            filesystem: None,
        }
    }

    pub fn with_filesystem(mut self, filesystem: &'a mut FilesystemReceiver<'a>) -> Self {
        self.filesystem = Some(filesystem);
        self
    }

    pub fn perform_filesystem(
        &mut self,
        effect: FilesystemEffect<'_>,
    ) -> Option<Result<Vec<u8>, FilesystemError>> {
        self.filesystem.as_mut().map(|receiver| receiver(effect))
    }

    pub fn emit(&mut self, output: Output<'_>) -> bool {
        if let Some(receiver) = &mut self.output {
            receiver(output);
            true
        } else {
            false
        }
    }

    pub fn at<'b>(&'b mut self, location: Option<&'b Location>) -> ExpansionContext<'b> {
        ExpansionContext {
            location,
            output: match &mut self.output {
                Some(output) => Some(&mut **output),
                None => None,
            },
            filesystem: match &mut self.filesystem {
                Some(filesystem) => Some(&mut **filesystem),
                None => None,
            },
        }
    }

    pub fn reborrow(&mut self) -> ExpansionContext<'_> {
        self.at(self.location)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Diagnostic {
    Text(String),
    Bytes(Vec<u8>),
}

impl Diagnostic {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Text(text) => text.as_bytes(),
            Self::Bytes(bytes) => bytes,
        }
    }
}

impl From<String> for Diagnostic {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for Diagnostic {
    fn from(value: &str) -> Self {
        Self::Text(value.into())
    }
}

impl From<Vec<u8>> for Diagnostic {
    fn from(value: Vec<u8>) -> Self {
        Self::Bytes(value)
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Formatting is for human-readable contexts only. CLI diagnostics must
        // write as_bytes() directly to retain arbitrary input bytes.
        output.write_str(&String::from_utf8_lossy(self.as_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::Diagnostic;

    #[test]
    fn raw_diagnostic_transport_does_not_use_lossy_display() {
        let message = Diagnostic::from(b"bad '\xff'".to_vec());
        assert_eq!(message.as_bytes(), b"bad '\xff'");
        assert_eq!(message.to_string(), "bad '\u{fffd}'");
        assert_eq!(Diagnostic::from("text").as_bytes(), b"text");
    }
}
