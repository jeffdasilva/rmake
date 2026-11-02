# Passive source ordering observations

All reports are observations on the pinned local profile, without general
performance or statistical acceptance claims. Profiles measure requests rather
than retained/peak memory. The [implementation note](../passive-source-order.md)
explains source selection, rejected models, controls and limitations.

## Selected and preceding elapsed controls

[Core A](passive-source-core-a.json), [core B](passive-source-core-b.json),
[arguments A](passive-source-runtime-a.json), [arguments B](passive-source-runtime-b.json).

## Correct elision experiment and reverse controls

[Elision core A](passive-source-elision-core-a.json),
[elision core B](passive-source-elision-core-b.json),
[elision arguments A](passive-source-elision-runtime-a.json),
[elision arguments B](passive-source-elision-runtime-b.json),
[elision versus copy C](passive-source-elision-prototype-runtime-c.json),
[elision versus copy D](passive-source-elision-prototype-runtime-d.json),
[copy versus elision E](passive-source-copy-control-runtime-e.json),
[copy versus elision F](passive-source-copy-control-runtime-f.json).

## Allocation observations

Each directory contains the report and its unmodified `trace.txt`.

| Input | Preceding 1 / 2 | Selected copy 1 / 2 | Unselected elision 1 / 2 |
| --- | --- | --- | --- |
| 0 definitions / 100 updates | [1](passive-source-alloc-preceding-0-1/report.json) / [2](passive-source-alloc-preceding-0-2/report.json) | [1](passive-source-alloc-copy-0-1/report.json) / [2](passive-source-alloc-copy-0-2/report.json) | [1](passive-source-alloc-elision-0-1/report.json) / [2](passive-source-alloc-elision-0-2/report.json) |
| 512 definitions / 100 updates | [1](passive-source-alloc-preceding-512-1/report.json) / [2](passive-source-alloc-preceding-512-2/report.json) | [1](passive-source-alloc-copy-512-1/report.json) / [2](passive-source-alloc-copy-512-2/report.json) | [1](passive-source-alloc-elision-512-1/report.json) / [2](passive-source-alloc-elision-512-2/report.json) |

## Excluded validation-overlap timing sequence

These reports earn zero performance credit and are not included in selected
timing conclusions. The [exclusion record](passive-source-overlap.json) preserves
the reason. Raw reports remain unchanged:
[core A](passive-source-overlap-core-a.json),
[core B](passive-source-overlap-core-b.json),
[arguments A](passive-source-overlap-runtime-a.json),
[arguments B](passive-source-overlap-runtime-b.json).
