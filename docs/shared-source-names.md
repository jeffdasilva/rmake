# Immutable source-name sharing

The 10,000-rule recipe-free no-op workload issued about 375,000 observed glibc
allocation requests. Sampled stacks included declaration finishing, rule cloning
and build traversal. Many source locations copied the same filename bytes.

`Location.file` now owns an `Arc<[u8]>`. Each `read_at` invocation copies its source
name once; line locations and their clones share those immutable bytes. Each
location retains its own line number. A nested include or eval gets the correct
source spelling and span, and retained locations remain valid after the reader
or input buffer goes away. Diagnostic rendering allocates a mutable output only
when needed. This is not caching file contents, bindings, timestamps or effects;
there is no global interner and no cross-invocation invalidation policy.

The core crate's experimental Rust Location API now accepts an Arc-backed name;
callers can use `filename.as_slice().into()`. Makefile-visible behavior and raw
Linux byte preservation do not change. Existing location/include/eval/raw-byte
and complete conformance tests validate the change rather than tests of Arc's
standard-library reference counting.

## Allocation evidence

A development-only glibc interposer observes malloc/calloc/realloc/free after
helper initialization. It samples every 512th request and every request of at
least 64 KiB, resolves executable frames with addr2line and retains raw traces.
It excludes aligned allocator entry points. Requested byte totals are not live
memory or peak RSS, and the instrumentation provides no CPU-time estimate.
Sampling identifies possible callers rather than statistically unbiased costs.

On the original `large-noop` source with same-path argv, preceding main's normal
release issued **374,749 requests / 25,471,719 requested bytes**; the current
release issued **284,742 requests / 23,827,321 requested bytes**. Sharing alone removes about
24% of observed requests. Deferring the pure continuation bitmap until commands
need it removes another 10,000 requests from this workload, for **274,742 requests /
23,817,315 requested bytes** in the final release: about 27% fewer requests and
6.5% fewer requested bytes than preceding main. Empty recipes retain their
existing recursion/touch classification through an allocation-free scan; no
body or configuration effects are suppressed. Startup/loader activity,
reallocations and destructor order are explicitly outside any leak claim.
Elapsed-time evidence must use the ordinary, uninstrumented release and a separate
paired benchmark run; fewer requests alone do not establish GNU performance parity.

`make profile-allocations` builds a separate optimized executable with frame
pointers and debug information under `artifacts/profile/target`, then runs a
trusted original core workload. `PROFILE_WORKLOAD` selects `tiny-noop`,
`large-noop` (default), or `expansion`. A local C compiler, glibc and addr2line are
required for this development tool; the helper is never linked into the product.
The parser rejects incomplete/malformed traces rather than reporting zero
allocations. Reports include executable, helper-source and input digests, raw
addresses, resolved symbols and the coverage limits above.

See [the roadmap](implementation-plan.md) for the remaining general-performance
and safe persistent-cache gates. This optimization does not skip expansion,
filesystem observations or commands, and does not introduce a new dependency.
