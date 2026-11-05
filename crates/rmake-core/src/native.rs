// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Native extensions enter the interpreter through owned requests and scoped hosts.
//! Operating-system loading and foreign calling conventions belong to adapters.

use std::rc::Rc;

use crate::diagnostic::{Location, Output};
use crate::variables::ExpandError;

pub trait NativeHost {
    fn expand(&mut self, input: &[u8]) -> Result<Vec<u8>, ExpandError>;
    fn evaluate(&mut self, input: &[u8], location: Option<&Location>) -> Result<(), ExpandError>;
    fn emit(&mut self, event: Output<'_>) -> bool;
}

pub trait NativeCallback {
    fn invoke(
        &self,
        name: &[u8],
        args: &[Vec<u8>],
        host: &mut dyn NativeHost,
    ) -> Result<Vec<u8>, ExpandError>;
}

/// A selected registration stays alive throughout argument evaluation and entry.
/// Adapters retain the associated object handle for this registration's lifetime.
pub struct NativeFunction {
    pub minimum: u8,
    pub maximum: Option<u8>,
    pub expand_arguments: bool,
    pub callback: Rc<dyn NativeCallback>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeLoadStatus {
    Loaded,
    Skipped,
    Failed,
}

#[derive(Debug)]
pub struct LoadOutcome {
    pub name: Vec<u8>,
    pub status: NativeLoadStatus,
}

pub trait NativeRuntime {
    fn lookup(&self, name: &[u8]) -> Option<Rc<NativeFunction>>;
    fn terminate(&self, _status: u8, _host: &mut dyn NativeHost) -> Result<(), ExpandError> {
        Err(ExpandError::Unsupported(
            b"native process termination".to_vec(),
        ))
    }

    fn unload(&self, _name: &[u8], _host: &mut dyn NativeHost) -> Result<bool, ExpandError> {
        Ok(false)
    }
    fn unloaded(&self) -> Vec<Vec<u8>> {
        Vec::new()
    }

