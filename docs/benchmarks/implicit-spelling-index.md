# Borrowed literal implicit-prerequisite measurements

Eight output-checked elapsed reports use 101 alternating samples per engine.
Each compares the selected frozen release, preceding shuffle release and pinned
GNU Make. A uses the GNU Make driver; B uses frozen rmake. All local validation
campaigns had exited before elapsed measurement. Host activity is not controlled;
these are workload observations, not statistical or general performance proof.

| Input | GNU A / B ms | Selected A / B ms | Preceding A / B ms |
| --- | --- | --- | --- |
| Tiny no-op | 0.638 / 0.641 | 0.742 / 0.725 | 0.750 / 0.729 |
| 10,000 explicit targets | 11.226 / 11.170 | 30.320 / 30.207 | 29.926 / 29.957 |
| Expansion | 2.782 / 2.817 | 5.073 / 5.152 | 5.087 / 5.113 |
| 64 rejected implicit patterns | 1.200 / 1.256 | 2.482 / 2.519 | 2.612 / 2.622 |
| 256 rejected implicit patterns | 3.377 / 3.338 | 11.470 / 11.381 | 12.080 / 12.043 |

Rejected-search medians are roughly 4–5.5% lower. Other controls are mixed,
including 0.8–1.3% higher large-noop medians; GNU remains faster overall.
Raw samples, exact output checks, identities and source hashes remain in:

- [Core A](implicit-spelling-core-a.json) / [B](implicit-spelling-core-b.json).
- [Implicit search A](implicit-spelling-implicit-search-a.json) / [B](implicit-spelling-implicit-search-b.json).
- [Rejected search A](implicit-spelling-implicit-rejections-a.json) / [B](implicit-spelling-implicit-rejections-b.json).
- [Target contexts A](implicit-spelling-target-contexts-a.json) / [B](implicit-spelling-target-contexts-b.json).

Twenty glibc request profiles repeat each engine/input twice. For rejected
searches, counts fall from 44,527 to 32,239 (64 patterns) and 182,065 to 122,929
(256 patterns), about 28–32%. Corresponding requested bytes fall from 4,244,179
to 4,147,184 and 16,716,883 to 16,145,264. Request counts are unchanged on the
three other inputs. Requests are not live/peak memory, complete allocator coverage
or CPU time; sampled stacks are hints. Each directory contains its report and
unmodified trace:

| Input | Preceding 1 / 2 | Selected 1 / 2 |
| --- | --- | --- |
| Large-noop | [1](implicit-spelling-alloc-preceding-large-noop-1/report.json) / [2](implicit-spelling-alloc-preceding-large-noop-2/report.json) | [1](implicit-spelling-alloc-selected-large-noop-1/report.json) / [2](implicit-spelling-alloc-selected-large-noop-2/report.json) |
| Expansion | [1](implicit-spelling-alloc-preceding-expansion-1/report.json) / [2](implicit-spelling-alloc-preceding-expansion-2/report.json) | [1](implicit-spelling-alloc-selected-expansion-1/report.json) / [2](implicit-spelling-alloc-selected-expansion-2/report.json) |
| Explicit target context | [1](implicit-spelling-alloc-preceding-target-context-explicit-1/report.json) / [2](implicit-spelling-alloc-preceding-target-context-explicit-2/report.json) | [1](implicit-spelling-alloc-selected-target-context-explicit-1/report.json) / [2](implicit-spelling-alloc-selected-target-context-explicit-2/report.json) |
| 64 rejected patterns | [1](implicit-spelling-alloc-preceding-implicit-rejected-64-1/report.json) / [2](implicit-spelling-alloc-preceding-implicit-rejected-64-2/report.json) | [1](implicit-spelling-alloc-selected-implicit-rejected-64-1/report.json) / [2](implicit-spelling-alloc-selected-implicit-rejected-64-2/report.json) |
| 256 rejected patterns | [1](implicit-spelling-alloc-preceding-implicit-rejected-256-1/report.json) / [2](implicit-spelling-alloc-preceding-implicit-rejected-256-2/report.json) | [1](implicit-spelling-alloc-selected-implicit-rejected-256-1/report.json) / [2](implicit-spelling-alloc-selected-implicit-rejected-256-2/report.json) |

The earlier unselected sharing experiment's eight elapsed reports and twelve
request profiles remain unchanged in its [archive](../../tests/research/prerequisite-sharing-experiments.json.gz)
and [manifest](../../tests/research/prerequisite-sharing-experiments.json).
It made fewer requests on large-noop but had 8–12% higher rejected-search medians
in two batches. Those observations earn no performance-win or conformance credit.
