# Standard streams across a process restart

The effect-only remake recipe writes the new include with `file` and arms a
native extension without starting a child in the old image. The five cases
compare two pinned GNU runs with rmake. They verify the complete filesystem,
exit status and streams, including ordinary and named-stdin reexecution,
public buffered output, and public or private stream write failures.

GNU flushes stdout and stderr before exec and proceeds if a public flush
fails. A plugin's private buffered file is not flushed at this boundary.
Run `make test-native-restart-stdio` or `make test-native-restart-stdio-release`
for the corresponding binary profile.
