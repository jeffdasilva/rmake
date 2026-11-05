# Database history cost and file key selection

The selected implementation keeps existing boxed filename keys when the file
record table grows or removes tombstones. Moving the keys preserves the GNU
logical history. Against otherwise identical source that copied them, the large
no-op control used 240,410 allocation requests instead of 254,814 and requested
110,866 fewer bytes. Both repeats produced the same allocation totals. The
paired median elapsed change was -0.84% for the large no-op and -2.32% for the
expansion control; the tiny no-op was effectively unchanged.

Sharing each timestamp key through `Arc` or `Rc` removed another 10,002 requests
on the large no-op. The corresponding elapsed changes against the selected
boxed-key build were +1.89% and +1.30% on that workload and +1.60% and +1.09% on
the expansion workload. Neither sharing alternative is selected. Both passed
41 database, six numeric, and six named-file lifecycle comparisons on their
frozen builds. The rehash-only comparison uses the same tests.

The new database compatibility work records logical history even when a
Makefile requests `-p` late. The complete selected build has these measured
default-path costs against the preceding collation build:

| Workload | Paired elapsed change | Selected median | GNU median |
| --- | ---: | ---: | ---: |
| Tiny no-op | +3.33% | 1.750 ms | 1.584 ms |
| Large no-op | +23.53% | 37.953 ms | 12.077 ms |
| Expansion | +5.62% | 6.192 ms | 3.632 ms |
| 5,000 startup arguments | +4.54% | 8.881 ms | 26.543 ms |

The six runtime argument controls were +2.02% to +4.30% against the preceding
build. The 5,000-argument case remains about 2.99 times faster than GNU in this
measurement, while the large no-op and expansion controls remain slower. This
is a measured compatibility cost and a target for further work.

The selected binary digest is
`a4143603e637d7b0562a5b0c2b82dc390f90f0211f97416de8b9ad6585adebb6`.
The six timing reports, 30 allocation reports, and functional comparisons
are retained in the local validation archive with SHA-256
`562bb3c5d01c88bcf93853e165731df85316b8b8fd71be4b66775a947a756d42`.
All samples ran serially on shared CPU 23 with the project's other work
paused. Core and
startup cases used 101 alternating samples per engine; runtime cases used 301.
These measurements do not establish exclusive CPU access, frequency, peak RSS,
IO or statistical confidence. Allocation traffic is not live memory. General
GNU performance parity and persistent caching remain open.
