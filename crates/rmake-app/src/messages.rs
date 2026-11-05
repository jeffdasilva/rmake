// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva
//! System messages only. Raw user/foreign bytes bypass this formatter.
use rmake_core::diagnostic::Diagnostic;
use rmake_core::messages::{Message, MessageRef};
use std::borrow::Cow;
pub fn try_format(message: MessageRef<'_>) -> Result<Vec<u8>, Vec<u8>> {
    rmake_linux::messages::render_ref(message)
        .map_err(|error| format!("invalid localized system message format: {error:?}").into_bytes())
}
pub fn system(message: Message<'_>) -> Vec<u8> {
    match try_format(MessageRef::Borrowed(message)) {
        Ok(bytes) | Err(bytes) => bytes,
    }
}
pub fn diagnostic(message: &Diagnostic) -> Cow<'_, [u8]> {
    match message.system_message() {
        Some(message) => Cow::Owned(match try_format(MessageRef::Owned(message)) {
            Ok(bytes) | Err(bytes) => bytes,
        }),
        None => Cow::Borrowed(message.as_bytes()),
    }
}
pub fn stop() -> Vec<u8> {
    system(Message::Stop)
}

pub fn stop_for(message: &Diagnostic) -> Cow<'_, [u8]> {
    message
        .frozen_stop()
        .map_or_else(|| Cow::Owned(stop()), Cow::Borrowed)
}
