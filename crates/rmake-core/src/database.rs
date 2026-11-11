// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Jeff DaSilva

//! Pure borrowed database diagnostics and independently recorded logical events.
//! Formatting never expands Make syntax, searches files, or changes bindings.
//! Logical hash counters describe the compatibility protocol, not Rust storage.

use std::collections::HashSet;
use std::ffi::CStr;
use std::io::{self, Write};

use crate::database_messages as msg;
use crate::diagnostic::Location;
use crate::messages::{Argument, FormatError, checked_format};
use crate::parser::{Database, Dependency, Recipe, Rule, SecondaryPrerequisite};
use crate::variables::{Assignment, Flavor, Origin, Variable, VariableStore};

#[derive(Clone, Copy, Debug)]
pub struct PatternVariableView<'a> {
    pub name: &'a [u8],
    pub value: &'a [u8],
    pub assignment: Assignment,
    pub origin: Origin,
    pub private: bool,
    pub export: Option<bool>,
    pub definition: Option<&'a Location>,
}

#[derive(Clone, Copy, Debug)]
pub struct VpathView<'a> {
    pub pattern: &'a [u8],
    pub directories: &'a [Vec<u8>],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HashStats {
    pub filled: usize,
    pub buckets: usize,
    pub rehashes: u64,
    pub lookups: u64,
    pub collisions: u64,
}

/// Integer percentage operands let the host preserve its numeric locale and
/// floating environment. Formatting performs no interpreter or filesystem work.
pub trait NumericFormatting {
    fn write_percent(
        &self,
        output: &mut dyn Write,
        numerator: u64,
        denominator: u64,
        fractional_digits: u8,
    ) -> io::Result<()>;
}

/// Convenience profile for standalone pure components: C locale and default
/// rounding. The Linux application supplies its live C numeric implementation.
pub struct RustNumericFormatting;
impl NumericFormatting for RustNumericFormatting {
    fn write_percent(
        &self,
        output: &mut dyn Write,
        numerator: u64,
        denominator: u64,
        fractional_digits: u8,
    ) -> io::Result<()> {
        let value = if numerator == 0 || denominator == 0 {
            0.0
        } else {
            100.0 * numerator as f64 / denominator as f64
        };
        match fractional_digits {
            0 => write!(output, "{value:.0}"),
            1 => write!(output, "{value:.1}"),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unsupported percentage precision",
            )),
        }
    }
}

/// A dump observes the host's live message and numeric locales separately.
type CatalogLookup = fn(&'static CStr) -> Result<Vec<u8>, FormatError>;

#[derive(Clone, Copy)]
pub struct Presentation<'a> {
    numeric: &'a dyn NumericFormatting,
    lookup: Option<CatalogLookup>,
}

impl<'a> Presentation<'a> {
    pub const fn localized(numeric: &'a dyn NumericFormatting, lookup: CatalogLookup) -> Self {
        Self {
            numeric,
            lookup: Some(lookup),
        }
    }

    const fn english(numeric: &'a dyn NumericFormatting) -> Self {
        Self {
            numeric,
            lookup: None,
        }
    }

    fn text(self, output: &mut impl Write, key: &'static CStr) -> io::Result<()> {
        match self.lookup.and_then(|lookup| lookup(key).ok()) {
            Some(translated) => output.write_all(&translated),
            None => output.write_all(key.to_bytes()),
        }
    }

    fn line(self, output: &mut impl Write, key: &'static CStr) -> io::Result<()> {
        self.text(output, key)?;
        output.write_all(b"\n")
    }

    fn format(
        self,
        output: &mut impl Write,
        key: msg::Key,
        arguments: &[Argument<'_>],
    ) -> io::Result<()> {
        let rendered = self
            .lookup
            .and_then(|lookup| lookup(key.msgid).ok())
            .and_then(|translation| checked_format(&translation, arguments).ok())
            .unwrap_or_else(|| {
                checked_format(key.msgid.to_bytes(), arguments)
                    .expect("static database key and arguments have matching types")
            });
        output.write_all(&rendered)
    }

    pub fn open(self, output: &mut impl Write, ctime: &[u8]) -> io::Result<()> {
        self.format(
            output,
            msg::OPEN,
            &[Argument::Bytes(&[ctime, b"\n"].concat())],
        )
    }

    pub fn close(self, output: &mut impl Write, ctime: &[u8]) -> io::Result<()> {
        self.format(
            output,
            msg::CLOSE,
            &[Argument::Bytes(&[ctime, b"\n"].concat())],
        )
    }
}

const ENGLISH: Presentation<'static> = Presentation::english(&RustNumericFormatting);

pub fn write_hash_stats(output: &mut impl Write, stats: HashStats) -> io::Result<()> {
    write_hash_stats_with_numeric(output, stats, &RustNumericFormatting)
}

pub fn write_hash_stats_with_numeric(
    output: &mut impl Write,
    stats: HashStats,
    numeric: &dyn NumericFormatting,
) -> io::Result<()> {
    write_hash_stats_with_presentation(output, stats, Presentation::english(numeric))
}

fn write_hash_stats_with_presentation(
    output: &mut impl Write,
    stats: HashStats,
    presentation: Presentation<'_>,
) -> io::Result<()> {
    let mut percent = Vec::new();
    presentation.numeric.write_percent(
        &mut percent,
        stats.filled as u64,
        stats.buckets as u64,
        0,
    )?;
    presentation.format(
        output,
        msg::LOAD,
        &[
            Argument::Unsigned(stats.filled as u64),
            Argument::Unsigned(stats.buckets as u64),
            Argument::FixedDecimal {
                bytes: &percent,
                digits: 0,
            },
        ],
    )?;
    presentation.format(output, msg::REHASH, &[Argument::Unsigned(stats.rehashes)])?;
    percent.clear();
    presentation
        .numeric
        .write_percent(&mut percent, stats.collisions, stats.lookups, 0)?;
    presentation.format(
        output,
        msg::COLLISIONS,
        &[
            Argument::Unsigned(stats.collisions),
            Argument::Unsigned(stats.lookups),
            Argument::FixedDecimal {
                bytes: &percent,
                digits: 0,
            },
        ],
    )
}

fn c_bytes(value: &[u8]) -> &[u8] {
    &value[..value
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(value.len())]
}

fn origin_label(origin: Origin) -> &'static CStr {
    match origin {
        Origin::Default => c"default",
        Origin::Environment => c"environment",
        Origin::File => c"makefile",
        Origin::EnvironmentOverride => c"environment under -e",
        Origin::CommandLine => c"command line",
        Origin::Override => c"'override' directive",
        Origin::Automatic => c"automatic",
    }
}

fn write_definition(
    output: &mut impl Write,
    origin: Origin,
    private: bool,
    definition: Option<&Location>,
    presentation: Presentation<'_>,
) -> io::Result<()> {
    output.write_all(b"# ")?;
    presentation.text(output, origin_label(origin))?;
    if private {
        output.write_all(b" private")?;
    }
    if let Some(location) = definition
        && let Some(file) = location.file.as_option()
    {
        presentation.format(
            output,
            msg::VARIABLE_DEFINITION,
            &[
                Argument::Bytes(file),
                Argument::Unsigned(location.line as u64),
            ],
        )?;
    }
    output.write_all(b"\n")
}

fn write_value(
    output: &mut impl Write,
    prefix: &[u8],
    name: &[u8],
    value: &[u8],
    flavor: Flavor,
    append: bool,
) -> io::Result<()> {
    let name = c_bytes(name);
    let value = c_bytes(value);
    output.write_all(prefix)?;
    if flavor == Flavor::Recursive && value.contains(&b'\n') {
        output.write_all(b"define ")?;
        output.write_all(name)?;
        output.write_all(b"\n")?;
        output.write_all(value)?;
        return output.write_all(b"\nendef\n");
    }
    output.write_all(name)?;
    output.write_all(match flavor {
        Flavor::Simple => b" := ",
        Flavor::Recursive if append => b" += ",
        Flavor::Recursive => b" = ",
    })?;
    if !value.is_empty() && value.iter().all(u8::is_ascii_whitespace) {
        output.write_all(b"$(subst ,,")?;
        output.write_all(value)?;
        output.write_all(b")")?;
    } else if flavor == Flavor::Recursive {
        output.write_all(value)?;
    } else {
        let mut start = 0;
        for (index, byte) in value.iter().enumerate() {
            if *byte == b'$' {
                output.write_all(&value[start..=index])?;
                output.write_all(b"$")?;
                start = index + 1;
            }
        }
        output.write_all(&value[start..])?;
    }
    output.write_all(b"\n")
}

pub fn write_variable(
    output: &mut impl Write,
    prefix: &[u8],
    name: &[u8],
    variable: &Variable,
) -> io::Result<()> {
    write_variable_value(
        output,
        prefix,
        name,
        variable,
        variable.raw_value(),
        ENGLISH,
    )
}

fn write_variable_value(
    output: &mut impl Write,
    prefix: &[u8],
    name: &[u8],
    variable: &Variable,
    value: &[u8],
    presentation: Presentation<'_>,
) -> io::Result<()> {
    write_definition(
        output,
        variable.origin(),
        variable.is_private(),
        variable.definition(),
        presentation,
    )?;
    write_value(
        output,
        prefix,
        name,
        value,
        variable.flavor(),
        variable.is_outer_append(),
    )
}

pub fn write_variables(
    output: &mut impl Write,
    store: &VariableStore,
    stats: HashStats,
) -> io::Result<()> {
    write_variables_with_presentation(output, store, stats, ENGLISH)
}

fn write_variables_with_presentation(
    output: &mut impl Write,
    store: &VariableStore,
    stats: HashStats,
    presentation: Presentation<'_>,
) -> io::Result<()> {
    presentation.line(output, c"\n# Variables\n")?;
    for (name, variable) in store.database_definitions() {
        let cached = store.database_cached_value(variable);
        write_variable_value(
            output,
            b"",
            name,
            variable,
            cached.as_deref().unwrap_or(variable.raw_value()),
            presentation,
        )?;
    }
    write_variable_stats(output, stats, presentation)
}

fn write_variable_stats(
    output: &mut impl Write,
    stats: HashStats,
    presentation: Presentation<'_>,
) -> io::Result<()> {
    presentation.text(output, c"# variable set hash-table stats:\n")?;
    output.write_all(b"# ")?;
    write_hash_stats_with_presentation(output, stats, presentation)?;
    output.write_all(b"\n")
}

pub fn write_pattern_variables(output: &mut impl Write, database: &Database) -> io::Result<()> {
    write_pattern_variables_with_presentation(output, database, ENGLISH)
}

