#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Replay discovery checks for the approved nested-short-option boundary."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import tempfile

import conformance as h
import oracle

ROOT = Path(__file__).resolve().parents[1]
NAME = 'nested-short-options-proposal'


def load():
    manifest = json.loads((ROOT / f'tests/research/{NAME}.json').read_text())
    packed = (ROOT / f'tests/research/{NAME}.json.gz').read_bytes()
    raw = gzip.decompress(packed)
    if (hashlib.sha256(packed).hexdigest() != manifest['sha256']
            or hashlib.sha256(raw).hexdigest() != manifest['uncompressed_sha256']):
        raise h.HarnessError('proposal evidence hash mismatch')
    return json.loads(raw)


def failures(case, observation, protected):
    errors = h.expectation_errors(case, observation)
    if observation.filesystem != protected:
        errors.append('unexpected or missing filesystem effects')
    return errors


def run(reference, candidate, destination):
    proof = load()
    def record(name):
        return json.loads(bytes.fromhex(proof['files'][name]['hex']))
    models = record('cells.json') + record('exact-cells.json') + record('boundary/cells.json')
    by_id = {r['id']: r for r in models}
    controls = record('control-cells.json')
    proposals = record('proposal-review-debug.json')['records']
    results = []
    with tempfile.TemporaryDirectory(prefix='rmake-nested-short-review-') as directory:
        root = Path(directory)
        work = root / 'observation'
        work.mkdir()
        for row in proposals + controls:
            proposal = '/' in row['id']
            name = row['id'].split('/')[0]
            model = by_id[name]
            prefix = model['path'].split('/artifacts/research/nested-flag-cursor/', 1)[1] + '/'
            source = root / 'source'
            source.mkdir(exist_ok=True)
            content = proof['files'][prefix + 'Makefile']
            (source / 'Makefile').write_bytes(bytes.fromhex(content['hex']))
            (source / 'Makefile').chmod(content['mode'])
            case = h.Case(source, row['id'], 'proposal.S004' if proposal else 'proposal.control',
                          'required', tuple(row['args']), 5,
                          row.get('environment', row.get('env', {})), row['expected'], 'separate')
            protected = dict(h.snapshot({'work': source}),
                             home={'type': 'directory', 'mode': 493},
                             tmp={'type': 'directory', 'mode': 493})
            observations = [h.execute(case, work, binary) for binary in
                            ((candidate, candidate) if proposal else (reference, reference, candidate))]
            for observed in observations:
                errors = failures(case, observed, protected)
                if errors:
                    raise h.HarnessError(f'{case.id}: {errors}')
            if any(h.differences(observations[0], o) for o in observations[1:]):
                raise h.HarnessError(f'{case.id}: repeated or paired observation mismatch')
            results.append(dict(id=row['id'],kind='safety-discovery-replay' if proposal else 'normal-control',
                                observations=[dict(h.observation_json(o),stdout_hex=o.stdout.hex(),
                                                   stderr_hex=o.stderr.hex()) for o in observations]))
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(dict(
        classification='Retained discovery checks; no additional conformance or exception credit.',
        totals=dict(proposal_cells=len(proposals),normal_controls=len(controls),fail=0,exact_conformance_credit=0,
                    approved_exception_credit=0),
        identities={str(p):h.sha256_file(p) for p in (reference,candidate)},records=results),indent=2)+'\n')
    print(f'Nested short boundary: {len(proposals)} repeated candidate-only cells and {len(controls)} normal controls pass; duplicate gate earns no additional credit.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, required=True)
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--report', type=Path, default=ROOT / 'artifacts/nested-short-proposal/report.json')
    args = parser.parse_args()
    oracle.verify(args.oracle.resolve())
    run(args.oracle.resolve(), args.candidate.resolve(), args.report)


if __name__ == '__main__':
    main()
