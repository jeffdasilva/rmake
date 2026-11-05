# Borrowed jobserver read flags

A supervisor owns two pipes and retains duplicates of every end. It seeds the
advertised pipe with one byte and seeds a separate unadvertised pipe with a
different byte. One recursive recipe uses the implicit job slot and samples
the actual advertised child protocol. The supervisor samples its original
descriptors and aliases before launching make and after reaping it.

The roomy case places the advertised pipe at descriptors 16 and 17 with a
limit of 64. The compact case uses 3 and 4 with a limit of 10. A roomy control
passes both pipes without advertising either. The comparison requires the
original read description and its alias to become nonblocking only after
joining the advertised pipe. The actual recursive recipe's protocol read
must also be nonblocking. Write aliases and the separate pipe keep their
original flags. Both seeded bytes stay queued. The gate retains raw descriptor
numbers and flags and checks their physical identity and access mode against
the supervisor's input. It also checks literal streams, status, filesystem
effects, and process containment.

Run the development gate with `make test-jobserver-read-flags` and the
optimized gate with `make test-jobserver-read-flags-release`. The harness
requires a Linux glibc host that can supply a hard descriptor limit of 64.