fn write_pattern_variables_with_presentation(
    output: &mut impl Write,
    database: &Database,
    presentation: Presentation<'_>,
) -> io::Result<()> {
    presentation.line(output, c"\n# Pattern-specific Variable Values")?;
    let mut count = 0;
    for (pattern, variable) in database.database_pattern_variables() {
        count += 1;
        output.write_all(b"\n")?;
        output.write_all(c_bytes(pattern.literal_bytes()))?;
        output.write_all(b" :\n")?;
        write_definition(
            output,
            variable.origin,
            variable.private,
            variable.definition,
            presentation,
        )?;
        // The declaration record has not become a target-local binding. Its
        // observable database representation is simple even for a deferred RHS.
        write_value(
            output,
            b"# ",
            variable.name,
            variable.value,
            Flavor::Simple,
            false,
        )?;
    }
    if count == 0 {
        presentation.line(output, c"\n# No pattern-specific variable values.")
    } else {
        presentation.format(output, msg::PATTERN_COUNT, &[Argument::Unsigned(count)])
    }
}

/// Borrow retained runtime filename decisions without searching or expanding.
/// Static components omit this view and preserve declaration spelling.
pub trait FileNames {
    fn resolve<'a>(&'a self, name: &'a [u8]) -> &'a [u8];
}

fn resolved_file_name<'a>(names: Option<&'a dyn FileNames>, name: &'a [u8]) -> &'a [u8] {
    names.map_or(name, |names| names.resolve(name))
}

pub fn write_prerequisites(output: &mut impl Write, dependencies: &[Dependency]) -> io::Result<()> {
    write_prerequisites_with_names(output, dependencies, None, None)
}

fn write_prerequisites_with_names(
    output: &mut impl Write,
    dependencies: &[Dependency],
    deferred: Option<&[SecondaryPrerequisite]>,
    names: Option<&dyn FileNames>,
) -> io::Result<()> {
    let occurrences = dependencies.iter().enumerate().map(|(index, dependency)| {
        let name = if deferred.is_some_and(|clauses| {
            clauses
                .binary_search_by_key(&index, |clause| clause.index)
                .is_ok()
        }) {
            crate::parser::trim(&dependency.name)
        } else {
            resolved_file_name(names, &dependency.name)
        };
        (name, dependency.order_only)
    });
    for (name, _) in occurrences.clone().filter(|(_, order_only)| !order_only) {
        output.write_all(b" ")?;
        output.write_all(c_bytes(name))?;
    }
    let mut order = occurrences.filter(|(_, order_only)| *order_only).peekable();
    if order.peek().is_some() {
        output.write_all(b" |")?;
        for (name, _) in order {
            output.write_all(b" ")?;
            output.write_all(c_bytes(name))?;
        }
    }
    output.write_all(b"\n")
}

pub fn write_recipes(
    output: &mut impl Write,
    recipes: &[Recipe],
    location: Option<&Location>,
) -> io::Result<()> {
    write_recipes_with_presentation(output, recipes, location, ENGLISH)
}

fn write_recipes_with_presentation(
    output: &mut impl Write,
    recipes: &[Recipe],
    location: Option<&Location>,
    presentation: Presentation<'_>,
) -> io::Result<()> {
    presentation.text(output, c"#  recipe to execute")?;
    let definition =
        location.and_then(|location| Some((location.file.as_option()?, location.line)));
    if let Some((file, line)) = definition
        && !recipes.first().is_some_and(|recipe| recipe.builtin)
    {
        presentation.format(
            output,
            msg::RECIPE_DEFINITION,
            &[Argument::Bytes(file), Argument::Unsigned(line as u64)],
        )?;
    } else {
        presentation.line(output, c" (built-in):")?;
    }
    if recipes.is_empty() {
        return output.write_all(b"\t\n");
    }
    for recipe in recipes {
        output.write_all(&[recipe.prefix])?;
        output.write_all(c_bytes(&recipe.bytes))?;
        output.write_all(b"\n")?;
    }
    Ok(())
}

pub fn write_implicit_rule(output: &mut impl Write, pattern: &[u8], rule: &Rule) -> io::Result<()> {
    write_implicit_rule_targets(output, std::iter::once(pattern), rule, ENGLISH)
}

fn write_implicit_rule_targets<'a>(
    output: &mut impl Write,
    patterns: impl IntoIterator<Item = &'a [u8]>,
    rule: &Rule,
    presentation: Presentation<'_>,
) -> io::Result<()> {
    for (index, pattern) in patterns.into_iter().enumerate() {
        if index != 0 {
            output.write_all(b" ")?;
        }
        output.write_all(c_bytes(pattern))?;
    }
    output.write_all(if rule.terminal { b"::" } else { b":" })?;
    write_prerequisites(output, &rule.dependencies)?;
    if let Some(recipes) = &rule.recipes {
        write_recipes_with_presentation(
            output,
            recipes,
            rule.recipe_location.as_ref(),
            presentation,
        )?;
    }
    Ok(())
}

pub fn write_implicit_rules(output: &mut impl Write, database: &Database) -> io::Result<()> {
    write_implicit_rules_with_presentation(output, database, ENGLISH)
}

fn write_implicit_rules_with_presentation(
    output: &mut impl Write,
    database: &Database,
    presentation: Presentation<'_>,
) -> io::Result<()> {
    presentation.line(output, c"\n# Implicit Rules")?;
    let rules = database.database_implicit_rules();
    let mut groups = HashSet::new();
    let mut count = 0;
    let mut terminal = 0;
    for (pattern, rule) in rules {
        if let Some(group) = &rule.group
            && let Some(identity) = group.implicit_identity
        {
            if !groups.insert(identity) {
                continue;
            }
            output.write_all(b"\n")?;
            write_implicit_rule_targets(
                output,
                group.targets.iter().map(Vec::as_slice),
                rule,
                presentation,
            )?;
        } else {
            output.write_all(b"\n")?;
            write_implicit_rule_targets(
                output,
                std::iter::once(pattern.literal_bytes()),
                rule,
                presentation,
            )?;
        }
        count += 1;
        terminal += u64::from(rule.terminal);
    }
    if count == 0 {
        return presentation.line(output, c"\n# No implicit rules.");
    }
    let mut percent = Vec::new();
    presentation
        .numeric
        .write_percent(&mut percent, terminal, count, 1)?;
    presentation.format(
        output,
        msg::IMPLICIT_COUNT,
        &[
            Argument::Unsigned(count),
            Argument::Unsigned(terminal),
            Argument::FixedDecimal {
                bytes: &percent,
                digits: 1,
            },
        ],
    )
}

pub fn write_vpaths(
    output: &mut impl Write,
    database: &Database,
    general: &[&[u8]],
) -> io::Result<()> {
    write_vpaths_with_presentation(output, database, general, ENGLISH)
}

fn write_vpaths_with_presentation(
    output: &mut impl Write,
    database: &Database,
    general: &[&[u8]],
    presentation: Presentation<'_>,
) -> io::Result<()> {
    presentation.line(output, c"\n# VPATH Search Paths\n")?;
    let mut count = 0;
    for path in database.database_vpaths() {
        count += 1;
        output.write_all(b"vpath ")?;
        output.write_all(c_bytes(path.pattern))?;
        output.write_all(b" ")?;
        for (index, directory) in path.directories.iter().enumerate() {
            if index != 0 {
                output.write_all(b":")?;
            }
            let printed = if directory.len() > 1 {
                directory.strip_suffix(b"/").unwrap_or(directory)
            } else {
                directory
            };
            output.write_all(c_bytes(printed))?;
        }
        output.write_all(b"\n")?;
    }
    if count == 0 {
        presentation.line(output, c"# No 'vpath' search paths.")?;
    } else {
        presentation.format(output, msg::VPATH_COUNT, &[Argument::Unsigned(count)])?;
    }
    if general.is_empty() {
        return presentation.line(output, c"\n# No general ('VPATH' variable) search path.");
    }
    presentation.text(output, c"\n# General ('VPATH' variable) search path:\n# ")?;
    for (index, directory) in general.iter().enumerate() {
        if index != 0 {
            output.write_all(b":")?;
        }
        let printed = if directory.len() > 1 {
            directory.strip_suffix(b"/").unwrap_or(directory)
        } else {
            directory
        };
        output.write_all(c_bytes(printed))?;
    }
    output.write_all(b"\n")
}

#[derive(Clone, Copy, Debug)]
pub enum HashKind {
    Variable,
    Filename,
}

// Compact arena offsets retain exact keys without one allocation per key.
// Zero and one are empty/deleted; live offsets include a two-byte bias.
#[derive(Clone, Debug)]
struct TableLayout {
    slots: Vec<usize>,
    keys: Vec<u8>,
    dead_bytes: usize,
}
impl TableLayout {
    fn new(buckets: usize) -> Self {
        Self {
            slots: vec![0; buckets],
            keys: Vec::new(),
            dead_bytes: 0,
        }
    }
    fn name(&self, slot: usize) -> &[u8] {
        let offset = slot - 2;
        let end = offset + std::mem::size_of::<usize>();
        let length = usize::from_ne_bytes(self.keys[offset..end].try_into().unwrap());
        &self.keys[end..end + length]
    }
    fn retire(&mut self, index: usize) {
        let size = self.name(self.slots[index]).len() + std::mem::size_of::<usize>();
        self.slots[index] = 1;
        self.dead_bytes += size;
        // Tombstone reuse need not trigger a hash-table rebuild. Independently
        // bound dead key storage even under repeated undefine/redefine cycles.
        if self.dead_bytes >= 4096 && self.dead_bytes >= self.keys.len() / 2 {
            let mut keys = Vec::with_capacity(self.keys.len() - self.dead_bytes);
            for slot in self.slots.iter_mut().filter(|slot| **slot > 1) {
                let offset = *slot - 2;
                let end = offset + std::mem::size_of::<usize>();
                let length = usize::from_ne_bytes(self.keys[offset..end].try_into().unwrap());
                *slot = keys.len() + 2;
                keys.extend_from_slice(&self.keys[offset..end + length]);
            }
            self.keys = keys;
            self.dead_bytes = 0;
        }
    }
    fn store(&mut self, name: &[u8]) -> usize {
        let slot = self
            .keys
            .len()
            .checked_add(2)
            .expect("logical key arena size");
        self.keys.extend_from_slice(&name.len().to_ne_bytes());
        self.keys.extend_from_slice(name);
        slot
    }
}

/// Logical lookup and mutation events, independent of physical Rust storage.
/// The application records them from startup to preserve late `-p` selection.
#[derive(Clone, Debug)]
pub struct TableTelemetry {
    kind: HashKind,
    slots: std::sync::Arc<TableLayout>,
    empty: usize,
    stats: HashStats,
}

