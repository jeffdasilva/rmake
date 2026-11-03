# Approved boundary for unstable nested short-option parsing

On 2026-11-02 the owner selected **Extend documented S004 errors** for the fifteen
normal-exit inputs below. The [explicit approval](../tests/safety/nested-short-normal-approval.json)
extends the inventory from 17 to 32 documented exceptions, with 128 candidate-only
cells. The original proposal archives retain their historical `approved=false`
labels unchanged. No observation here earns exact-conformance credit.

The finite historical `flag-payload-ordered-14` input is still reproducible:

```make
MAKEFLAGS = MAKEFLAGS=-s Y:=two Z:=three
$(info after X=[$(X)]/[$(value X)] Y=[$(Y)] Z=[$(Z)] flags=[$(MAKEFLAGS)] over=[$(MAKEOVERRIDES)])
all: ; @true
```

Two fresh pinned GNU observations set Y and skip Z. The preceding implementation sets both.
Twenty-four independently modeled neighbors vary one to six trailing bindings,
simple/recursive assignment and short `-s` versus long `--silent`. Fourteen
ordinary controls (all twelve long-option inputs and both one-tail inputs)
match GNU and the selected guard in both frozen builds. Nine original behavioral
hypotheses were rejected and retained unchanged. None is silently promoted.

Eight additional intervening-option probes preserve the difference between a
flag before the next binding and a binding before the next flag. Four more
normal controls pass in both builds. One new binding-before-flag input also
shows an unrequested keep-going difference between GNU executions. Its rejected
initial hypothesis remains unchanged.

Four short-option inputs differ between the two GNU executions in policy or
unrequested output, beyond ordinary PID/address nondeterminism:

| Original input ID | Observed GNU difference |
| --- | --- |
| `nested-cursor-tail-4-recursive-short` | One invocation publishes `krRs`; the other publishes `rRs`, although no keep-going option was requested. |
| `nested-cursor-tail-5-simple-short` | One invocation unexpectedly prints the version banner; the other does not. |
| `nested-cursor-tail-5-recursive-short` | One invocation publishes `drRs` and emits debug/trace output; the other publishes `rRs`. |
| `nested-boundary-simple-p1-short` | Keep-going is unrequested, but one invocation publishes `krRs` and the other publishes `rRs`. |

Every input above terminates normally in these observations. These are **not
crash/hang evidence**, and the existing safety inventory correctly rejects
normal-exit evidence in the original fault-only path. `ordered-14` now has the
owner's explicit normal-exit approval. Source inspection alone did not waive it.

GNU's `decode_env_switches` uses stack-local argument buffers; `decode_switches`
re-enters process-global getopt parsing. The pinned executable imports
`getopt_long@GLIBC_2.2.5`. Retained short-option scan state into a retired nested
buffer is a source-informed explanation, not a claim that every observed byte
or libc lifetime was reconstructed. The implementation must not reproduce
dangling-memory accesses or silently discard unexpected flags to claim a match.

## Approved behavior

S004 now covers fifteen recorded inputs: the historical exact
input plus both assignment forms with two through six trailing bindings and
the nested short `-s`, and four forms with a binding followed by another option
and binding. The new guard requires a following binding and additional words;
an intervening option before that binding remains allowed. It rejects at the nested reset before subsequent payload
effects, with exit status 2 and this located diagnostic:

```text
Makefile:1: *** rmake safety error [S004]: unsafe reentrant MAKEFLAGS argument decoding.  Stop.
```

For each frozen build, 60 candidate-only cells run twice: fifteen inputs across
normal, parallel, dry-run and target-output modes. All have the exact selected
error, empty stdout, unchanged protected inputs/filesystem, no reached bounds
and no leftover process. Eighteen normal control inputs retain exact GNU
semantics and are now ordinary required fixtures. The discovery replay remains
a duplicate gate with no additional acceptance credit. The safety manifest's
normal-exit path requires the exact approval, scope, input/invocation hashes and
four recorded policy instabilities; the original fault path remains strict.

`make test-nested-short-proposal` retains the strict live discovery gate. It executes
unsafe inputs only with rmake; GNU runs only the normal control inputs.
The same gate is present in development/optimized CI. Passing it cannot approve
the policy or earn exact-conformance credit.

The [manifest](../tests/research/nested-short-options-proposal.json) and
[compressed raw evidence](../tests/research/nested-short-options-proposal.json.gz)
retain every original input/model, both GNU observations, preceding candidate
observations, frozen prototype identities/source patch, all repeated proposal
checks and normal-control comparisons. No output was normalized and no observed
stream was copied into a new expected model.

The earlier broader prototype passed all 25,030 existing ordinary cases in
development and optimized builds, plus all 68 approved safety cells. The new
intervening-option controls exposed four additional ordinary regressions in that
prototype, retained unchanged. The tightened guard closes all four and passes
the complete 18-control gate in both builds. Its full acceptance and exact-head
CI remain required before any merge. The initial source, evidence and identity
remain in a [separate historical manifest](../tests/research/nested-short-options-proposal-initial.json)
and [archive](../tests/research/nested-short-options-proposal-initial.json.gz).

The owner approved this bounded extension. It does not authorize blanket
exceptions for arbitrary normal GNU inputs. Full conformance/runtime/CI gates
still apply before merging. M3/M6 and general performance parity remain open.
