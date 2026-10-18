# Word and wordlist numeric arguments

`word` and `wordlist` accept signed decimal integers within the signed 64-bit
range of the pinned Linux GNU Make 4.4.1 profile. An optional sign, leading
zeroes and surrounding C-locale whitespace are accepted. This is a separate
grammar from `intcmp`, whose integers can have arbitrary length.

`word` requires a positive index. `wordlist` requires a positive start and a
nonnegative end; negative zero is a valid end and selects no words. The start
is validated before the end. Numeric-prefix overflow takes priority over a
later invalid suffix. A malformed argument retains its original expanded bytes
in the diagnostic, including surrounding whitespace and non-UTF-8 bytes.
Parsed domain errors use GNU's canonical integer text where applicable.

All supplied arguments are expanded from left to right before validating
argument count or numbers. Their output and filesystem effects remain visible
even if validation fails. A nested expansion failure aborts first. Source
locations retain the normal innermost definition context for validation errors,
and the active caller context for the `error` function.

Checked magnitude accumulation handles both signed limits without overflowing,
and conversion to a word offset cannot wrap. Selected words retain their prior
whitespace-preservation rules. No filesystem observations, command results or
expansion effects are cached.

## Verification and provenance

Original curated fixtures cover accepted boundaries, invalid arguments, exact
byte diagnostics, contexts and effect ordering. `make test-word-index` adds an
independent fixed matrix with authored output, status and complete filesystem
expectations. `WORD_INDEX_CASE` replays one case. Both the pinned oracle and
rmake must satisfy the anchors before their observations are compared.

The pinned GNU Make 4.4.1 `src/function.c` functions `parse_numeric`, `func_word`
and `func_wordlist` were consulted as semantic references. Rust code and all
tests were independently authored; no implementation or upstream tests were
copied or translated. This slice covers these two functions' numeric and
argument-count behavior; other built-in argument-count gaps and broader GNU
compatibility work remain separate requirements.
