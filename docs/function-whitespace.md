# Pure-function whitespace boundaries

Word and filename-list functions now recognize the six C-locale whitespace
bytes: space, tab, LF, CR, vertical tab and form feed. Non-ASCII bytes and other
control bytes remain literal data. This change is confined to function semantics;
Makefile directive and assignment tokenization retain their own rules.

Each function preserves its own output contract. Word-list transformations
normally join results with spaces; `wordlist` returns the original selected span,
and literal-pattern `patsubst` preserves untouched source separators. The final
variable in `let` receives the exact remainder from its first word, including
interior and trailing separators. `foreach` uses the first expanded name token
for its local binding and iterates over the expanded list's words.

`if`, `or` and `and` trim their raw condition arguments before expansion. A value
that expands to whitespace remains nonempty; it is neither trimmed away nor
reexpanded. Lazy branches keep their effect ordering, while `call` retains eager
argument evaluation before invoking a built-in function. Raw-byte and side-effect
tests distinguish these evaluation stages.

## Verification and limits

`make test-whitespace` compares fixed original cases against authored byte,
status and complete filesystem expectations and both executables. Cases cover
all six separators and non-whitespace controls, transformations, local binding
scope, selected/skipped effects and fatal ordering. `WHITESPACE_CASE` replays one
case. The curated corpus supplies additional combinations and known gaps.

GNU Make 4.4.1 can terminate with SIGSEGV for an insufficient `let` list, including
`$(let x y,  ,[$(x)][$(y)])`. The original minimized fixture records the oracle's
signal and remains pending: rmake safely binds missing values to empty strings.
No memory corruption or deliberate crash was added to imitate that behavior.
The positive generated campaign excludes this known crash family explicitly;
those inputs are not counted as matches. The six invalid numeric-argument
diagnostics found in this slice are covered by the subsequent
[word index implementation](word-indices.md), with their exact bytes retained
in required fixtures.

This implementation and its fixtures were independently written from original
oracle probes. No GNU implementation source or upstream tests were incorporated
for this slice. The common no-NUL expansion path uses safe slice membership
checks before locating a NUL, preserving the file-function consumer boundaries.

## Streaming word positions

`wordlist`, `let` and literal-pattern `patsubst` consume word positions from a
safe iterator instead of allocating a vector containing every position.
`wordlist` stops scanning positions at its requested ending word; `let` scans
only the positions needed for its bindings and preserves the final raw tail.
Literal `patsubst` still scans the full input while retaining every untouched
separator. Temporary position storage is constant size; expanded arguments,
bindings and output still have their existing allocations and resource limits.

All argument expansion, side effects and validation happen at the same stages
as before. Ending the position scan early does not suppress expansion in the
unselected portion of an argument. No observation or persistent cache is involved.
Curated regressions and the whitespace matrix cover all three consumers, exact
raw-byte boundaries and eager effects; larger original probes exercise long lists.

`make bench-compare BENCH_SUITE=word-spans` compares six fixed workloads with the
verified GNU oracle. Prefix, middle, full and out-of-range selections expose
different scanning costs, with `let` and literal `patsubst` measured separately.
These workloads support focused measurements, not a general performance claim.
