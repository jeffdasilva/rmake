# Native floating-point environment profile

The standalone `tools/native_fenv.py` gate uses eighteen original C/public-ABI
models. They require exact stdout/stderr/status, ordered `seen` witnesses,
independent complete filesystem expectations, two reference observations and
bounded child/output cleanup. There are no output normalizers or added
compatibility exceptions. Each module is compiled once per case with `libm`,
then all engines receive byte-identical inputs at the same reset paths.

The models cover default state, upward/downward/toward-zero rounding, subsequent
callbacks, reading-to-recipe continuity, constructor/setup ownership, nested
`gmk_expand` and `gmk_eval`, explicit `fegetenv`/`fesetenv` restore, sticky
exception bits, and native finalization. All eighteen models qualify against GNU
Make 4.4.1 twice. The immutable preceding candidate
`46448db1f1a00ba24dd53f63fb3178d1c9c044fe5dbaeb7af34690b7b7865363`
passes the default-state control and fails seventeen comparisons. Eight focused
driver tests prevent shared wrong state, missing or reordered effects, changed
compiled inputs, ignored resource limits and successful exits on a failed gate.

The first research accidentally spelled zero-argument queries as `$(fp_get)`;
GNU correctly interpreted those as variable references. Fifteen initial models
therefore did not qualify. The original fixture, model generator, observer and
complete observations are retained. The corrected inputs use `$(fp_get )`; their
prewritten fenv expectations and ordered witnesses are unchanged. Complete
reference/candidate reports retain both research stages rather than rewriting
that mistake.

The ordinary C-fibre correction and the optional leading-C dispatcher require
separate candidate/source proofs. Reusing the same OS thread does not by itself
prove that `swapcontext` keeps its current floating-point environment. The
leading dispatcher has different stack ownership and is not credited by an
ordinary-path pass. These bounded profiles do not establish full M3/M6,
Guile/profile integration, all resource behavior, or performance parity. Existing
native and signal-mask fixtures remain unchanged.

Run the gate through its exact-case CLI, for example:

```sh
python3 -B tools/native_fenv.py --oracle /path/to/make --candidate /path/to/rmake --report /path/to/report.json
```

Use `--oracle-only` instead of `--candidate` to qualify the independent models;
`--case` accepts repeatable exact IDs. Any reference or candidate mismatch makes
the gate fail. The selected Linux/x86-64 C-locale profiles provide no ordinary
conformance-ledger credit.