impl TableTelemetry {
    pub fn new(kind: HashKind, buckets: usize) -> Option<Self> {
        (buckets >= 16 && buckets.is_power_of_two()).then(|| Self {
            kind,
            slots: std::sync::Arc::new(TableLayout::new(buckets)),
            empty: buckets,
            stats: HashStats {
                buckets,
                ..HashStats::default()
            },
        })
    }
    pub fn stats(&self) -> HashStats {
        self.stats
    }
    fn probe(kind: HashKind, layout: &TableLayout, name: &[u8]) -> (usize, bool, u64) {
        let hash = match kind {
            HashKind::Variable => crate::variable_order::fingerprint(name) as usize,
            HashKind::Filename => crate::file_records::fingerprint(name) as usize,
        };
        let slots = &layout.slots;
        let home = hash & (slots.len() - 1);
        let mut vacant = None;
        let mut collisions = 0_u64;
        for distance in 0..slots.len() {
            let index = (home + distance) & (slots.len() - 1);
            match slots[index] {
                0 => return (vacant.unwrap_or(index), false, collisions),
                1 => {
                    vacant.get_or_insert(index);
                }
                slot if layout.name(slot) == name => return (index, true, collisions),
                _ => collisions = collisions.wrapping_add(1),
            }
        }
        (
            vacant.expect("logical table always retains a vacant slot"),
            false,
            collisions,
        )
    }
    fn locate(&mut self, name: &[u8], counted: bool) -> (usize, bool) {
        let (index, found, collisions) = Self::probe(self.kind, &self.slots, name);
        if counted {
            self.stats.lookups = self.stats.lookups.wrapping_add(1);
            self.stats.collisions = self.stats.collisions.wrapping_add(collisions);
        }
        (index, found)
    }
    pub fn lookup(&mut self, name: &[u8]) -> bool {
        self.locate(name, true).1
    }
    /// Repeated immutable-table lookups have the same probe path. Count their
    /// logical multiplicity without walking it repeatedly. No insertion,
    /// deletion, or user effect is permitted between the represented events.
    pub fn lookup_repeated(&mut self, name: &[u8], count: u64) {
        if count == 0 {
            return;
        }
        let collisions = self.stats.collisions;
        self.lookup(name);
        let cost = self.stats.collisions.wrapping_sub(collisions);
        self.stats.lookups = self.stats.lookups.wrapping_add(count - 1);
        self.stats.collisions = self
            .stats
            .collisions
            .wrapping_add(cost.wrapping_mul(count - 1));
    }
    pub fn insert(&mut self, name: &[u8]) -> bool {
        self.put(name, true)
    }
    pub fn insert_after_lookup(&mut self, name: &[u8]) -> bool {
        self.put(name, false)
    }
    fn put(&mut self, name: &[u8], counted: bool) -> bool {
        let (index, found) = self.locate(name, counted);
        if found {
            return false;
        }
        let layout = std::sync::Arc::make_mut(&mut self.slots);
        self.empty -= usize::from(layout.slots[index] == 0);
        let key = layout.store(name);
        layout.slots[index] = key;
        self.stats.filled += 1;
        if self.empty < layout.slots.len() / 16 {
            let capacity = layout.slots.len() - layout.slots.len() / 16;
            let size = layout.slots.len() * if self.stats.filled >= capacity { 2 } else { 1 };
            let previous = std::mem::replace(&mut layout.slots, vec![0; size]);
            self.stats.rehashes = self.stats.rehashes.wrapping_add(1);
            self.stats.buckets = size;
            self.empty = size - self.stats.filled;
            for slot in previous.into_iter().filter(|slot| *slot > 1) {
                let (next, _, collisions) = Self::probe(self.kind, layout, layout.name(slot));
                self.stats.lookups = self.stats.lookups.wrapping_add(1);
                self.stats.collisions = self.stats.collisions.wrapping_add(collisions);
                layout.slots[next] = slot;
            }
            self.locate(name, true);
        }
        true
    }
    pub fn remove(&mut self, name: &[u8]) -> bool {
        self.remove_using(name, true)
    }
    pub fn remove_after_lookup(&mut self, name: &[u8]) -> bool {
        self.remove_using(name, false)
    }
    fn remove_using(&mut self, name: &[u8], counted: bool) -> bool {
        let (index, found) = self.locate(name, counted);
        if found {
            std::sync::Arc::make_mut(&mut self.slots).retire(index);
            self.stats.filled -= 1;
        }
        found
    }
    pub fn names(&self) -> impl Iterator<Item = &[u8]> {
        self.slots
            .slots
            .iter()
            .filter(|slot| **slot > 1)
            .map(|slot| self.slots.name(*slot))
    }
}

/// Shared logical identity for one GNU variable/file table. Cloned runtime
/// views of the same scope keep a common history; newly created scopes get a
/// separate table. Nothing consults the Rust map's implementation statistics.
#[derive(Clone, Debug)]
pub struct TableHistory(std::sync::Arc<TableHistoryState>);
#[derive(Debug)]
struct TableHistoryState {
    table: std::sync::Mutex<TableTelemetry>,
    strings: Option<std::sync::Arc<std::sync::Mutex<StringCacheTelemetry>>>,
}

impl TableHistory {
    pub fn new(kind: HashKind, buckets: usize) -> Self {
        Self::with_strings(kind, buckets, None)
    }
    fn with_strings(
        kind: HashKind,
        buckets: usize,
        strings: Option<std::sync::Arc<std::sync::Mutex<StringCacheTelemetry>>>,
    ) -> Self {
        Self(std::sync::Arc::new(TableHistoryState {
            table: std::sync::Mutex::new(
                TableTelemetry::new(kind, buckets).expect("fixed compatible table capacity"),
            ),
            strings,
        }))
    }
    fn with<R>(&self, visit: impl FnOnce(&mut TableTelemetry) -> R) -> R {
        visit(
            &mut self
                .0
                .table
                .lock()
                .unwrap_or_else(|error| error.into_inner()),
        )
    }
    pub(crate) fn registered_function_name(&self, name: &[u8]) {
        if let Some(strings) = &self.0.strings {
            strings
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .intern(name);
        }
    }
    pub fn stats(&self) -> HashStats {
        self.with(|table| table.stats())
    }
    pub(crate) fn same_table(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
    pub(crate) fn ordered_items<T>(&self, mut select: impl FnMut(&[u8]) -> Option<T>) -> Vec<T> {
        self.with(|table| table.names().filter_map(&mut select).collect())
    }
    pub fn lookup(&self, name: &[u8]) -> bool {
        self.with(|table| table.lookup(name))
    }
    pub fn enter(&self, name: &[u8]) -> bool {
        self.with(|table| table.insert(name))
    }
    pub fn enter_after_lookup(&self, name: &[u8]) -> bool {
        self.with(|table| table.insert_after_lookup(name))
    }
    pub fn remove(&self, name: &[u8]) -> bool {
        self.with(|table| table.remove(name))
    }
    pub fn remove_after_lookup(&self, name: &[u8]) -> bool {
        self.with(|table| table.remove_after_lookup(name))
    }
    pub fn names(&self) -> Vec<Vec<u8>> {
        self.with(|table| table.names().map(<[u8]>::to_vec).collect())
    }
}

/// Deferred counters for accepted effect-free plans. Each table contributes
/// one immutable layout snapshot, independent of the number or length of reads.
/// No insertion, deletion, or user effect occurs between preparing and committing
/// an accepted prefix. Discarding a plan discards its observations as well.
#[derive(Default)]
pub(crate) struct LookupBatch {
    tables: Vec<LookupTable>,
}
struct LookupTable {
    history: TableHistory,
    layout: std::sync::Arc<TableLayout>,
    kind: HashKind,
    lookups: u64,
    collisions: u64,
}
impl LookupTable {
    fn same_layout(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.layout, &other.layout)
    }
    fn combine(&mut self, other: Self) -> Option<()> {
        // A caller with a mutable table must finish its selected prefix before
        // changing bindings. A changed snapshot cannot be merged speculatively.
        self.same_layout(&other).then(|| {
            self.lookups = self.lookups.wrapping_add(other.lookups);
            self.collisions = self.collisions.wrapping_add(other.collisions);
        })
    }
}
impl LookupBatch {
    pub(crate) fn add(&mut self, history: &TableHistory, name: &[u8], count: u64) -> Option<()> {
        if count == 0 {
            return Some(());
        }
        let index = if let Some(index) = self
            .tables
            .iter()
            .position(|table| std::sync::Arc::ptr_eq(&table.history.0, &history.0))
        {
            index
        } else {
            if self.tables.len()
                >= crate::variables::MAX_OUTPUT / std::mem::size_of::<LookupTable>()
            {
                return None;
            }
            self.tables.push(history.with(|table| LookupTable {
                history: history.clone(),
                layout: table.slots.clone(),
                kind: table.kind,
                lookups: 0,
                collisions: 0,
            }));
            self.tables.len() - 1
        };
        let table = &mut self.tables[index];
        let (_, _, collisions) = TableTelemetry::probe(table.kind, &table.layout, name);
        table.lookups = table.lookups.wrapping_add(count);
        table.collisions = table
            .collisions
            .wrapping_add(collisions.wrapping_mul(count));
        Some(())
    }
    pub(crate) fn extend(&mut self, other: Self) -> Option<()> {
        for table in other.tables {
            if let Some(old) = self
                .tables
                .iter_mut()
                .find(|old| std::sync::Arc::ptr_eq(&old.history.0, &table.history.0))
            {
                old.combine(table)?;
            } else {
                if self.tables.len()
                    >= crate::variables::MAX_OUTPUT / std::mem::size_of::<LookupTable>()
                {
                    return None;
                }
                self.tables.push(table);
            }
        }
        Some(())
    }
    pub(crate) fn commit(self) {
        for batch in self.tables {
            batch.history.with(|table| {
                // All callers borrow the selected bindings immutably and commit
                // before any effectful leaf. The owned snapshot also makes a
                // rejected plan harmless if its source is subsequently edited.
                debug_assert!(std::sync::Arc::ptr_eq(&table.slots, &batch.layout));
                table.stats.lookups = table.stats.lookups.wrapping_add(batch.lookups);
                table.stats.collisions = table.stats.collisions.wrapping_add(batch.collisions);
            });
        }
    }
}

