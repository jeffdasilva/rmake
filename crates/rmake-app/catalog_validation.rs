// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva
//! Build-only reader for the public MO table format, independent of libc internals.
use crate::system_messages::MessageId;
use std::path::Path;

pub fn validate(path: &Path) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    validate_bytes(&bytes)
}

pub fn validate_bytes(bytes: &[u8]) -> Result<(), String> {
    let little = match bytes.get(..4) {
        Some([0xde, 0x12, 0x04, 0x95]) => true,
        Some([0x95, 0x04, 0x12, 0xde]) => false,
        _ => return Err("invalid message-catalog magic".into()),
    };
    let word = |offset: usize| -> Result<usize, String> {
        let end = offset.checked_add(4).ok_or("catalog word overflow")?;
        let raw: [u8; 4] = bytes
            .get(offset..end)
            .ok_or("truncated message-catalog word")?
            .try_into()
            .map_err(|_| "truncated word")?;
        Ok((if little {
            u32::from_le_bytes(raw)
        } else {
            u32::from_be_bytes(raw)
        }) as usize)
    };
    if word(4)? >> 16 != 0 {
        return Err("unsupported owned message-catalog revision".into());
    }
    let count = word(8)?;
    let originals = word(12)?;
    let translations = word(16)?;
    let entry = |table: usize, index: usize| -> Result<&[u8], String> {
        let row = index
            .checked_mul(8)
            .and_then(|value| table.checked_add(value))
            .ok_or("catalog offset overflow")?;
        let length = word(row)?;
        let offset = word(row.checked_add(4).ok_or("catalog offset overflow")?)?;
        let end = offset
            .checked_add(length)
            .ok_or("catalog string overflow")?;
        if bytes.get(end) != Some(&0) {
            return Err("unterminated catalog string".into());
        }
        bytes
            .get(offset..end)
            .ok_or_else(|| "truncated catalog string".into())
    };
    for id in MessageId::ALL {
        let mut template = id.msgid().to_bytes();
        for index in 0..count {
            if entry(originals, index)? == template {
                let translated = entry(translations, index)?;
                if !translated.is_empty() {
                    template = translated;
                }
                break;
            }
        }
        id.sample()
            .render(template)
            .map_err(|error| format!("invalid {:?} translation schema: {error:?}", id))?;
    }
    Ok(())
}
