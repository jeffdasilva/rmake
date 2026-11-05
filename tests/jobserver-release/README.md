# Ordinary jobserver release

The twenty original cases compare exact status, stdout, stderr, filesystem
bytes and process containment with two GNU Make 4.4.1 runs. Each case starts
with one borrowed pipe token. The paired inputs use either a noncanonical
`Q` or the canonical `+`. The original controller observes queue capacity
with `FIONREAD` and holds the live children with file barriers. It reads the
returned token only after make exits.

The cases cover ordinary completion in both child orders, an ignored error
with a live continuation, a failed exec, a signal delivered to the recipe
child, a no-child command, and aborts before and after reservation.
Post-reservation configuration cases also cover a failure after a real
child has completed, while a separate live child still holds the implicit
slot.

`previous-sentinel.json` retains the exact original #167 borrowed FIFO
model. Its Makefile, child recipe, worker, and two-byte `QZ` seed still run
in the mixed-engine gate. The current normal assertion expects the two
`+` bytes observed in GNU Make, and checks the original output and
filesystem against both GNU repetitions.

Parent or group cancellation has separate fixtures and still checks
original-byte recovery for outstanding rmake leases. Those tests retain the
observed GNU capacity loss. These ordinary release cases add no safety
exceptions and make no broader claim about fatal jobserver cleanup.
