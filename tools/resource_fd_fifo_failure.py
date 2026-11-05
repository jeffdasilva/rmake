#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""One historical FD4 FIFO cell, with exact byte and physical evidence."""
import argparse
import base64
import errno
import hashlib
import json
import os
from pathlib import Path
import tempfile

import conformance as h
import oracle

LAUNCHER = Path(__file__).resolve().parents[1] / 'tests/resource-fd-fifo/failure_launch.py'
ARGS = ('-rR', '--no-print-directory', '-j2', '--jobserver-style=fifo')
MAKEFILE = b'all:\n\t@$(if $(findstring --jobserver-auth=fifo:,$(MAKEFLAGS)),,$(error missing FIFO))/usr/bin/touch goal\n'
EXPECTED_STDERR = b'make: *** init jobserver pipe: Bad file descriptor.  Stop.\n'


def file_entry(content):
    return {'type': 'file', 'mode': 0o644, 'size': len(content),
            'sha256': hashlib.sha256(content).hexdigest(), 'content': base64.b64encode(content).decode()}


def inspect(observation):
    errors = []
    if (observation['stdout_hex'] != '' or observation['stderr_hex'] != EXPECTED_STDERR.hex() or
            observation['status'] != 2 or observation['bound'] or observation['leftovers']):
        errors.append('exact streams/status/bounds')
    witness = observation['witness']
    expected_witness = {
        'owner_pid': witness.get('owner_pid'), 'owner_uid': os.geteuid(), 'exact_child_reaped': True,
        'soft_limit': 4, 'hard_limit': 4, 'owner_wait_status': 2,
    }
    pid = expected_witness['owner_pid']
    if type(pid) is not int or pid <= 0:
        errors.append('independent owner child PID')
    name = 'GMfifo' + str(pid)
    path = witness.get('path')
    filesystem = dict(observation['initial_filesystem'])
    if not isinstance(path, dict):
        errors.append('owned FIFO absent')
        expected_witness['path'] = None
    else:
        identity = path.get('identity')
        if not isinstance(identity, list) or len(identity) != 2 or any(type(v) is not int or v <= 0 for v in identity):
            errors.append('invalid physical FIFO identity')
        expected_witness.update({
            'path': {'basename': name, 'identity': identity, 'fifo': True, 'mode': 0o600,
                     'uid': os.geteuid(), 'gid': observation['private_tmp_gid'], 'nlink': 1, 'size': 0},
            'writer_probe_errno': errno.ENXIO, 'reader_probe_identity': identity,
            'reader_probe_data_hex': '', 'post_probe_identity': identity,
        })
    filesystem['tmp/' + name] = {'type': 'fifo', 'mode': 0o600}
    if witness != expected_witness:
        errors.append('owner, identity, mode, endpoint or process witness')
    filesystem['work/observer.json'] = file_entry((json.dumps(expected_witness, sort_keys=True) + '\n').encode())
    if observation['filesystem'] != filesystem:
        errors.append('independent complete filesystem')
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--oracle', type=Path, default=oracle.PREFIX / 'bin/make')
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    reference = args.oracle.resolve(strict=True)
    candidate = args.candidate.resolve(strict=True)
    profile = oracle.verify(reference)
    identity = [h.executable_identity(reference, oracle=True), h.executable_identity(candidate)]
    records = []
    with tempfile.TemporaryDirectory(prefix='rmake-v5-fd4-fifo-') as directory:
        root = Path(directory)
        inputs, runtime = root / 'inputs', root / 'runtime'
        inputs.mkdir()
        runtime.mkdir()
        (inputs / 'Makefile').write_bytes(MAKEFILE)
        (inputs / 'launch.py').write_bytes(LAUNCHER.read_bytes())
        case = h.Case(inputs, 'owned-fifo-command-4', 'resource.fifo.failure', 'required', ARGS, 5, {}, {})
        for label, binary in [('gnu', reference), ('gnu', reference), ('candidate', candidate)]:
            h.reset_workspace(case, runtime, binary)
            (runtime/'tmp').chmod(0o700)
            roots = {part: runtime / part for part in ('work', 'home', 'tmp')}
            initial = h.snapshot(roots)
            command = ['/usr/bin/python3', 'launch.py', str(runtime/'bin/make'), *ARGS]
            stdout, stderr, status, bound, leftover = h.capture(command, runtime/'work',
                                                                 h.clean_environment(runtime), 5)
            witness_path = runtime/'work/observer.json'
            witness = json.loads(witness_path.read_bytes()) if witness_path.is_file() else {}
            record = {'engine': label, 'argv': command, 'stdout_hex': stdout.hex(), 'stderr_hex': stderr.hex(),
                      'status': status, 'bound': bound, 'leftovers': leftover, 'initial_filesystem': initial,
                      'filesystem': h.snapshot(roots), 'witness': witness,
                      'private_tmp_gid': (runtime/'tmp').stat().st_gid}
            record['errors'] = inspect(record)
            records.append(record)
            print(label, status, repr(stderr), witness.get('path'), record['errors'], flush=True)
            if bound or leftover or status < 0:
                break
    passed = len(records) == 3 and not any(r['errors'] for r in records)
    report = {'oracle_profile': profile['profile_id'], 'identities': identity, 'records': records,
              'source_hashes': {path.name: h.sha256_file(path) for path in (Path(__file__), LAUNCHER)},
              'functional_affinity': sorted(os.sched_getaffinity(0)), 'passed': passed}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    with args.report.open('w') as stream:
        json.dump(report, stream, indent=2)
        stream.write('\n')
    return int(not passed)


if __name__ == '__main__':
    raise SystemExit(main())