/// Opt-in diagnostic history, enabled before initial imports/defaults. The
/// application keeps it alive even if the option is first selected later by
/// MAKEFLAGS or eval. It contains only actual logical events, never replayed
/// expansions/searches/effects. Production enablement remains a host decision.
#[derive(Clone, Debug)]
pub struct History {
    variables: TableHistory,
    files: TableHistory,
    directories: TableHistory,
    builtin_files: std::sync::Arc<std::sync::Mutex<std::collections::HashSet<Vec<u8>>>>,
    strings: std::sync::Arc<std::sync::Mutex<StringCacheTelemetry>>,
}
impl Default for History {
    fn default() -> Self {
        let strings = std::sync::Arc::new(std::sync::Mutex::new(StringCacheTelemetry::default()));
        Self {
            variables: TableHistory::with_strings(HashKind::Variable, 1024, Some(strings.clone())),
            files: TableHistory::new(HashKind::Filename, 1024),
            directories: TableHistory::new(HashKind::Filename, 256),
            builtin_files: std::sync::Arc::default(),
            strings,
        }
    }
}
impl History {
    pub fn variable_table(&self) -> TableHistory {
        self.variables.clone()
    }
    pub fn variable_stats(&self) -> HashStats {
        self.variables.stats()
    }
    pub fn file_stats(&self) -> HashStats {
        self.files.stats()
    }
    pub fn record_variable_lookup(&self, name: &[u8]) -> bool {
        self.variables.lookup(name)
    }
    pub fn record_file_lookup(&self, name: &[u8]) -> bool {
        self.files.lookup(name)
    }
    pub fn record_file_enter(&self, name: &[u8]) -> bool {
        self.builtin_files
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(name);
        self.files.enter(name)
    }
    pub fn mark_file_builtin(&self, name: &[u8]) {
        self.builtin_files
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(name.to_vec());
    }
    pub fn is_builtin_file(&self, name: &[u8]) -> bool {
        self.builtin_files
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .contains(name)
    }
    pub fn record_file_remove(&self, name: &[u8]) -> bool {
        self.files.remove(name)
    }
    pub fn record_directory(&self, name: &[u8]) {
        self.directories.enter(name);
    }
    pub fn directory_names(&self) -> Vec<Vec<u8>> {
        self.directories.names()
    }
    pub fn intern(&self, value: &[u8]) -> Interned {
        self.strings
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .intern(value)
    }
    /// Record a supplied filename-buffer span. Identity ends at the first NUL,
    /// while a new cache allocation retains the complete supplied extent.
    pub fn intern_buffer(&self, value: &[u8]) -> Interned {
        self.strings
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .intern_buffer(value)
    }
    pub fn with_string_cache<R>(&self, visit: impl FnOnce(&StringCacheTelemetry) -> R) -> R {
        visit(
            &self
                .strings
                .lock()
                .unwrap_or_else(|error| error.into_inner()),
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub enum DirectoryState {
    Missing,
    Unopened {
        device: i64,
        inode: i64,
    },
    Enumerated {
        device: i64,
        inode: i64,
        files: usize,
        impossible: usize,
        complete: bool,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct DirectoryView<'a> {
    pub name: &'a [u8],
    pub state: DirectoryState,
}

pub fn write_directories(
    output: &mut impl Write,
    directories: &[DirectoryView<'_>],
) -> io::Result<()> {
    write_directories_with_presentation(output, directories, ENGLISH)
}

fn write_directories_with_presentation(
    output: &mut impl Write,
    directories: &[DirectoryView<'_>],
    presentation: Presentation<'_>,
) -> io::Result<()> {
    presentation.line(output, c"\n# Directories\n")?;
    let mut files = 0;
    let mut impossible = 0;
    for directory in directories {
        let name = Argument::Bytes(directory.name);
        match directory.state {
            DirectoryState::Missing => {
                presentation.format(output, msg::DIRECTORY_MISSING, &[name])?
            }
            DirectoryState::Unopened { device, inode } => presentation.format(
                output,
                msg::DIRECTORY_UNOPENED,
                &[name, Argument::Signed(device), Argument::Signed(inode)],
            )?,
            DirectoryState::Enumerated {
                device,
                inode,
                files: count,
                impossible: rejected,
                complete,
            } => {
                presentation.format(
                    output,
                    msg::DIRECTORY_ENUMERATED,
                    &[name, Argument::Signed(device), Argument::Signed(inode)],
                )?;
                write_directory_counts(output, count, rejected, presentation)?;
                presentation.text(output, c" impossibilities")?;
                if complete {
                    output.write_all(b".\n")?;
                } else {
                    presentation.line(output, c" so far.")?;
                }
                files += count;
                impossible += rejected;
            }
        }
    }
    output.write_all(b"\n# ")?;
    write_directory_counts(output, files, impossible, presentation)?;
    presentation.format(
        output,
        msg::DIRECTORY_TOTAL,
        &[Argument::Unsigned(directories.len() as u64)],
    )
}

fn write_directory_counts(
    output: &mut impl Write,
    files: usize,
    impossible: usize,
    presentation: Presentation<'_>,
) -> io::Result<()> {
    if files == 0 {
        presentation.text(output, c"No")?;
    } else {
        write!(output, "{files}")?;
    }
    presentation.text(output, c" files, ")?;
    if impossible == 0 {
        presentation.text(output, c"no")
    } else {
        write!(output, "{impossible}")
    }
}

#[derive(Clone, Copy, Debug)]
pub enum FileTime<'a> {
    Unchecked,
    Missing,
    Old,
    Formatted(&'a [u8]),
}

#[derive(Clone, Copy, Debug, Default)]
pub enum FileStatus {
    #[default]
    Unattempted,
    Success,
    Question,
    Failed,
    Running,
    DependenciesRunning,
}

#[derive(Clone, Copy, Debug)]
pub struct AutomaticVariables<'a> {
    pub definitions: &'a [(&'a [u8], &'a Variable)],
    pub stats: HashStats,
}

#[derive(Clone, Copy, Debug)]
pub struct FileView<'a> {
    pub name: &'a [u8],
    pub rule: Option<&'a Rule>,
    pub expanded_prerequisites: Option<&'a [Dependency]>,
    pub is_target: bool,
    pub precious: bool,
    pub phony: bool,
    pub command_line: bool,
    pub dont_care: bool,
    pub builtin: bool,
    pub implicit_tried: bool,
    pub stem: Option<&'a [u8]>,
    pub intermediate: bool,
    pub not_intermediate: bool,
    pub secondary: bool,
    pub also_make: &'a [&'a [u8]],
    pub time: FileTime<'a>,
    pub updated: bool,
    pub status: FileStatus,
    pub double_colon: bool,
    pub target_variables: Option<&'a crate::variables::TargetScope>,
    /// Actual runtime local definitions, when a saved file scope is available.
    pub local_variables: Option<&'a [(&'a [u8], &'a Variable)]>,
    pub automatic_variables: Option<AutomaticVariables<'a>>,
}

impl<'a> FileView<'a> {
    pub fn unchecked(name: &'a [u8], rule: Option<&'a Rule>) -> Self {
        Self {
            name,
            rule,
            expanded_prerequisites: None,
            is_target: rule.is_some_and(|rule| !rule.record_only),
            precious: false,
            phony: false,
            command_line: false,
            dont_care: false,
            builtin: false,
            implicit_tried: false,
            stem: None,
            intermediate: false,
            not_intermediate: false,
            secondary: false,
            also_make: &[],
            time: FileTime::Unchecked,
            updated: false,
            status: FileStatus::Unattempted,
            double_colon: rule.is_some_and(|rule| rule.double_colon.is_some()),
            target_variables: None,
            local_variables: None,
            automatic_variables: None,
        }
    }
}

/// Stateful only for the recipe prefix selected by preceding printed records.
/// The caller supplies file traversal/branch order and cached runtime facts.
pub struct FilePrinter<'a, W> {
    output: &'a mut W,
    recipe_prefix: u8,
    presentation: Presentation<'a>,
    names: Option<&'a dyn FileNames>,
}

impl<'a, W: Write> FilePrinter<'a, W> {
    pub fn new(output: &'a mut W) -> Self {
        Self::with_numeric(output, &RustNumericFormatting)
    }
    pub fn with_numeric(output: &'a mut W, numeric: &'a dyn NumericFormatting) -> Self {
        Self::with_numeric_and_names(output, numeric, None)
    }
    pub fn with_numeric_and_names(
        output: &'a mut W,
        numeric: &'a dyn NumericFormatting,
        names: Option<&'a dyn FileNames>,
    ) -> Self {
        Self::with_presentation_and_names(output, Presentation::english(numeric), names)
    }

    fn with_presentation_and_names(
        output: &'a mut W,
        presentation: Presentation<'a>,
        names: Option<&'a dyn FileNames>,
    ) -> Self {
        Self {
            output,
            recipe_prefix: b'\t',
            presentation,
            names,
        }
    }

    pub fn write(&mut self, file: FileView<'_>) -> io::Result<()> {
        let names = self.names;
        let presentation = self.presentation;
        let file_name = resolved_file_name(names, file.name);
        let output = &mut self.output;
        output.write_all(b"\n")?;
        if let Some(recipes) = file.rule.and_then(|rule| rule.recipes.as_ref()) {
            let prefix = recipes.first().map_or(b'\t', |recipe| recipe.prefix);
            if prefix != self.recipe_prefix {
                output.write_all(b".RECIPEPREFIX = ")?;
                if prefix != b'\t' {
                    output.write_all(&[prefix])?;
                }
                output.write_all(b"\n")?;
                self.recipe_prefix = prefix;
            }
        }
        let declared = file
            .target_variables
            .map(|scope| scope.database_definitions().collect::<Vec<_>>());
        if let Some(variables) = file.local_variables.or(declared.as_deref()) {
            for &(name, variable) in variables {
                if variable.origin() != Origin::Automatic {
                    write_definition(
                        output,
                        variable.origin(),
                        variable.is_private(),
                        variable.definition(),
                        presentation,
                    )?;
                    output.write_all(c_bytes(file_name))?;
                    write_value(
                        output,
                        b": ",
                        name,
                        variable.raw_value(),
                        variable.flavor(),
                        variable.is_outer_append(),
                    )?;
                }
            }
        }
        if !file.is_target {
            presentation.line(output, c"# Not a target:")?;
        }
        output.write_all(c_bytes(file_name))?;
        output.write_all(if file.double_colon { b"::" } else { b":" })?;
        let (dependencies, deferred) = match (file.expanded_prerequisites, file.rule) {
            (Some(dependencies), _) => (dependencies, None),
            (None, Some(rule)) => (
                rule.dependencies.as_slice(),
                rule.secondary.as_deref().map(Vec::as_slice),
            ),
            (None, None) => (&[][..], None),
        };
        write_prerequisites_with_names(output, dependencies, deferred, names)?;
        for (selected, text) in [
            (
                file.precious,
                c"#  Precious file (prerequisite of .PRECIOUS).",
            ),
            (file.phony, c"#  Phony target (prerequisite of .PHONY)."),
            (file.command_line, c"#  Command line target."),
            (
                file.dont_care,
                c"#  A default, MAKEFILES, or -include/sinclude makefile.",
            ),
            (file.builtin, c"#  Builtin rule"),
        ] {
            if selected {
                presentation.line(output, text)?;
            }
        }
        presentation.line(
            output,
            if file.implicit_tried {
                c"#  Implicit rule search has been done."
            } else {
                c"#  Implicit rule search has not been done."
            },
        )?;
        if let Some(stem) = file.stem {
            presentation.format(output, msg::STEM, &[Argument::Bytes(stem)])?;
        }
        for (selected, text) in [
            (
                file.intermediate,
                c"#  File is an intermediate prerequisite.",
            ),
            (
                file.not_intermediate,
                c"#  File is a prerequisite of .NOTINTERMEDIATE.",
            ),
            (
                file.secondary,
                c"#  File is secondary (prerequisite of .SECONDARY).",
            ),
        ] {
            if selected {
                presentation.line(output, text)?;
            }
        }
        if !file.also_make.is_empty() {
            presentation.text(output, c"#  Also makes:")?;
            for name in file.also_make {
                output.write_all(b" ")?;
                output.write_all(c_bytes(name))?;
            }
            output.write_all(b"\n")?;
        }
        match file.time {
            FileTime::Unchecked => {
                presentation.line(output, c"#  Modification time never checked.")?
            }
            FileTime::Missing => presentation.line(output, c"#  File does not exist.")?,
            FileTime::Old => presentation.line(output, c"#  File is very old.")?,
            FileTime::Formatted(value) => {
                presentation.format(output, msg::MTIME, &[Argument::Bytes(value)])?;
            }
        }
        presentation.line(
            output,
            if file.updated {
                c"#  File has been updated."
            } else {
                c"#  File has not been updated."
            },
        )?;
        let status = match file.status {
            FileStatus::Unattempted => None,
            FileStatus::Success => Some(c"#  Successfully updated."),
            FileStatus::Question => Some(c"#  Needs to be updated (-q is set)."),
            FileStatus::Failed => Some(c"#  Failed to be updated."),
            FileStatus::Running => Some(c"#  Recipe currently running (THIS IS A BUG)."),
            FileStatus::DependenciesRunning => {
                Some(c"#  Dependencies recipe running (THIS IS A BUG).")
            }
        };
        if let Some(status) = status {
            presentation.line(output, status)?;
        }
        if let Some(variables) = file.automatic_variables {
            for (name, variable) in variables.definitions {
                if variable.origin() == Origin::Automatic {
                    write_variable_value(
                        output,
                        b"# ",
                        name,
                        variable,
                        variable.raw_value(),
                        presentation,
                    )?;
                }
            }
            write_variable_stats(output, variables.stats, presentation)?;
        }
        if let Some(rule) = file.rule
            && let Some(recipes) = &rule.recipes
        {
            write_recipes_with_presentation(
                output,
                recipes,
                rule.recipe_location.as_ref(),
                presentation,
            )?;
        }
        Ok(())
    }
}

/// Write file records in the host's already-established GNU slot/branch order.
/// No map lookup, timestamp query, or implicit resolution occurs here.
pub fn write_files(
    output: &mut impl Write,
    files: &[FileView<'_>],
    stats: HashStats,
) -> io::Result<()> {
    write_files_with_presentation(output, files, stats, ENGLISH, None)
}

fn write_files_with_presentation(
    output: &mut impl Write,
    files: &[FileView<'_>],
    stats: HashStats,
    presentation: Presentation<'_>,
    names: Option<&dyn FileNames>,
) -> io::Result<()> {
    presentation.line(output, c"\n# Files")?;
    let mut printer = FilePrinter::with_presentation_and_names(output, presentation, names);
    for file in files {
        printer.write(*file)?;
    }
    presentation.text(output, c"\n# files hash-table stats:\n# ")?;
    // The following VPATH heading starts with its one protocol newline.
    write_hash_stats_with_presentation(output, stats, presentation)
}

/// Immutable diagnostic inputs. The runtime owns observations and logical
/// event history; this view deliberately cannot derive either from final maps.
pub struct DatabaseView<'a> {
    pub database: &'a Database,
    pub variable_stats: HashStats,
    pub directories: &'a [DirectoryView<'a>],
    pub files: &'a [FileView<'a>],
    pub file_stats: HashStats,
    pub general_vpath: &'a [&'a [u8]],
    pub string_cache: &'a StringCacheTelemetry,
}

