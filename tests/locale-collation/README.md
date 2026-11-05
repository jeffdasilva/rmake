# Original wildcard collation evidence

`models.json` contains 74 complete static models. No output field is normalized.
The selected candidate builds pass all 74; the preceding release passes 41 and
fails 33. `locale_switch.c` is an original public-libc/native fixture, with no
GNU header dependency.

Run the strict gate with:

```sh
python3 tools/locale_collation.py --candidate target/debug/rmake --report artifacts/locale-collation/development.json
```

The driver provisions its pinned private French locale automatically.
[LOCALE-DATA.md](LOCALE-DATA.md) and
[locale-data-history.json](locale-data-history.json) retain authenticated input,
license, cold/warm and complete generated-tree evidence.

The immutable compressed archives are indexed by SHA-256, raw SHA-256 and raw
byte count in their adjacent JSON manifests:

| Archive | Evidence |
| --- | --- |
| [history.json](history.json) | Original 65 hypotheses, six first prediction failures, raw inline-parser failures, three growth prediction failures, public libc equal-key matrix, observer errors, every intermediate candidate and original helper/model bytes. |
| [qualification.json](qualification.json) | Final 74 GNU2/preceding/development/optimized observations, both old183 regressions, coherent implementation/build/harness closure, frozen binary identities and checks. |
| [locale-data-history.json](locale-data-history.json) | Signed release/index/two-package provenance and independently checked cold/warm private locale generation. |

Wrong hypotheses and observer failures retain zero acceptance credit. An
earlier observer source was restored by reversing its recorded LOCPATH-only
patch, then verified byte-for-byte against the immutable first report's
original helper hash. That provenance is explicit in the history. No compiled fixture is tracked as an executable. Complete filesystem observations
retain its owned bytes and hashes; the original helper/input source is retained.
Third-party locale package payloads stay external, with authenticated identities.

These controls establish the selected behavior only; the
[scope and remaining work](../../docs/locale-collation.md) keep broader locale,
cache-history, resource, performance and M3/M6 acceptance open.