    fn load(
        &self,
        name: &[u8],
        optional: bool,
        location: &Location,
        host: &mut dyn NativeHost,
    ) -> Result<LoadOutcome, ExpandError>;
}

pub fn is_builtin_function(name: &[u8]) -> bool {
    crate::variables::arity(name).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::ExpansionContext;
    use crate::parser::Database;
    use crate::variables::{Origin, VariableStore};
    use std::cell::RefCell;
    use std::collections::{HashMap, HashSet};

    type Functions = RefCell<HashMap<Vec<u8>, Rc<NativeFunction>>>;
    enum Operation {
        Echo,
        Expand,
        Evaluate,
        Deep,
        Install(std::rc::Weak<Functions>),
    }
    impl NativeCallback for Operation {
        fn invoke(
            &self,
            _: &[u8],
            args: &[Vec<u8>],
            host: &mut dyn NativeHost,
        ) -> Result<Vec<u8>, ExpandError> {
            match self {
                Self::Echo => {
                    let mut value = format!("argc={}", args.len()).into_bytes();
                    for arg in args {
                        value.push(b'[');
                        value.extend_from_slice(arg);
                        value.push(b']');
                    }
                    Ok(value)
                }
                Self::Expand => host.expand(args.first().map(Vec::as_slice).unwrap_or_default()),
                Self::Evaluate => {
                    host.evaluate(args.first().map(Vec::as_slice).unwrap_or_default(), None)?;
                    host.expand(b"$(VALUE)")
                }
                Self::Deep => {
                    let number: usize = std::str::from_utf8(&args[0]).unwrap().parse().unwrap();
                    if number == 0 {
                        host.expand(b"$(info leaf)$(VALUE)")
                    } else {
                        host.expand(format!("$(deep {})", number - 1).as_bytes())
                    }
                }
                Self::Install(functions) => {
                    functions.upgrade().unwrap().borrow_mut().insert(
                        b"created".to_vec(),
                        function(Operation::Echo, 0, None, true),
                    );
                    Ok(b"installed".to_vec())
                }
            }
        }
    }
    fn function(
        operation: Operation,
        minimum: u8,
        maximum: Option<u8>,
        expand_arguments: bool,
    ) -> Rc<NativeFunction> {
        Rc::new(NativeFunction {
            minimum,
            maximum,
            expand_arguments,
            callback: Rc::new(operation),
        })
    }
    struct Runtime {
        functions: Rc<Functions>,
        loaded: RefCell<HashSet<Vec<u8>>>,
    }
    impl Runtime {
        fn new() -> Rc<Self> {
            let functions = Rc::new(Functions::default());
            for (name, operation, minimum, maximum, expand) in [
                (b"native".as_slice(), Operation::Echo, 0, None, true),
                (b"pair".as_slice(), Operation::Echo, 2, Some(2), true),
                (b"raw".as_slice(), Operation::Echo, 0, Some(2), false),
                (b"expand".as_slice(), Operation::Expand, 0, Some(1), true),
                (b"deep".as_slice(), Operation::Deep, 1, Some(1), true),
                (
                    b"evaluate".as_slice(),
                    Operation::Evaluate,
                    0,
                    Some(1),
                    false,
                ),
                (
                    b"install".as_slice(),
                    Operation::Install(Rc::downgrade(&functions)),
                    0,
                    Some(1),
                    true,
                ),
            ] {
                functions
                    .borrow_mut()
                    .insert(name.to_vec(), function(operation, minimum, maximum, expand));
            }
            Rc::new(Self {
                functions,
                loaded: RefCell::default(),
            })
        }
    }
    impl NativeRuntime for Runtime {
        fn lookup(&self, name: &[u8]) -> Option<Rc<NativeFunction>> {
            self.functions.borrow().get(name).cloned()
        }
        fn terminate(&self, status: u8, host: &mut dyn NativeHost) -> Result<(), ExpandError> {
            host.evaluate(format!("FINALIZED := $(VALUE):{status}").as_bytes(), None)
        }

        fn load(
            &self,
            name: &[u8],
            _optional: bool,
            location: &Location,
            host: &mut dyn NativeHost,
        ) -> Result<LoadOutcome, ExpandError> {
            let name = name.split(|&b| b == b'(').next().unwrap().to_vec();
            let status = if !self.loaded.borrow_mut().insert(name.clone()) {
                NativeLoadStatus::Skipped
            } else if name == b"missing" {
                NativeLoadStatus::Failed
            } else {
                host.evaluate(b"FROM_SETUP := $(VALUE)", Some(location))?;
                NativeLoadStatus::Loaded
            };
            Ok(LoadOutcome { name, status })
        }
    }

    #[test]
    fn argument_policies_and_late_custom_arity_keep_ordered_effects() {
        let runtime = Runtime::new();
        let variables = VariableStore::new();
        let mut events = Vec::new();
        let mut emit = |event: Output<'_>| {
            if let Output::Info(bytes) = event {
                events.push(bytes.to_vec());
            }
        };
        let value = variables
            .expand_with(
                b"$(raw $(info skipped),tail,more)|$(native one,two,three)",
                ExpansionContext::new(None, &mut emit).with_native_runtime(runtime.clone()),
            )
            .unwrap();
        assert_eq!(
            value,
            b"argc=2[$(info skipped)][tail,more]|argc=3[one][two][three]"
        );
        let error = variables
            .expand_with(
                b"$(pair $(info evaluated))",
                ExpansionContext::new(None, &mut emit).with_native_runtime(runtime),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            ExpandError::InsufficientArguments { count: 1, .. }
        ));
        assert_eq!(events, [b"evaluated".to_vec()]);
    }