pub fn write_database_contents(output: &mut impl Write, view: DatabaseView<'_>) -> io::Result<()> {
    write_database_contents_with_numeric(output, view, &RustNumericFormatting)
}

pub fn write_database_contents_with_numeric(
    output: &mut impl Write,
    view: DatabaseView<'_>,
    numeric: &dyn NumericFormatting,
) -> io::Result<()> {
    write_database_contents_with_numeric_and_names(output, view, numeric, None)
}

pub fn write_database_contents_with_numeric_and_names(
    output: &mut impl Write,
    view: DatabaseView<'_>,
    numeric: &dyn NumericFormatting,
    names: Option<&dyn FileNames>,
) -> io::Result<()> {
    Presentation::english(numeric).contents(output, view, names)?;
    output.write_all(b"\n")
}

impl Presentation<'_> {
    pub fn contents(
        self,
        output: &mut impl Write,
        view: DatabaseView<'_>,
        names: Option<&dyn FileNames>,
    ) -> io::Result<()> {
        write_variables_with_presentation(
            output,
            &view.database.variables,
            view.variable_stats,
            self,
        )?;
        write_pattern_variables_with_presentation(output, view.database, self)?;
        write_directories_with_presentation(output, view.directories, self)?;
        write_implicit_rules_with_presentation(output, view.database, self)?;
        write_files_with_presentation(output, view.files, view.file_stats, self, names)?;
        write_vpaths_with_presentation(output, view.database, view.general_vpath, self)?;
        view.string_cache.write_with_presentation(output, self)
    }
}

/// The host supplies the actual C-locale clock separately at each emission
/// point, including fatal exits and pre-reexec dumps. It must not substitute a
/// fixed process-start timestamp for the closing observation.
pub fn write_database_open(output: &mut impl Write, ctime: &[u8]) -> io::Result<()> {
    output.write_all(b"\n# Make data base, printed on ")?;
    output.write_all(ctime)?;
    output.write_all(b"\n")
}

pub fn write_database_close(output: &mut impl Write, ctime: &[u8]) -> io::Result<()> {
    output.write_all(b"# Finished Make data base on ")?;
    output.write_all(ctime)?;
    output.write_all(b"\n\n")
}

#[derive(Clone, Copy, Debug, Default)]
struct CacheBuffer {
    size: usize,
    used: usize,
    strings: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interned {
    Existing,
    New,
    UncachedHuge,
}

/// Logical string-cache keys and buffer accounting. The application records
/// actual additions from startup; this arena contains no interpreter values.
#[derive(Clone, Debug)]
pub struct StringCacheTelemetry {
    names: TableTelemetry,
    available: Vec<CacheBuffer>,
    full: Vec<CacheBuffer>,
    adds: u64,
    storage: u64,
    strings: u64,
    capacity: usize,
}

impl Default for StringCacheTelemetry {
    fn default() -> Self {
        Self {
            names: TableTelemetry::new(HashKind::Filename, 8192).unwrap(),
            available: Vec::new(),
            full: Vec::new(),
            adds: 0,
            storage: 0,
            strings: 0,
            capacity: 8162,
        }
    }
}

impl StringCacheTelemetry {
    pub fn intern(&mut self, value: &[u8]) -> Interned {
        self.intern_buffer(c_bytes(value))
    }

    pub fn intern_buffer(&mut self, value: &[u8]) -> Interned {
        if value.len() > 65534 {
            return Interned::UncachedHuge;
        }
        self.adds = self.adds.wrapping_add(1);
        if !self.names.insert(c_bytes(value)) {
            return Interned::Existing;
        }
        let bytes = value.len() + 1;
        self.strings = self.strings.wrapping_add(1);
        self.storage = self.storage.wrapping_add(bytes as u64);
        if bytes > self.capacity {
            self.full.push(CacheBuffer {
                size: bytes,
                used: bytes,
                strings: 1,
            });
        } else {
            let index = self
                .available
                .iter()
                .position(|buffer| buffer.size - buffer.used > bytes)
                .unwrap_or_else(|| {
                    self.available.insert(
                        0,
                        CacheBuffer {
                            size: self.capacity,
                            ..CacheBuffer::default()
                        },
                    );
                    0
                });
            let buffer = &mut self.available[index];
            buffer.used += bytes;
            buffer.strings += 1;
            if self.strings > 20
                && buffer.size - buffer.used < (self.storage / self.strings) as usize + 1
            {
                self.full.push(self.available.remove(index));
            }
        }
        Interned::New
    }
    pub fn hash_stats(&self) -> HashStats {
        self.names.stats()
    }
    pub fn write(&self, output: &mut impl Write) -> io::Result<()> {
        self.write_with_numeric(output, &RustNumericFormatting)
    }
    pub fn write_with_numeric(
        &self,
        output: &mut impl Write,
        numeric: &dyn NumericFormatting,
    ) -> io::Result<()> {
        self.write_with_presentation(output, Presentation::english(numeric))?;
        output.write_all(b"\n")
    }

