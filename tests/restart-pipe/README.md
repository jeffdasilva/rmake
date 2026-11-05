# Inherited pipe across process replacement

The original fixture seeds one pipe and advertises descriptors 198 and 199.
It passes the independently captured device and inode to a native module.
After startup in each process image, the module checks both original numbers,
their pipe identity, and their close-on-exec flags. A generated include forces
a real restart. An ordered witness, exact streams and status, containment,
and the complete final fixture filesystem distinguish a successful rejoin
from silently falling back to a serial build.

`tools/restart_pipe.py --candidate PATH --report PATH` compares the fixture to
the verified GNU oracle twice. `--oracle-only` checks the original prediction.
The focused case ID is `restart-inherited-pipe-success`. The original model is
unchanged from discovery. It contains no GNU implementation, header, or test.

For a self-exec, the runtime verifies the incoming descriptors against the
live owned pipe and exposes only those numbers. Its ordinary spawned child
protocol continues using the private pair. A failed exec restores the
matching incoming descriptors to close-on-exec before temporary source cleanup.
Synchronous native replacement of an incoming number with an unrelated
descriptor cannot cause this adapter to expose it. Arbitrary concurrent
native descriptor rebinding while the host uses those numbers is outside the
descriptor ownership contract.

Both build modes passed the exact profile and the 61 mixed/normal jobserver
cases. The 14 lifecycle cases still pass, including the failed-exec
close-on-exec witness. A separate candidate safety control confirmed an
unrelated rebound descriptor stayed private. The complete local qualification
archive is identified by SHA-256
`d505723921e8c1567e3ed00b8bac63d5171ebd25dc7114842b0c430e7750fa12`.