    #[test]
    fn call_expands_raw_callback_arguments_and_native_names_outrank_variables() {
        let runtime = Runtime::new();
        let mut variables = VariableStore::new();
        variables
            .define_literal(b"native", b"variable", Origin::File)
            .unwrap();
        let mut events = Vec::new();
        let mut emit = |event: Output<'_>| {
            if let Output::Info(bytes) = event {
                events.push(bytes.to_vec());
            }
        };
        let value = variables
            .expand_with(
                b"$(native)|$(native )|$(call raw,$(info expanded)one,two,three)",
                ExpansionContext::new(None, &mut emit).with_native_runtime(runtime),
            )
            .unwrap();
        assert_eq!(value, b"variable|argc=1[]|argc=3[one][two][three]");
        assert_eq!(events, [b"expanded".to_vec()]);
    }

    #[test]
    fn reentrant_expansion_keeps_call_and_lexical_bindings_and_eval_is_fresh() {
        let runtime = Runtime::new();
        let mut database = Database::default();
        database
            .read(b"VALUE = global\nwrapper = $(expand $$(1))\n", b"host.mk")
            .unwrap();
        let value = database.expand_with(b"$(foreach VALUE,local,$(expand $$(VALUE))):$(call wrapper,parameter):$(evaluate VALUE := changed):$(VALUE)", ExpansionContext::pure().with_native_runtime(runtime)).unwrap();
        assert_eq!(value, b"local:parameter:changed:changed");
    }

    #[test]
    fn registration_is_visible_to_the_next_reference_in_the_same_buffer() {
        let runtime = Runtime::new();
        let value = VariableStore::new()
            .expand_with(
                b"$(install )$(created hello)",
                ExpansionContext::pure().with_native_runtime(runtime),
            )
            .unwrap();
        assert_eq!(value, b"installedargc=1[hello]");
    }

    #[test]
    fn module_setup_metadata_and_remake_records_keep_identity_and_location() {
        let runtime = Runtime::new();
        let mut database = Database::default();
        database.read_with(b"VALUE := before\n.LOADED := prior\nload module$literal(custom)\nload module$literal(custom)\n-load missing\n", b"host.mk", ExpansionContext::pure().with_native_runtime(runtime)).unwrap();
        // Dollar quoting is a reader expansion: use an ordinary raw name below
        // to verify the shared metadata helper does not expand its own input.
        let runtime = Runtime::new();
        let location = Location {
            file: b"raw.mk".as_slice().into(),
            line: 7,
        };
        database
            .load_native_with(
                b"raw$literal",
                false,
                &location,
                &mut ExpansionContext::pure().with_native_runtime(runtime),
            )
            .unwrap();
        assert_eq!(
            database.variables.get(b".LOADED").unwrap().raw_value(),
            b"prior moduleiteral raw$literal"
        );
        assert_eq!(
            database.variables.expand(b"$(FROM_SETUP)").unwrap(),
            b"before"
        );
        assert_eq!(database.makefiles.len(), 3);
        assert_eq!(database.loaded_modules[b"missing".as_slice()].line, 5);
        assert_eq!(database.loaded_modules[b"raw$literal".as_slice()], location);
    }

    #[test]
    fn deep_native_argument_storage_stops_before_a_later_effect() {
        let runtime = Runtime::new();
        let mut variables = VariableStore::new();
        variables
            .define_literal(
                b"BIG",
                &vec![b'x'; crate::variables::MAX_OUTPUT / 4],
                Origin::File,
            )
            .unwrap();
        for index in 0..160 {
            variables
                .define(
                    format!("R{index}").as_bytes(),
                    format!("$(strip a$(R{})b)", index + 1).as_bytes(),
                    crate::variables::Assignment::Recursive,
                    Origin::File,
                )
                .unwrap();
        }
        variables
            .define(
                b"R160",
                b"$(native $(BIG),$(BIG),$(BIG),$(BIG),$(info forbidden))",
                crate::variables::Assignment::Recursive,
                Origin::File,
            )
            .unwrap();
        let mut events = Vec::new();
        let mut emit = |event: Output<'_>| {
            if let Output::Info(bytes) = event {
                events.push(bytes.to_vec());
            }
        };
        let error = variables
            .expand_with(
                b"$(R0)",
                ExpansionContext::new(None, &mut emit).with_native_runtime(runtime.clone()),
            )
            .unwrap_err();
        assert_eq!(error, ExpandError::ExpansionLimit);
        assert!(events.is_empty());
        assert_eq!(
            variables
                .expand_with(
                    b"$(native okay)",
                    ExpansionContext::pure().with_native_runtime(runtime)
                )
                .unwrap(),
            b"argc=1[okay]",
        );
    }

    #[test]
    fn termination_host_keeps_database_and_active_location_alive() {
        let runtime = Runtime::new();
        let mut database = Database::default();
        database.read(b"VALUE := retained\n", b"before.mk").unwrap();
        let location = Location {
            file: b"exit.mk".as_slice().into(),
            line: 9,
        };
        database
            .terminate_native_with(
                2,
                &mut ExpansionContext::pure()
                    .with_native_runtime(runtime)
                    .at(Some(&location)),
            )
            .unwrap();
        let finalized = database.variables.get(b"FINALIZED").unwrap();
        assert_eq!(finalized.raw_value(), b"retained:2");
        assert_eq!(finalized.definition(), Some(&location));
    }

    #[test]
    fn required_load_failure_stops_before_remake_records_and_keeps_requested_symbol() {
        let runtime = Runtime::new();
        let mut database = Database::default();
        let location = Location {
            file: b"host.mk".as_slice().into(),
            line: 2,
        };
        let error = database
            .load_native_with(
                b"missing(custom_setup)",
                false,
                &location,
                &mut ExpansionContext::pure().with_native_runtime(runtime),
            )
            .unwrap_err();
        assert_eq!(
            error.diagnostic().as_bytes(),
            b"missing(custom_setup): failed to load"
        );
        assert!(database.loaded_modules.is_empty());
        assert!(database.makefiles.is_empty());
    }

    #[test]
    fn load_history_observes_resolved_identity_before_setup_and_conditional_entry_after() {
        struct Observer {
            history: crate::database::History,
            status: NativeLoadStatus,
            before_files: u64,
        }
        impl NativeRuntime for Observer {
            fn lookup(&self, _: &[u8]) -> Option<Rc<NativeFunction>> {
                None
            }
            fn load(
                &self,
                name: &[u8],
                _: bool,
                _: &Location,
                _: &mut dyn NativeHost,
            ) -> Result<LoadOutcome, ExpandError> {
                assert_eq!(name, b"module.so(custom_setup)");
                assert_eq!(self.history.file_stats().lookups, self.before_files + 1);
                self.history.with_string_cache(|cache| {
                    assert_eq!(cache.hash_stats().filled, 1);
                    assert_eq!(cache.hash_stats().lookups, 1);
                });
                Ok(LoadOutcome {
                    name: b"module.so".to_vec(),
                    status: self.status,
                })
            }
        }
        for (status, optional, existing, expected_reads) in [
            (NativeLoadStatus::Loaded, false, false, 3),
            (NativeLoadStatus::Failed, true, false, 3),
            (NativeLoadStatus::Failed, false, false, 1),
            (NativeLoadStatus::Skipped, false, true, 2),
        ] {
            let mut database = Database::default();
            database.enable_database_history().unwrap();
            if existing {
                database.file_records.insert(b"module.so");
            }
            let history = database.database_history().unwrap().clone();
            let before_files = history.file_stats().lookups;
            let runtime = Rc::new(Observer {
                history: history.clone(),
                status,
                before_files,
            });
            let location = Location {
                file: b"caller.mk".as_slice().into(),
                line: 7,
            };
            let result = database.load_native_with(
                b"module.so(custom_setup)",
                optional,
                &location,
                &mut ExpansionContext::pure().with_native_runtime(runtime),
            );
            assert_eq!(
                result.is_err(),
                status == NativeLoadStatus::Failed && !optional
            );
            assert_eq!(history.file_stats().lookups, before_files + expected_reads);
            assert_eq!(
                database.file_records.contains(b"module.so"),
                expected_reads != 1
            );
        }
    }

    struct TestStack {
        active: std::cell::Cell<usize>,
        limit: usize,
    }
    impl crate::diagnostic::EvaluationStack for TestStack {
        fn run(
            &self,
            action: &mut dyn FnMut() -> Result<(), crate::parser::ParseError>,
        ) -> Option<Result<(), crate::parser::ParseError>> {
            let depth = self.active.get();
            if depth == self.limit {
                return None;
            }
            self.active.set(depth + 1);
            let result = action();
            self.active.set(depth);
            Some(result)
        }
    }

    #[test]
    fn bounded_native_reentry_keeps_lexical_scope_and_evaluates_the_leaf_once() {
        // The test adapter owns a bounded large host stack; the application uses
        // its qualified same-thread growing-stack adapter instead.
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                let stack = TestStack {
                    active: std::cell::Cell::new(0),
                    limit: 512,
                };
                let runtime = Runtime::new();
                let mut variables = VariableStore::new();
                variables
                    .define_literal(b"VALUE", b"global", Origin::File)
                    .unwrap();
                let mut events = Vec::new();
                let mut emit = |event: Output<'_>| {
                    if let Output::Info(bytes) = event {
                        events.push(bytes.to_vec());
                    }
                };
                let value = variables
                    .expand_with(
                        b"$(foreach VALUE,local,$(deep 256))",
                        ExpansionContext::new(None, &mut emit)
                            .with_native_runtime(runtime)
                            .with_evaluation_stack(Some(&stack)),
                    )
                    .unwrap();
                assert_eq!(value, b"local");
                assert_eq!(events, [b"leaf".to_vec()]);
                assert_eq!(stack.active.get(), 0);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn native_reentry_preserves_cycle_detection_and_stops_fatal_effects() {
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                let stack = TestStack {
                    active: std::cell::Cell::new(0),
                    limit: 512,
                };
                let runtime = Runtime::new();
                let mut variables = VariableStore::new();
                variables
                    .define(
                        b"VALUE",
                        b"$(deep 1)",
                        crate::variables::Assignment::Recursive,
                        Origin::File,
                    )
                    .unwrap();
                let mut events = Vec::new();
                let mut emit = |event: Output<'_>| {
                    if let Output::Info(bytes) = event {
                        events.push(bytes.to_vec());
                    }
                };
                let error = variables
                    .expand_with(
                        b"$(VALUE)",
                        ExpansionContext::new(None, &mut emit)
                            .with_native_runtime(runtime.clone())
                            .with_evaluation_stack(Some(&stack)),
                    )
                    .unwrap_err();
                assert!(matches!(error, ExpandError::RecursiveVariable(name) if name == b"VALUE"));
                variables
                    .define(
                        b"VALUE",
                        b"$(error stop)$(info forbidden)",
                        crate::variables::Assignment::Recursive,
                        Origin::File,
                    )
                    .unwrap();
                let error = variables
                    .expand_with(
                        b"$(deep 256)$(info after)",
                        ExpansionContext::new(None, &mut emit)
                            .with_native_runtime(runtime)
                            .with_evaluation_stack(Some(&stack)),
                    )
                    .unwrap_err();
                assert!(error.diagnostic().as_bytes().ends_with(b"stop"));
                assert_eq!(events, [b"leaf".to_vec(), b"leaf".to_vec()]);
                assert_eq!(stack.active.get(), 0);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn native_reentry_without_stack_adapter_keeps_the_library_limit() {
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                let runtime = Runtime::new();
                let mut events = Vec::new();
                let mut emit = |event: Output<'_>| {
                    if let Output::Info(bytes) = event {
                        events.push(bytes.to_vec());
                    }
                };
                assert_eq!(
                    VariableStore::new().expand_with(
                        b"$(deep 256)",
                        ExpansionContext::new(None, &mut emit).with_native_runtime(runtime)
                    ),
                    Err(ExpandError::ExpansionLimit),
                );
                assert!(events.is_empty());
                let stack = TestStack {
                    active: std::cell::Cell::new(0),
                    limit: 0,
                };
                assert!(
                    VariableStore::new()
                        .expand_with(
                            b"$(deep 1)",
                            ExpansionContext::pure()
                                .with_native_runtime(Runtime::new())
                                .with_evaluation_stack(Some(&stack))
                        )
                        .is_err()
                );
                assert_eq!(stack.active.get(), 0);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn nullable_eval_sources_survive_definitions_recipes_and_module_origins() {
        use crate::diagnostic::SourceName;
        for file in [
            SourceName::null(),
            b"".as_slice().into(),
            b"<builtin>".as_slice().into(),
        ] {
            let runtime = Runtime::new();
            let mut database = Database::default();
            let location = Location {
                file: file.clone(),
                line: 0,
            };
            database
                .read_evaluated(
                    b"VALUE := retained\nuser:;@:\nload module\n",
                    Some(&location),
                    ExpansionContext::pure().with_native_runtime(runtime),
                )
                .unwrap();
            assert_eq!(
                database.variables.get(b"VALUE").unwrap().definition(),
                Some(&location)
            );
            let recipe = &database.rules[b"user".as_slice()].recipes.as_ref().unwrap()[0];
            assert_eq!(recipe.location, location);
            assert!(!recipe.builtin);
            assert_eq!(database.loaded_modules[b"module".as_slice()], location);
        }
        let mut database = Database::default();
        database
            .read_evaluated(
                b"load anonymous\n",
                None,
                ExpansionContext::pure().with_native_runtime(Runtime::new()),
            )
            .unwrap();
        assert!(
            database.loaded_modules[b"anonymous".as_slice()]
                .file
                .is_null()
        );
    }

    #[test]
    fn explicit_eval_source_lines_preserve_zero_and_unsigned_boundaries() {
        for line in [0, 1, 23, 42, u32::MAX as usize, usize::MAX] {
            let mut database = Database::default();
            let location = Location {
                file: b"native-lines.mk".as_slice().into(),
                line,
            };
            let mut warnings = Vec::new();
            let mut emit = |event: Output<'_>| {
                if let Output::Warning { location, message } = event {
                    warnings.push((location.cloned(), message.to_vec()));
                }
            };
            database
                .read_evaluated(
                    b"$(warning first)\nVALUE := saved\n$(warning second)\n",
                    Some(&location),
                    ExpansionContext::new(None, &mut emit),
                )
                .unwrap();
            assert_eq!(
                warnings,
                [
                    (Some(location.clone()), b"first".to_vec()),
                    (Some(location.clone()), b"second".to_vec()),
                ]
            );
            assert_eq!(
                database.variables.get(b"VALUE").unwrap().definition(),
                Some(&location)
            );
            let error = database
                .read_evaluated(b"broken\n", Some(&location), ExpansionContext::pure())
                .unwrap_err();
            assert_eq!(error.location, location);
        }
    }

    #[test]
    fn null_native_eval_location_preserves_immediate_warning_context() {
        let runtime = Runtime::new();
        let mut database = Database::default();
        let location = Location {
            file: b"host.mk".as_slice().into(),
            line: 4,
        };
        let mut warnings = Vec::new();
        let mut emit = |event: Output<'_>| {
            if let Output::Warning { location, message } = event {
                warnings.push((location.cloned(), message.to_vec()));
            }
        };
        database
            .expand_with(
                b"$(evaluate $(warning native-warning))",
                ExpansionContext::new(Some(&location), &mut emit).with_native_runtime(runtime),
            )
            .unwrap();
        assert_eq!(warnings, [(Some(location), b"native-warning".to_vec())]);
    }

    #[test]
    fn malformed_registered_function_has_function_diagnostics() {
        let runtime = Runtime::new();
        let error = VariableStore::new()
            .expand_with(
                b"$(native one$(VALUE)",
                ExpansionContext::pure().with_native_runtime(runtime),
            )
            .unwrap_err();
        assert!(
            matches!(error, ExpandError::UnterminatedFunction { function, close: b')' } if function == b"native")
        );
    }
}
