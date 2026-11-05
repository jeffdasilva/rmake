// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

#[path = "../src/debug_messages.rs"]
mod debug_messages;

// Exercise the actual build-time reader with independently constructed MO data.
use rmake_core::messages as system_messages;
#[allow(dead_code)]
#[path = "../catalog_validation.rs"]
mod catalog_validation;

fn one_message(original: &[u8], translated: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    for word in [
        0x950412de_u32,
        0,
        1,
        28,
        36,
        0,
        0,
        original.len() as u32,
        44,
        translated.len() as u32,
        45 + original.len() as u32,
    ] {
        output.extend_from_slice(&word.to_le_bytes());
    }
    output.extend_from_slice(original);
    output.push(0);
    output.extend_from_slice(translated);
    output.push(0);
    output
}

#[test]
fn build_reader_checks_the_catalog_schema_before_runtime() {
    let id = system_messages::MessageId::UndefinedVariable
        .msgid()
        .to_bytes();
    let valid = one_message(id, b"name=%2$.*1$s/%2$.*1$s");
    catalog_validation::validate_bytes(&valid).unwrap();
    let omitted = one_message(id, b"literal without arguments");
    catalog_validation::validate_bytes(&omitted).unwrap();
    let invalid = one_message(id, b"wrong type=%2$d");
    assert!(
        catalog_validation::validate_bytes(&invalid)
            .unwrap_err()
            .contains("ArgumentType")
    );
    assert!(catalog_validation::validate_bytes(&valid[..valid.len() - 1]).is_err());
}

#[test]
fn build_reader_checks_debug_keys_with_reordered_byte_precision() {
    let id = debug_messages::TRYING_RULE.msgid.to_bytes();
    let valid = one_message(id, b"stem=%3$.*2$s rule=%1$s");
    catalog_validation::validate_bytes(&valid).unwrap();
    let invalid = one_message(id, b"stem=%3$.*1$s");
    assert!(
        catalog_validation::validate_bytes(&invalid)
            .unwrap_err()
            .contains("ArgumentType")
    );
}
