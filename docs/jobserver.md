# Linux shared jobservers

Finite `-jN` pools now create a shared jobserver, using a FIFO by default or
`--jobserver-style=pipe`. Recursive rmake and GNU Make processes can join one
pool in either direction. MAKEFLAGS carries `-j` and the current authentication;
explicit child job options reset inherited mode, with GNU's warning. Unavailable
inherited servers fall back to one job. Unlimited `-j` remains unlimited.

Each process has its implicit slot, inherited from its parent command. Every
additional active target must acquire one byte from the shared server. A
non-cloneable permit stays with the complete recipe continuation, including
ignored failures and recursive commands. Normal completion, fatal expansion,
launch failures and unignored errors release held permits. Token return errors
are reported. The original acquired byte is returned, supporting external
clients whose tokens have distinct byte values.

The interpreter owns pool decisions and permits. Workers own prepared commands
and release their server references before reporting completion, so process exit
cannot race final FIFO cleanup against worker teardown. Owned FIFOs have private
permissions, are never created over existing files, and are unlinked only while
the path still identifies the created FIFO. Borrowed resources are not unlinked.
FIFO authentication rejects regular files and symlinks without writing to them.

Pipe protocol descriptors are close-on-exec except for recursive/forced commands.
The Linux adapter retains their ownership for each configured Command's lifetime
and makes only async-signal-safe descriptor operations in pre-exec callbacks.
Its separate nonblocking reader uses `/proc/self/fd`, preserving the blocking
flags shared with GNU clients. All unsafe operations remain inside the reviewed
Linux effect adapter; the application and semantic crates keep `unsafe_code =
forbid`. Pipe mode requires procfs. Very large requested pools can fail when token
initialization exceeds available pipe capacity; they cannot block initialization
indefinitely. External token availability is checked within 5 ms while local job
completions wake immediately; this is an optimization opportunity, not a claim
of scheduler performance parity.

`make test-jobserver` runs the original mixed-engine campaign and is part of CI.
Three repetitions cover FIFO and pipe transports, all four GNU/rmake parent/child
combinations, success and recursive failure with keep-going, borrowed pools and
token restoration. A lock-protected counter spans all six worker recipes, requires
three concurrent jobs, rejects oversubscription, and must return to zero. Separate
file/output anchors reject matching but incorrect engines. Exact bytes, statuses
and filesystem snapshots are compared with GNU baselines; temporary resources
must be gone. There are 61 recorded checks, including an additional candidate
sentinel-byte preservation probe. GNU controls use its normal `+` tokens: the
4.4.1 executable replaces distinctive borrowed token bytes with `+`, so that
additional preservation probe is a safety/protocol check, not a GNU-equivalence
claim. Reports retain binary/profile identities, fixture hashes and observations.
These are trusted fixtures with process containment, not a security sandbox.

Fifteen additional differential fixtures exercise client access, recursive and
ordinary pipe descriptor inheritance, recursive mode resets, unavailable-server
fallback and diagnostics. Six Linux adapter tests cover token ownership,
distinctive-byte restoration, resource lifetime/replacement, invalid inputs,
symlinks, regular files and recursive descriptor lifetime; a CLI test covers
transport/reset decoding. Three tooling tests protect independent campaign
anchors. The corpus now has 2,116 cases: 2,103 required and 13 pending.

Full GNU compatibility remains unfinished. Complete interrupt/signal forwarding,
interrupted-target cleanup and exit signaling, load limits and output
synchronization still need work. This slice does not claim signal-safe token
recovery on process termination. The tracked fatal-resumption GNU diagnostic
mismatch also remains pending; its FIFO leak is not reproduced.

Incoming pipe authentication also verifies that the reopened read and write
ends identify the same FIFO/pipe object before changing inherited FD flags or
using the server. Distinct valid pipes are not a valid pair; they produce the
unavailable-server fallback without consuming or redirecting tokens. The invalid
authentication regression now checks mismatched pairs and intact original pools.
