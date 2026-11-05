# File-size limits during Makefile writes

These fifteen original Linux profiles compare GNU Make 4.4.1 with rmake
under a hard and soft `RLIMIT_FSIZE`. They check exact status and streams,
complete filesystem state, and bounded process lifetime.

An independent inotify watcher records the identity and ordered lifecycle
of the temporary stdin source. The test uses that one kernel-observed
name for the diagnostic and filesystem checks. The models distinguish
buffered close failure followed by publication, a write failure before
publication, and a delivered SIGXFSZ that interrupts the copy before it
can publish the source. Retained files must have the exact capped bytes
and mode. The regular file-function controls separately exercise immediate
write, close, and append errors.
