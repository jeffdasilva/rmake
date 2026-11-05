# Low descriptor limits

The four startup cases set the soft and hard descriptor limits to 4, 5, 6,
and 7. The three cancellation cases run a serial recipe that writes an owned
target before the controller signals make. At limit 6, the controller
independently verifies the live parent and child limits, the recipe's process
identity, the installed signal handler, and the absence of private pipe
descriptors. The roomy TERM case checks the wake pipe when resources allow it.

Each case compares exact streams, status, and the complete fixture filesystem
against the declared model and two runs of the verified GNU oracle. The
cancellation cases additionally require target deletion before releasing the
recipe and verify that its process identity is gone afterward. The protected
input and the absence of continuation files are part of the filesystem model.

Run `tools/resource_fd.py --candidate PATH --report PATH` or use
`--oracle-only`. The focused cancellation ID is `low6-term-live-recipe`.
These profiles cover startup and live cancellation. Recipe launch at limit 4
is tracked separately.
