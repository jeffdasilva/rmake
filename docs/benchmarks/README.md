# Paired compatibility-slice measurements

These original raw reports retain all 101 alternating samples per executable,
output checks, workload definitions, binary identities and platform metadata for
the restart-directory, startup/profile-input and explicit-argument slices. Builds and acceptance
runs were not concurrent with timing. Host activity was not controlled.

The local executable paths identify the measured frozen binaries; they are not
portable commands. The reports do not establish statistical improvement, CPU
isolation, reproducible binary builds or general performance parity. GNU remains
faster on the standard workloads; rmake is faster on the recorded large-argv workloads. Interpret each report as one paired observation,
not as a comparison between host conditions in separate reports.
