// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Startup binding serialization and the post-read environment-origin replay.

use rmake_core::diagnostic::ExpansionContext;
use rmake_core::parser::assignment;
use rmake_core::variables::{Assignment, ExpandError, Flavor, MAX_OUTPUT, Origin, VariableStore};

const SNAPSHOT: &[u8] = b"-*-command-variables-*-";

pub(crate) fn quote(output: &mut Vec<u8>, input: &[u8]) -> Result<(), ExpandError> {
    for &byte in input {
        let escape = match byte {
            b'$' => Some(b'$'),
            b' ' | b'\t' | b'\\' => Some(b'\\'),
            _ => None,
        };
        if output.len() + 1 + usize::from(escape.is_some()) > MAX_OUTPUT {
            return Err(ExpandError::ExpansionLimit);
        }
        output.extend(escape);
        output.push(byte);
    }
    Ok(())
}

/// Names are unique in first-declaration order. Snapshot the winning bindings,
/// including environment values retained by conditional command assignments.
pub fn prepare(variables: &mut VariableStore, names: &[Vec<u8>]) -> Result<bool, ExpandError> {
    if !names.is_empty() {
        let mut encoded = Vec::new();
        for name in names.iter().rev() {
            let Some(variable) = variables.get(name) else {
                continue;
            };
            if !encoded.is_empty() {
                if encoded.len() == MAX_OUTPUT {
                    return Err(ExpandError::ExpansionLimit);
                }
                encoded.push(b' ');
            }
            quote(&mut encoded, name)?;
            if variable.flavor() == Flavor::Simple {
                quote(&mut encoded, b":")?;
            }
            quote(&mut encoded, b"=")?;
            quote(&mut encoded, variable.raw_value())?;
        }
        variables.define_literal(SNAPSHOT, &encoded, Origin::Automatic)?;
        variables.define(
            b"MAKEOVERRIDES",
            b"${-*-command-variables-*-}",
            Assignment::Recursive,
            Origin::Default,
        )?;
    }
    Ok(variables
        .get(b"MAKEOVERRIDES")
        .is_some_and(|variable| !variable.raw_value().is_empty()))
}

fn tokens(input: &[u8]) -> impl Iterator<Item = Vec<u8>> + '_ {
    let mut input = input;
    while input
        .first()
        .is_some_and(|byte| matches!(byte, b' ' | b'\t'..=b'\r'))
    {
        input = &input[1..];
    }
    std::iter::from_fn(move || {
        while input
            .first()
            .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
        {
            input = &input[1..];
        }
        if input.is_empty() {
            return None;
        }
        let mut word = Vec::new();
        while let Some(&byte) = input.first() {
            if matches!(byte, b' ' | b'\t') {
                break;
            }
            input = &input[1..];
            if byte == b'\\'
                && let Some(&quoted) = input.first()
            {
                word.push(quoted);
                input = &input[1..];
            } else {
                word.push(byte);
            }
        }
        Some(word)
    })
}

pub fn replay(
    variables: &mut VariableStore,
    active: bool,
    environment_origin: Origin,
    mut context: ExpansionContext<'_>,
) -> Result<Vec<Vec<u8>>, ExpandError> {
    if !active {
        return Ok(Vec::new());
    }
    let encoded = variables.expand_at_invocation_with(b"$(MAKEOVERRIDES)", context.reborrow())?;
    let mut names = Vec::new();
    for token in tokens(&encoded) {
        let Some(spec) = assignment(&token) else {
            // These words follow MAKEFLAGS' '--'. Environment-origin words
            // that aren't assignments do not become goals or syntax errors.
            continue;
        };
        let name = variables.expand_with(spec.name, context.reborrow())?;
        variables.define_with(
            &name,
            spec.value,
            spec.kind,
            environment_origin,
            None,
            context.reborrow(),
        )?;
        names.push(name);
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_tokens_keep_blanks_dollars_and_literal_other_whitespace() {
        let mut quoted = Vec::new();
        quote(&mut quoted, b"X=a b\t\\$\x0b\n").unwrap();
        assert_eq!(quoted, b"X=a\\ b\\\t\\\\$$\x0b\n");
        assert_eq!(
            tokens(&quoted).collect::<Vec<_>>(),
            [b"X=a b\t\\$$\x0b\n".to_vec()]
        );
    }

    #[test]
    fn replay_uses_the_captured_binding_after_undefine() {
        let mut variables = VariableStore::new();
        variables
            .define_literal(b"X", b"literal$X", Origin::CommandLine)
            .unwrap();
        let active = prepare(&mut variables, &[b"X".to_vec()]).unwrap();
        variables.undefine(b"X", Origin::Override);
        replay(
            &mut variables,
            active,
            Origin::Environment,
            ExpansionContext::pure(),
        )
        .unwrap();
        let variable = variables.get(b"X").unwrap();
        assert_eq!(variable.raw_value(), b"literal$X");
        assert_eq!(variable.origin(), Origin::Environment);
        assert_eq!(variable.flavor(), Flavor::Simple);
    }
}