    fn write_with_presentation(
        &self,
        output: &mut impl Write,
        presentation: Presentation<'_>,
    ) -> io::Result<()> {
        let prefix = Argument::Bytes(b"#");
        let Some(current) = self.available.first() else {
            return presentation.format(output, msg::CACHE_EMPTY, &[prefix]);
        };
        let buffers = self.available.len() + self.full.len();
        presentation.format(
            output,
            msg::CACHE_BUFFERS,
            &[
                prefix,
                Argument::Unsigned(buffers as u64),
                Argument::Unsigned(self.full.len() as u64),
                Argument::Unsigned(self.strings),
                Argument::Unsigned(self.storage),
                Argument::Unsigned(self.storage / self.strings),
            ],
        )?;
        presentation.format(
            output,
            msg::CACHE_CURRENT,
            &[
                prefix,
                Argument::Unsigned(self.capacity as u64),
                Argument::Unsigned(current.used as u64),
                Argument::Unsigned(current.strings as u64),
                Argument::Unsigned((current.used / current.strings) as u64),
            ],
        )?;
        if buffers > 1 {
            let others = self.available.iter().skip(1).chain(&self.full);
            let mut free = 0;
            let mut min = usize::MAX;
            let mut max = 0;
            for buffer in others {
                let unused = buffer.size - buffer.used;
                free += unused;
                min = min.min(unused);
                max = max.max(unused);
            }
            let used = self.storage - current.used as u64;
            let count = self.strings - current.strings as u64;
            presentation.format(
                output,
                msg::CACHE_OTHER_USED,
                &[
                    prefix,
                    Argument::Unsigned(used),
                    Argument::Unsigned(count),
                    Argument::Unsigned(used / count),
                ],
            )?;
            presentation.format(
                output,
                msg::CACHE_OTHER_FREE,
                &[
                    prefix,
                    Argument::Unsigned(free as u64),
                    Argument::Unsigned(max as u64),
                    Argument::Unsigned(min as u64),
                    Argument::Unsigned((free / (buffers - 1)) as u64),
                ],
            )?;
        }
        presentation.format(
            output,
            msg::CACHE_PERFORMANCE,
            &[
                prefix,
                Argument::Unsigned(self.adds),
                Argument::Unsigned(
                    (100.0 * (self.adds - self.strings) as f64 / self.adds as f64) as u64,
                ),
            ],
        )?;
        presentation.text(output, c"# hash-table stats:\n# ")?;
        write_hash_stats_with_presentation(output, self.names.stats(), presentation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supplied_filename_span_keeps_c_identity_and_first_allocation_extent() {
        let mut cache = StringCacheTelemetry::default();
        assert_eq!(cache.intern_buffer(b"path\0"), Interned::New);
        assert_eq!(cache.storage, 6); // supplied span plus its final terminator
        assert_eq!(cache.intern(b"path"), Interned::Existing);
        assert_eq!(cache.intern_buffer(b"path\0unused"), Interned::Existing);
        assert_eq!(cache.storage, 6);
        assert_eq!(cache.strings, 1);
        assert_eq!(cache.hash_stats().lookups, 3);
    }

    #[test]
    fn runtime_filename_views_resolve_diagnostics_without_mutating_declarations() {
        struct Names;
        impl FileNames for Names {
            fn resolve<'a>(&'a self, name: &'a [u8]) -> &'a [u8] {
                match name {
                    b"all" => b"remote/all",
                    b"input" => b"remote/input",
                    b"order" => b"remote/order",
                    b"peer" => b"remote/peer",
                    _ => name,
                }
            }
        }
        let mut database = Database::default();
        database.read(b"all: input | order\n", b"Makefile").unwrap();
        let rule = &database.rules[b"all".as_slice()];
        let peers = [b"peer".as_slice()];
        let mut view = FileView::unchecked(b"all", Some(rule));
        view.also_make = &peers;
        let mut output = Vec::new();
        FilePrinter::with_numeric_and_names(&mut output, &RustNumericFormatting, Some(&Names))
            .write(view)
            .unwrap();
        assert!(output.starts_with(b"\nremote/all: remote/input | remote/order\n"));
        assert!(
            output
                .windows(b"#  Also makes: peer\n".len())
                .any(|text| text == b"#  Also makes: peer\n")
        );
        assert_eq!(rule.dependencies[0].name, b"input");
        assert_eq!(rule.dependencies[1].name, b"order");
    }
    use crate::variables::Assignment;

    #[test]
    fn raw_variable_values_do_not_expand_and_keep_protocol_escaping() {
        let mut store = VariableStore::new();
        store
            .define(
                b"DB_R",
                b"$(file >forbidden,NO)",
                Assignment::Recursive,
                Origin::File,
            )
            .unwrap();
        store
            .define_literal(b"DB_D", b"$cash", Origin::File)
            .unwrap();
        store.define_literal(b"DB_S", b" ", Origin::File).unwrap();
        store
            .define(b"DB_M", b"one\ntwo", Assignment::Recursive, Origin::File)
            .unwrap();
        let mut output = Vec::new();
        for name in [b"DB_R".as_slice(), b"DB_D", b"DB_S", b"DB_M"] {
            write_variable(&mut output, b"", name, store.get(name).unwrap()).unwrap();
        }
        assert_eq!(output, b"# makefile\nDB_R = $(file >forbidden,NO)\n# makefile\nDB_D := $$cash\n# makefile\nDB_S := $(subst ,, )\n# makefile\ndefine DB_M\none\ntwo\nendef\n");
    }

    #[test]
    fn global_view_has_native_order_without_flattening_scope_or_expanding_metadata() {
        let mut store = VariableStore::new();
        store.enable_variable_names().unwrap();
        for name in [b"DB_Z".as_slice(), b"DB_A", b"DB_M"] {
            store.define_literal(name, name, Origin::File).unwrap();
        }
        let names = store
            .database_definitions()
            .map(|(name, _)| name)
            .collect::<Vec<_>>();
        assert_ne!(names, {
            let mut sorted = names.clone();
            sorted.sort();
            sorted
        });
        assert_eq!(store.get(b".VARIABLES").unwrap().raw_value(), b"");
    }

    #[test]
    fn hash_statistics_are_exact_and_zero_safe() {
        let mut output = Vec::new();
        write_hash_stats(
            &mut output,
            HashStats {
                filled: 40,
                buckets: 1024,
                lookups: 72,
                collisions: 1,
                ..HashStats::default()
            },
        )
        .unwrap();
        assert_eq!(output, b"Load=40/1024=4%, Rehash=0, Collisions=1/72=1%");
    }

    #[test]
    fn logical_events_preserve_lookup_growth_and_deletion_history() {
        let mut table = TableTelemetry::new(HashKind::Variable, 16).unwrap();
        for index in 0..16 {
            assert!(table.insert(format!("V{index}").as_bytes()));
        }
        assert_eq!(table.stats().filled, 16);
        assert_eq!(table.stats().buckets, 32);
        assert_eq!(table.stats().rehashes, 1);
        assert_eq!(table.stats().lookups, 33);
        assert!(table.remove(b"V3"));
        assert!(!table.lookup(b"V3"));
        assert!(table.insert_after_lookup(b"V3"));
        assert_eq!(table.stats().filled, 16);
        assert_eq!(table.names().count(), 16);
    }

    #[test]
    fn writer_failure_propagates_without_executing_any_make_operation() {
        struct Failure;
        impl Write for Failure {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("stopped"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut store = VariableStore::new();
        store
            .define_literal(b"RAW", b"value", Origin::File)
            .unwrap();
        assert!(write_variable(&mut Failure, b"", b"RAW", store.get(b"RAW").unwrap()).is_err());
    }
    #[test]
    fn directory_views_keep_os_identity_and_partial_enumeration_explicit() {
        let mut output = Vec::new();
        write_directories(
            &mut output,
            &[
                DirectoryView {
                    name: b".",
                    state: DirectoryState::Enumerated {
                        device: 2080,
                        inode: 42,
                        files: 4,
                        impossible: 0,
                        complete: true,
                    },
                },
                DirectoryView {
                    name: b"missing",
                    state: DirectoryState::Missing,
                },
            ],
        )
        .unwrap();
        assert_eq!(output, b"\n# Directories\n\n# . (device 2080, inode 42): 4 files, no impossibilities.\n# missing: could not be stat'd.\n\n# 4 files, no impossibilities in 2 directories.\n");
    }

    #[test]
    fn file_view_requires_cached_state_without_implicit_search_or_stat() {
        let mut output = Vec::new();
        let mut file = FileView::unchecked(b"missing.mk", None);
        file.implicit_tried = true;
        file.time = FileTime::Missing;
        file.updated = true;
        file.status = FileStatus::Failed;
        FilePrinter::new(&mut output).write(file).unwrap();
        assert_eq!(output, b"\n# Not a target:\nmissing.mk:\n#  Implicit rule search has been done.\n#  File does not exist.\n#  File has been updated.\n#  Failed to be updated.\n");
    }

    #[test]
    fn branch_view_prints_runtime_locals_and_explicit_double_colon() {
        let mut variables = VariableStore::new();
        variables
            .define_literal(b"RUNTIME", b"local", Origin::Environment)
            .unwrap();
        variables
            .define_literal(b"@", b"branch", Origin::Automatic)
            .unwrap();
        let definitions: Vec<_> = variables.database_definitions().collect();
        let mut file = FileView::unchecked(b"branch", None);
        file.is_target = true;
        file.double_colon = true;
        file.local_variables = Some(&definitions);
        let mut output = Vec::new();
        FilePrinter::new(&mut output).write(file).unwrap();
        assert!(output.starts_with(b"\n# environment\nbranch: RUNTIME := local\nbranch::\n"));
        assert!(!output.windows(3).any(|bytes| bytes == b"@ ="));
    }

    #[test]
    fn string_cache_counts_unique_bytes_and_hits_without_an_arena() {
        let mut cache = StringCacheTelemetry::default();
        assert_eq!(cache.intern(b"all"), Interned::New);
        assert_eq!(cache.intern(b"Makefile"), Interned::New);
        assert_eq!(cache.intern(b"all"), Interned::Existing);
        let mut output = Vec::new();
        cache.write(&mut output).unwrap();
        assert!(output.starts_with(b"\n# strcache buffers: 1 (0) / strings = 2 / storage = 13 B / avg = 6 B\n# current buf: size = 8162 B / used = 13 B / count = 2 / avg = 6 B\n"));
        assert!(
            output
                .windows(b"lookups = 3 / hit rate = 33%".len())
                .any(|part| part == b"lookups = 3 / hit rate = 33%")
        );
        assert_eq!(cache.intern(&vec![b'x'; 65535]), Interned::UncachedHuge);
        assert_eq!(cache.hash_stats().filled, 2);
    }
    #[test]
    fn original_pattern_implicit_and_vpath_sections_keep_declaration_protocol() {
        let mut database = Database::default();
        database.read(
            b"%.o: DB_PATTERN = $(DB_BASE)\n%.out: %.in | stamp\n\t@:\n%.bad:: %.seed\n\t@:\nvpath %.c search fallback\nvpath %.h fallback\n",
            b"Makefile",
        ).unwrap();
        let mut output = Vec::new();
        write_pattern_variables(&mut output, &database).unwrap();
        assert_eq!(output, b"\n# Pattern-specific Variable Values\n\n%.o :\n# makefile (from 'Makefile', line 1)\n# DB_PATTERN := $$(DB_BASE)\n\n# 1 pattern-specific variable values");
        output.clear();
        write_implicit_rules(&mut output, &database).unwrap();
        assert_eq!(output, b"\n# Implicit Rules\n\n%.out: %.in | stamp\n#  recipe to execute (from 'Makefile', line 3):\n\t@:\n\n%.bad:: %.seed\n#  recipe to execute (from 'Makefile', line 5):\n\t@:\n\n# 2 implicit rules, 1 (50.0%) terminal.");
        output.clear();
        write_vpaths(&mut output, &database, &[b"search", b"fallback"]).unwrap();
        assert_eq!(output, b"\n# VPATH Search Paths\n\nvpath %.c search:fallback\nvpath %.h fallback\n\n# 2 'vpath' search paths.\n\n# General ('VPATH' variable) search path:\n# search:fallback\n");
    }

    #[test]
    fn inline_recipe_printing_retains_raw_bytes_after_execution_preparation() {
        let mut database = Database::default();
        database
            .read(
                b".RECIPEPREFIX := >\nall: ; @printf '$(info left \\\n>right)' \\\n>>data\n>@:\n",
                b"Makefile",
            )
            .unwrap();
        let rule = &database.rules[b"all".as_slice()];
        let recipes = rule.recipes.as_ref().unwrap();
        assert_eq!(
            recipes[0].execution_bytes().as_ref(),
            b" @printf '$(info left >right)' \\\n>data"
        );
        assert_eq!(recipes[1].execution_bytes().as_ref(), b"@:");
        let mut output = Vec::new();
        write_recipes(&mut output, recipes, rule.recipe_location.as_ref()).unwrap();
        assert_eq!(
            output,
            b"#  recipe to execute (from 'Makefile', line 2):\n> @printf '$(info left \\\n>right)' \\\n>>data\n>@:\n"
        );
    }

    #[test]
    fn file_records_retain_target_declarations_and_recipe_prefix_transitions() {
        let mut database = Database::default();
        database.read(
            b"all: private DB_TARGET = local\nall: helper helper | order\n\t@:\n.RECIPEPREFIX = >\nnext:;@:\n",
            b"Makefile",
        ).unwrap();
        let mut file = FileView::unchecked(b"all", database.rules.get(b"all".as_slice()));
        file.target_variables = database.target_scopes.get(b"all".as_slice());
        file.phony = true;
        file.time = FileTime::Missing;
        file.updated = true;
        file.status = FileStatus::Success;
        let next = FileView::unchecked(b"next", database.rules.get(b"next".as_slice()));
        let mut output = Vec::new();
        let mut printer = FilePrinter::new(&mut output);
        printer.write(file).unwrap();
        printer.write(next).unwrap();
        assert_eq!(output, b"\n# makefile private (from 'Makefile', line 1)\nall: DB_TARGET = local\nall: helper helper | order\n#  Phony target (prerequisite of .PHONY).\n#  Implicit rule search has not been done.\n#  File does not exist.\n#  File has been updated.\n#  Successfully updated.\n#  recipe to execute (from 'Makefile', line 3):\n\t@:\n\n.RECIPEPREFIX = >\nnext:\n#  Implicit rule search has not been done.\n#  Modification time never checked.\n#  File has not been updated.\n#  recipe to execute (from 'Makefile', line 5):\n>@:\n");
    }

    #[test]
    fn complete_section_order_and_clock_boundaries_do_not_reenter_semantics() {
        let database = Database::default();
        let cache = StringCacheTelemetry::default();
        let mut output = Vec::new();
        write_database_open(&mut output, b"Sun Sep 13 12:26:40 2020").unwrap();
        write_database_contents(
            &mut output,
            DatabaseView {
                database: &database,
                variable_stats: HashStats::default(),
                directories: &[],
                files: &[],
                file_stats: HashStats::default(),
                general_vpath: &[],
                string_cache: &cache,
            },
        )
        .unwrap();
        write_database_close(&mut output, b"Sun Sep 13 12:26:41 2020").unwrap();
        let text = std::str::from_utf8(&output).unwrap();
        let mut end = 0;
        for section in [
            "# Variables\n",
            "# Pattern-specific Variable Values\n",
            "# Directories\n",
            "# Implicit Rules\n",
            "# Files\n",
            "# VPATH Search Paths\n",
            "# No strcache buffers\n",
            "# Finished Make data base on ",
        ] {
            let position = text[end..].find(section).unwrap() + end;
            end = position + section.len();
        }
        assert!(output.starts_with(b"\n# Make data base, printed on Sun Sep 13 12:26:40 2020\n"));
        assert!(output.ends_with(b"# Finished Make data base on Sun Sep 13 12:26:41 2020\n\n"));
        assert!(text.contains("# No strcache buffers\n\n# Finished Make data base on "));
        assert!(!text.contains("Load=0/0=0%, Rehash=0, Collisions=0/0=0%\n\n# VPATH"));
    }
    #[test]
    fn database_prints_observed_metadata_cache_without_refreshing_its_generation() {
        let mut store = VariableStore::new();
        store.enable_variable_names().unwrap();
        store
            .define_literal(b"DB_BEFORE", b"", Origin::File)
            .unwrap();
        let observed = store.expand(b"$(.VARIABLES)").unwrap();
        store
            .define_literal(b"DB_AFTER", b"", Origin::File)
            .unwrap();
        let mut output = Vec::new();
        write_variables(&mut output, &store, HashStats::default()).unwrap();
        let expected = [b".VARIABLES := ".as_slice(), &observed, b"\n"].concat();
        assert!(output.windows(expected.len()).any(|line| line == expected));
        assert_eq!(
            store.database_cached_value(store.get(b".VARIABLES").unwrap()),
            Some(observed)
        );
        assert_eq!(store.get(b".VARIABLES").unwrap().raw_value(), b"");
        store
            .define(
                b".VARIABLES",
                b"$(error must-not-expand)",
                Assignment::Recursive,
                Origin::File,
            )
            .unwrap();
        output.clear();
        write_variables(&mut output, &store, HashStats::default()).unwrap();
        let expected = b".VARIABLES = $(error must-not-expand)\n";
        assert!(output.windows(expected.len()).any(|line| line == expected));
    }
    #[test]
    fn actual_assignment_reference_and_undefine_events_feed_logical_history() {
        let mut database = Database::default();
        database.enable_database_history().unwrap();
        let variables = &mut database.variables;
        variables
            .define_literal(b"A", b"literal", Origin::File)
            .unwrap();
        variables
            .define(b"B", b"$(A)", Assignment::Recursive, Origin::File)
            .unwrap();
        assert_eq!(variables.database_stats().unwrap().lookups, 2);
        assert_eq!(variables.expand(b"$(B)").unwrap(), b"literal");
        assert_eq!(variables.database_stats().unwrap().lookups, 4);
        variables
            .define(b"C", b"$(B)", Assignment::Simple, Origin::File)
            .unwrap();
        assert_eq!(variables.database_stats().unwrap().lookups, 7);
        assert!(
            variables
                .define(
                    b"D",
                    b"$(error stop-before-definition)",
                    Assignment::Simple,
                    Origin::File
                )
                .is_err()
        );
        assert_eq!(variables.database_stats().unwrap().lookups, 7);
        assert!(variables.undefine(b"B", Origin::File));
        let stats = variables.database_stats().unwrap();
        assert_eq!((stats.filled, stats.lookups), (2, 8));
        let _ = variables.get(b"missing");
        let _ = variables.database_definitions().count();
        let mut output = Vec::new();
        write_variables(&mut output, variables, stats).unwrap();
        assert_eq!(variables.database_stats(), Some(stats));
        assert!(variables.database_lookup(b"missing").is_none());
        assert_eq!(
            variables.database_stats().unwrap().lookups,
            stats.lookups + 1
        );
    }

    #[test]
    fn target_tables_have_distinct_identity_and_keep_visible_lookup_history() {
        let mut database = Database::default();
        database.enable_database_history().unwrap();
        database
            .variables
            .define_literal(b"GLOBAL", b"value", Origin::File)
            .unwrap();
        let parent = database.variables.with_target_scopes(&[]);
        assert_eq!(parent.expand(b"$(GLOBAL)").unwrap(), b"value");
        assert_eq!(parent.database_stats().unwrap().lookups, 1);
        assert_eq!(database.variables.database_stats().unwrap().lookups, 2);
        let mut child = parent.with_target_scopes(&[]);
        child
            .define_literal(b"@", b"child", Origin::Automatic)
            .unwrap();
        assert_eq!(child.database_stats().unwrap().filled, 1);
        assert_eq!(parent.database_stats().unwrap().filled, 0);
        assert_eq!(parent.database_stats().unwrap().lookups, 1);
        assert_eq!(child.expand(b"$@").unwrap(), b"child");
        assert_eq!(child.database_stats().unwrap().lookups, 2);
    }

    #[test]
    fn filename_preparation_and_file_enters_retain_actual_events_without_replay() {
        let mut database = Database::default();
        database.enable_database_history().unwrap();
        let names = database
            .rule_filenames(
                b"alpha alpha beta",
                &mut crate::diagnostic::ExpansionContext::pure(),
            )
            .unwrap();
        // GNU prepares the whole filename sequence before its caller consumes
        // a subset. Logical interning must not depend on iterator consumption.
        drop(names.take(1).collect::<Vec<_>>());
        let history = database.database_history().unwrap();
        history.with_string_cache(|cache| {
            assert_eq!(cache.hash_stats().filled, 2);
            assert_eq!(cache.hash_stats().lookups, 3);
        });
        database.file_records.insert(b"alpha");
        database.file_records.insert(b"alpha");
        database.file_records.insert(b"beta");
        let stats = database.database_history().unwrap().file_stats();
        assert_eq!((stats.filled, stats.lookups), (2, 3));
        assert!(
            database
                .database_history()
                .unwrap()
                .record_file_lookup(b"alpha")
        );
        assert_eq!(database.database_history().unwrap().file_stats().lookups, 4);
    }

    #[test]
    fn late_history_enable_is_rejected_instead_of_fabricating_prior_events() {
        let mut database = Database::default();
        database
            .variables
            .define_literal(b"EARLY", b"", Origin::File)
            .unwrap();
        assert!(database.enable_database_history().is_err());
        assert!(database.database_history().is_none());
    }
    #[test]
    fn builtin_record_origin_is_retained_until_an_actual_ordinary_enter() {
        let mut database = Database::default();
        database.enable_database_history().unwrap();
        database.initialize_suffixes(false).unwrap();
        assert!(
            database
                .database_history()
                .unwrap()
                .is_builtin_file(b".SUFFIXES")
        );
        assert_eq!(database.database_history().unwrap().file_stats().filled, 1);
        database.read(b".SUFFIXES:\n", b"Makefile").unwrap();
        assert!(
            !database
                .database_history()
                .unwrap()
                .is_builtin_file(b".SUFFIXES")
        );
        assert_eq!(database.database_history().unwrap().file_stats().filled, 1);
        database.file_records.insert_builtin(b".SUFFIXES");
        assert!(
            !database
                .database_history()
                .unwrap()
                .is_builtin_file(b".SUFFIXES")
        );
    }
    #[test]
    fn directory_table_uses_the_pinned_256_slot_name_order() {
        let history = History::default();
        for name in [b".".as_slice(), b"RCS", b"SCCS"] {
            history.record_directory(name);
        }
        assert_eq!(
            history.directory_names(),
            [b"RCS".to_vec(), b"SCCS".to_vec(), b".".to_vec()]
        );
    }
    #[test]
    fn vpath_observes_directories_during_reading_before_following_output() {
        use std::cell::RefCell;
        let events = RefCell::new(Vec::<Vec<u8>>::new());
        let mut output = |event: crate::diagnostic::Output<'_>| {
            if let crate::diagnostic::Output::Info(message) = event {
                events.borrow_mut().push(message.to_vec());
            }
        };
        let mut filesystem = |effect: crate::diagnostic::FilesystemEffect<'_>| {
            let crate::diagnostic::FilesystemEffect::DirectoryName(name) = effect else {
                panic!("unexpected filesystem operation")
            };
            events.borrow_mut().push(name.to_vec());
            Ok::<_, crate::diagnostic::FilesystemError>(name.to_vec())
        };
        let mut database = Database::default();
        database
            .read_with(
                b"vpath %.c left/:right/ .\n$(info MIDDLE)\nvpath %.h right/\n",
                b"Makefile",
                crate::diagnostic::ExpansionContext::new(None, &mut output)
                    .with_filesystem(&mut filesystem),
            )
            .unwrap();
        assert_eq!(
            *events.borrow(),
            [
                b"left".to_vec(),
                b"right".to_vec(),
                b"MIDDLE".to_vec(),
                b"right".to_vec()
            ]
        );
        let paths = database.database_vpaths().collect::<Vec<_>>();
        // The view borrows original search spelling; only emission trims it.
        assert_eq!(
            paths[0].directories,
            [b"left/".to_vec(), b"right/".to_vec()]
        );
    }

    #[test]
    fn empty_vpath_operand_deletes_but_dot_only_operand_preserves_prior_paths() {
        let mut database = Database::default();
        database
            .read(b"vpath %.c retained\nvpath %.c .::\n", b"Makefile")
            .unwrap();
        assert_eq!(database.database_vpaths().count(), 1);
        database.read(b"vpath %.c\n", b"Makefile").unwrap();
        assert_eq!(database.database_vpaths().count(), 0);
    }
    #[test]
    fn prerequisite_binding_looks_up_then_enters_only_missing_records() {
        let mut database = Database::default();
        database.enable_database_history().unwrap();
        database.file_records.enter_prerequisite(b"input");
        assert_eq!(database.database_history().unwrap().file_stats().lookups, 2);
        database.file_records.enter_prerequisite(b"input");
        assert_eq!(database.database_history().unwrap().file_stats().lookups, 3);
        database.file_records.insert_builtin(b".SUFFIXES");
        database.file_records.enter_prerequisite(b".SUFFIXES");
        assert!(
            database
                .database_history()
                .unwrap()
                .is_builtin_file(b".SUFFIXES")
        );
    }

    #[test]
    fn include_preparation_and_selected_read_have_distinct_intern_events() {
        let mut database = Database::default();
        database.enable_database_history().unwrap();
        let mut include = |name: &[u8]| {
            assert_eq!(name, b"child.mk");
            Ok::<_, crate::diagnostic::IncludeFailure>(crate::diagnostic::IncludedFile {
                name: b"search/child.mk".to_vec(),
                input: crate::reader::Input::Bytes(Vec::new()),
            })
        };
        database
            .read_with(
                b"include child.mk\n",
                b"Makefile",
                crate::diagnostic::ExpansionContext::pure().with_includes(&mut include),
            )
            .unwrap();
        assert!(database.file_records.contains(b"search/child.mk"));
        assert!(!database.file_records.contains(b"child.mk"));
        assert_eq!(database.database_history().unwrap().file_stats().lookups, 2);
        database
            .database_history()
            .unwrap()
            .with_string_cache(|cache| {
                assert_eq!(cache.hash_stats().filled, 2);
                assert_eq!(cache.hash_stats().lookups, 2);
            });
    }
    #[test]
    fn deferred_lookup_batches_commit_only_selected_immutable_observations() {
        let table = TableHistory::new(HashKind::Variable, 16);
        for name in [b"A".as_slice(), b"B", b"C"] {
            table.enter(name);
        }
        let before = table.stats();
        let mut rejected = LookupBatch::default();
        rejected.add(&table, b"A", 17).unwrap();
        drop(rejected);
        assert_eq!(table.stats(), before);
        let mut accepted = LookupBatch::default();
        accepted.add(&table, b"A", 17).unwrap();
        let mut more = LookupBatch::default();
        more.add(&table, b"missing", 9).unwrap();
        accepted.extend(more).unwrap();
        let mut ordinary = table.with(|state| state.clone());
        for _ in 0..17 {
            ordinary.lookup(b"A");
        }
        for _ in 0..9 {
            ordinary.lookup(b"missing");
        }
        accepted.commit();
        assert_eq!(table.stats(), ordinary.stats());
    }
    #[test]
    fn snapshot_batches_retain_probe_costs_across_tombstones_and_repeated_reads() {
        let table = TableHistory::new(HashKind::Variable, 16);
        let mut names = Vec::new();
        let home = crate::variable_order::fingerprint(b"collision") & 15;
        for index in 0..1000 {
            let name = format!("collision{index}").into_bytes();
            if crate::variable_order::fingerprint(&name) & 15 == home {
                names.push(name);
                if names.len() == 4 {
                    break;
                }
            }
        }
        assert_eq!(names.len(), 4);
        for name in &names[..3] {
            table.enter(name);
        }
        table.remove(&names[0]);
        let mut expected = table.with(|state| state.clone());
        let mut batch = LookupBatch::default();
        for (name, count) in [(&names[1], 3), (&names[3], 7), (&names[2], 5)] {
            batch.add(&table, name, count).unwrap();
            for _ in 0..count {
                expected.lookup(name);
            }
        }
        assert_eq!(batch.tables.len(), 1);
        batch.commit();
        assert_eq!(table.stats(), expected.stats());
    }

    #[test]
    fn tombstone_reuse_bounds_dead_keys_and_preserves_retained_snapshots() {
        let mut table = TableTelemetry::new(HashKind::Variable, 16).unwrap();
        table.insert(b"permanent");
        table.insert(b"reused");
        let snapshot = table.clone();
        for _ in 0..20000 {
            assert!(table.remove(b"reused"));
            assert!(table.insert(b"reused"));
        }
        assert_eq!(
            table.names().collect::<Vec<_>>(),
            snapshot.names().collect::<Vec<_>>()
        );
        assert!(table.slots.keys.len() < 4200);
        assert_eq!(snapshot.slots.dead_bytes, 0);
        assert!(snapshot.slots.keys.len() < 64);
        assert_eq!(table.stats().rehashes, 0);
        assert_eq!(table.stats().lookups, 40002);
    }

    #[test]
    fn rejected_snapshot_can_outlive_table_edits_without_changing_events() {
        let table = TableHistory::new(HashKind::Filename, 16);
        table.enter(b"before");
        let mut rejected = LookupBatch::default();
        rejected.add(&table, b"before", 99).unwrap();
        let old_layout = rejected.tables[0].layout.clone();
        for index in 0..40 {
            table.enter(format!("file{index}").as_bytes());
        }
        assert!(TableTelemetry::probe(HashKind::Filename, &old_layout, b"before").1);
        assert!(!TableTelemetry::probe(HashKind::Filename, &old_layout, b"file0").1);
        let before_discard = table.stats();
        let mut current = LookupBatch::default();
        current.add(&table, b"file0", 2).unwrap();
        assert!(current.extend(rejected).is_none());
        drop(current);
        assert_eq!(table.stats(), before_discard);
        assert!(table.lookup(b"file0"));
    }
    #[test]
    fn vpath_display_borrows_trimmed_names_without_changing_search_spelling() {
        let mut database = Database::default();
        database.read(b"vpath %.c d//\n", b"Makefile").unwrap();
        assert_eq!(
            database.vpath_directories(b"file.c").collect::<Vec<_>>(),
            [b"d//"]
        );
        let mut output = Vec::new();
        write_vpaths(&mut output, &database, &[b"general//".as_slice()]).unwrap();
        assert!(
            output
                .windows(b"vpath %.c d/\n".len())
                .any(|s| s == b"vpath %.c d/\n")
        );
        assert!(output.ends_with(b"# general/\n"));
        assert_eq!(
            database.vpath_directories(b"file.c").collect::<Vec<_>>(),
            [b"d//"]
        );
    }

    #[test]
    fn numeric_boundary_receives_integer_operands_for_every_percentage() {
        struct Numeric(std::cell::RefCell<Vec<(u64, u64, u8)>>);
        impl NumericFormatting for Numeric {
            fn write_percent(
                &self,
                output: &mut dyn Write,
                n: u64,
                d: u64,
                digits: u8,
            ) -> io::Result<()> {
                self.0.borrow_mut().push((n, d, digits));
                output.write_all(b"<numeric>")
            }
        }
        let numeric = Numeric(std::cell::RefCell::new(Vec::new()));
        let mut output = Vec::new();
        write_hash_stats_with_numeric(
            &mut output,
            HashStats {
                filled: 1,
                buckets: 16,
                lookups: 3,
                collisions: 2,
                ..HashStats::default()
            },
            &numeric,
        )
        .unwrap();
        assert_eq!(
            output,
            b"Load=1/16=<numeric>%, Rehash=0, Collisions=2/3=<numeric>%"
        );
        assert_eq!(*numeric.0.borrow(), [(1, 16, 0), (2, 3, 0)]);
        numeric.0.borrow_mut().clear();
        let mut database = Database::default();
        database
            .read(b"%.a: %.b\n%.x:: %.y\n", b"rules.mk")
            .unwrap();
        output.clear();
        write_implicit_rules_with_presentation(
            &mut output,
            &database,
            Presentation::english(&numeric),
        )
        .unwrap();
        assert_eq!(*numeric.0.borrow(), [(1, 2, 1)]);
        assert!(output.ends_with(b"# 2 implicit rules, 1 (<numeric>%) terminal."));
    }

    #[test]
    fn actual_registration_names_share_global_cache_across_setup_and_local_expansion() {
        use crate::diagnostic::ExpansionContext;
        use crate::native::{
            LoadOutcome, NativeCallback, NativeFunction, NativeHost, NativeLoadStatus,
            NativeRuntime,
        };
        use crate::variables::ExpandError;
        use std::rc::Rc;
        struct Callback;
        impl NativeCallback for Callback {
            fn invoke(
                &self,
                _: &[u8],
                _: &[Vec<u8>],
                host: &mut dyn NativeHost,
            ) -> Result<Vec<u8>, ExpandError> {
                host.registered_function_name(b"installed_later");
                Ok(Vec::new())
            }
        }
        struct Runtime;
        impl NativeRuntime for Runtime {
            fn lookup(&self, name: &[u8]) -> Option<Rc<NativeFunction>> {
                (name == b"install").then(|| {
                    Rc::new(NativeFunction {
                        minimum: 0,
                        maximum: None,
                        expand_arguments: true,
                        callback: Rc::new(Callback),
                    })
                })
            }
            fn load(
                &self,
                _: &[u8],
                _: bool,
                _: &Location,
                host: &mut dyn NativeHost,
            ) -> Result<LoadOutcome, ExpandError> {
                host.registered_function_name(b"installed_setup");
                host.registered_function_name(b"installed_setup");
                Ok(LoadOutcome {
                    name: b"module.so".to_vec(),
                    status: NativeLoadStatus::Loaded,
                })
            }
        }
        let mut database = Database::default();
        database.enable_database_history().unwrap();
        let runtime = Rc::new(Runtime);
        database
            .load_native_with(
                b"module.so",
                false,
                &Location {
                    file: b"Makefile".as_slice().into(),
                    line: 1,
                },
                &mut ExpansionContext::pure().with_native_runtime(runtime.clone()),
            )
            .unwrap();
        let history = database.database_history().unwrap().clone();
        history.with_string_cache(|cache| {
            assert_eq!(cache.hash_stats().lookups, 3); // module plus both actual registrations
            assert_eq!(cache.hash_stats().filled, 2);
        });
        database.read(b"all: LOCAL = local\n", b"Makefile").unwrap();
        let before = history.with_string_cache(|cache| cache.hash_stats());
        let context = database
            .variables
            .with_target_scopes(&[&database.target_scopes[b"all".as_slice()]]);
        assert_eq!(
            context
                .expand_with(
                    b"$(install )$(install )",
                    ExpansionContext::pure().with_native_runtime(runtime)
                )
                .unwrap(),
            b""
        );
        history.with_string_cache(|cache| {
            assert_eq!(cache.hash_stats().lookups, before.lookups + 2);
            assert_eq!(cache.hash_stats().filled, before.filled + 1);
        });
    }
}
