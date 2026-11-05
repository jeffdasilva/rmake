# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Independent finite input replay for exact GNU4.4.1 Linux database profiles.

No production rmake modules/formatters are imported. The table-driven 32-bit
mix/final operations follow Bob Jenkins's public-domain lookup3 specification:
https://burtleburtle.net/bob/c/lookup3.c . Zero-terminated four-byte framing and
observable table/cache contracts were consulted in the pinned primary reference;
no GNU implementation/header/test contents are copied here.
"""
from __future__ import annotations

from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path
import hashlib
import json
import os
import re
import stat
import shutil
import subprocess
from typing import Any

import conformance as h

MASK32 = (1 << 32) - 1
CACHE_CAPACITY = 8162
CACHE_BUCKETS = 8192
MIX = ((0, 2, 1, 4), (1, 0, 2, 6), (2, 1, 0, 8),
       (0, 2, 1, 16), (1, 0, 2, 19), (2, 1, 0, 4))
FINAL = ((2, 1, 14), (0, 2, 11), (1, 0, 25), (2, 1, 16),
         (0, 2, 4), (1, 0, 14), (2, 1, 24))


def rotate(value: int, amount: int) -> int:
    return ((value << amount) | (value >> (32 - amount))) & MASK32


@lru_cache(maxsize=4096)
def string_hash(key: bytes) -> int:
    """Model the supported case-sensitive NUL key framing mathematically."""
    key = key.split(b'\0', 1)[0]
    words = [int.from_bytes(key[start:start + 4], 'little')
             for start in range(0, len(key), 4)]
    complete = len(key) // 4
    # A final zero/partial word terminates framing, even at a 12-byte boundary.
    if len(key) % 4 == 0:
        words.append(0)
    state = [0xDEADBEEF] * 3
    for index, word in enumerate(words):
        state[index % 3] = (state[index % 3] + word) & MASK32
        if index < complete and index % 3 == 2:
            for target, source, third, shift in MIX:
                state[target] = ((state[target] - state[source]) & MASK32) ^ rotate(state[source], shift)
                state[source] = (state[source] + state[third]) & MASK32
    for target, source, shift in FINAL:
        state[target] = ((state[target] ^ state[source]) - rotate(state[source], shift)) & MASK32
    return (state[2] + 4 * complete) & MASK32


@dataclass(frozen=True)
class CacheState:
    keys: tuple[bytes, ...]
    strings: int
    storage: int
    lookups: int
    collisions: int


class StringCache:
    """Bounded single-buffer model, exact for the selected input families.

    All41 original profiles use at most2512 bytes in one8162-byte buffer.
    Reject inputs outside the declared buffer/table scope; never fit a delta
    to a golden number or read actual diagnostic bytes to choose a result.
    """
    def __init__(self):
        self.slots: dict[int, bytes] = {}
        self.keys: list[bytes] = []
        self.storage = 0
        self.lookups = 0
        self.collisions = 0

    def add(self, span: bytes, nominal_length: int | None = None) -> bool:
        length = len(span) if nominal_length is None else nominal_length
        if length < 0 or length > 65534 or length != len(span):
            raise h.HarnessError('cache replay needs exact supported nominal span bytes')
        key = span.split(b'\0', 1)[0]
        self.lookups += 1
        if self.lookups > 1000000:
            raise h.HarnessError('cache replay lookup bound exceeded')
        for distance in range(CACHE_BUCKETS):
            slot = (string_hash(key) + distance) & (CACHE_BUCKETS - 1)
            if slot not in self.slots:
                allocated = length + 1
                storage = self.storage + allocated
                count = len(self.keys) + 1
                if (storage >= CACHE_CAPACITY
                    or (count > 20 and CACHE_CAPACITY - storage < storage // count + 1)
                    or count >= CACHE_BUCKETS * 15 // 16):
                    raise h.HarnessError('cache replay exceeds selected one-buffer/table scope')
                self.slots[slot] = key
                self.keys.append(key)
                self.storage = storage
                return True
            if self.slots[slot] == key:
                return False
            self.collisions += 1
        raise h.HarnessError('cache replay table has no vacant slot')

    def state(self) -> CacheState:
        return CacheState(tuple(self.keys), len(self.keys), self.storage, self.lookups, self.collisions)

    def render(self, witness: 'PublicWitness', rounding: str = 'nearest') -> bytes:
        state = self.state()
        if not state.strings:
            return b'\n# No strcache buffers\n\n'
        average = state.storage // state.strings
        load = witness.numeric(state.strings, CACHE_BUCKETS, rounding, 'rounded')
        collisions = witness.numeric(state.collisions, state.lookups, rounding, 'rounded')
        hits = witness.numeric(state.lookups - state.strings, state.lookups, rounding, 'truncated')
        return (f'\n# strcache buffers: 1 (0) / strings = {state.strings} / storage = {state.storage} B / avg = {average} B\n'
                f'# current buf: size = {CACHE_CAPACITY} B / used = {state.storage} B / count = {state.strings} / avg = {average} B\n'
                f'\n# strcache performance: lookups = {state.lookups} / hit rate = {hits}%\n'
                '# hash-table stats:\n'
                f'# Load={state.strings}/{CACHE_BUCKETS}={load}%, Rehash=0, Collisions={state.collisions}/{state.lookups}={collisions}%\n').encode()


class PublicWitness:
    """Original public libc observer; subprocess boundaries bound all effects."""
    def __init__(self, binary: Path, source_sha256: str, compilation: dict[str, Any]):
        self.binary = binary
        self.source_sha256 = source_sha256
        self.compilation = compilation
        self._numeric: dict[tuple[int, int, str, str], str] = {}

    @classmethod
    def compile(cls, directory: Path) -> 'PublicWitness':
        source = Path(__file__).parent / 'fixtures/database_inputs.c'
        compiler = shutil.which('cc', path='/usr/bin:/bin')
        if compiler is None:
            raise h.HarnessError('exact database public input witness requires cc')
        directory = directory.resolve()
        directory.mkdir(parents=True, exist_ok=True)
        for name in ('home', 'tmp'):
            (directory / name).mkdir()
        binary = directory / 'database-input-witness'
        command = [compiler, '-std=c11', '-Wall', '-Wextra', '-Werror', '-g0', '-O2',
                   '-frounding-math', str(source), '-lm', '-o', str(binary)]
        out, err, status, limit, leftovers = h.capture(command, directory, h.clean_environment(directory), 30)
        if status or out or err or limit or leftovers:
            raise h.HarnessError('public input witness compile failed: ' + repr((status, err, limit, leftovers)))
        record = {'command': command, 'source_sha256': h.sha256_file(source),
                  'binary_sha256': h.sha256_file(binary), 'compiler_sha256': h.sha256_file(Path(compiler))}
        return cls(binary, record['source_sha256'], record)

    def run(self, arguments: list[str | bytes]) -> bytes:
        command = [os.fsencode(self.binary), *[os.fsencode(value) for value in arguments]]
        try:
            result = subprocess.run(command, env=h.clean_environment(self.binary.parent),
                                    stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                    stderr=subprocess.PIPE, timeout=10)
        except subprocess.TimeoutExpired as error:
            raise h.HarnessError('public input witness exceeded its time bound') from error
        if result.returncode or result.stderr or len(result.stdout) > 4 * 1024 * 1024:
            raise h.HarnessError('public input witness failed: ' + repr((result.returncode, result.stderr)))
        return result.stdout

    def numeric(self, numerator: int, denominator: int, mode: str, style: str) -> str:
        key = (numerator, denominator, mode, style)
        if (not 0 <= numerator <= 1000000000 or not 1 <= denominator <= 1000000000
            or mode not in ('nearest', 'downward', 'upward', 'towardzero')
            or style not in ('rounded', 'truncated')):
            raise h.HarnessError('invalid public numeric witness input')
        if key not in self._numeric:
            text = self.run(['numeric', str(numerator), str(denominator), mode, style])
            if not text.endswith(b'\n') or not text[:-1].isdigit():
                raise h.HarnessError('malformed public numeric witness output')
            self._numeric[key] = text[:-1].decode('ascii')
        return self._numeric[key]

    def directories(self, paths: list[bytes]) -> list[dict[str, Any]]:
        raw = self.run(['directories', *paths])
        result: list[dict[str, Any]] = []
        current: dict[str, Any] | None = None
        for line in raw.splitlines():
            fields = line.split(b'\t')
            if len(fields) < 2 or not fields[1].isdigit():
                raise h.HarnessError('malformed public directory witness index')
            index = int(fields[1])
            if index >= len(paths):
                raise h.HarnessError('public directory witness index exceeds declared paths')
            if fields[0] == b'D':
                if current is not None or index != len(result) or len(fields) not in (3, 5):
                    raise h.HarnessError('malformed public directory witness beginning')
                error = int(fields[2])
                current = {'path_hex': paths[index].hex(), 'errno': error, 'entries': []}
                if error:
                    if len(fields) != 3:
                        raise h.HarnessError('failed directory witness has identity fields')
                    result.append(current); current = None
                else:
                    if len(fields) != 5:
                        raise h.HarnessError('directory witness lacks open descriptor identity')
                    current.update(device=int(fields[3]), inode=int(fields[4]))
            elif fields[0] == b'E':
                if current is None or index != len(result) or len(fields) != 5:
                    raise h.HarnessError('unowned public directory entry')
                name = bytes.fromhex(fields[4].decode())
                if not name or b'\0' in name or b'/' in name:
                    raise h.HarnessError('invalid public directory entry name')
                current['entries'].append({'inode': int(fields[2]), 'type': int(fields[3]), 'name_hex': name.hex()})
            elif fields[0] == b'Z':
                if current is None or index != len(result) or len(fields) != 2:
                    raise h.HarnessError('malformed public directory witness end')
                result.append(current); current = None
            else:
                raise h.HarnessError('unknown public directory witness record')
        if current is not None or len(result) != len(paths):
            raise h.HarnessError('incomplete public directory input capture')
        return result


@dataclass(frozen=True)
class BuildConfiguration:
    profile_id: str
    configure_arguments: tuple[str, ...]
    includedir: bytes
    default_paths: tuple[bytes, ...]
    directory_facts: tuple[tuple[bytes, bool], ...]

    def evidence(self) -> dict[str, Any]:
        return {'verified_profile_id': self.profile_id, 'configure_arguments': list(self.configure_arguments),
                'compiled_includedir_hex': self.includedir.hex(),
                'active_default_paths_hex': [path.hex() for path in self.default_paths],
                'directory_existence': [{'path_hex': path.hex(), 'is_directory': exists}
                                        for path, exists in self.directory_facts]}


def configuration(verified_profile: dict[str, Any]) -> BuildConfiguration:
    """Use verified provisioning arguments and independently stat default paths."""
    arguments = tuple(verified_profile['build']['configure_args'])
    if not arguments or not all(isinstance(argument, str) for argument in arguments):
        raise h.HarnessError('verified profile lacks configure arguments')
    prefixes = [argument.partition('=')[2] for argument in arguments if argument.startswith('--prefix=')]
    explicit = [argument.partition('=')[2] for argument in arguments if argument.startswith('--includedir=')]
    if len(prefixes) != 1 or len(explicit) > 1:
        raise h.HarnessError('database profile needs one declared installation prefix')
    prefix = prefixes[0]
    chosen = explicit[0] if explicit else prefix.rstrip('/') + '/include'
    if (not prefix.startswith('/') or not chosen.startswith('/')
        or any(byte in chosen for byte in ('\0', '\n', '$'))):
        raise h.HarnessError('database profile requires a literal absolute configured include directory')
    includedir = os.fsencode(chosen.rstrip('/') or '/')
    ordered = (includedir, b'/usr/gnu/include', b'/usr/local/include', b'/usr/include')
    facts = []
    for path in ordered:
        try:
            exists = stat.S_ISDIR(os.stat(path).st_mode)
        except (FileNotFoundError, NotADirectoryError):
            exists = False
        facts.append((path, exists))
    paths = tuple(path for path, exists in facts if exists)
    if paths != (includedir, b'/usr/local/include', b'/usr/include'):
        # Adding or omitting a directory changes make's variable-table events
        # too. These finite plans qualify spelling and order, not that other
        # default-directory inventory.
        raise h.HarnessError('unqualified default include-directory inventory')
    # The trusted standalone provisioning probe corroborates configuration.
    # It is not a source of case output, cache counters, or directory order.
    observed = os.fsencode(verified_profile['observed']['variables']['.INCLUDE_DIRS'])
    if b' '.join(paths) != observed:
        raise h.HarnessError('compiled default include inputs disagree with verified provisioning profile')
    return BuildConfiguration(verified_profile['profile_id'], arguments, includedir, paths, tuple(facts))


@dataclass(frozen=True)
class PublicInputs:
    config: BuildConfiguration
    before: dict[str, dict[str, Any]]
    after: dict[str, dict[str, Any]]
    witness: PublicWitness
    plan: dict[str, Any]

    def evidence(self) -> dict[str, Any]:
        return {'configuration': self.config.evidence(), 'public_directory_order_before': self.before,
                'public_directory_order_after': self.after, 'witness_compilation': self.witness.compilation,
                'trusted_trace_sha256': self.plan['trusted_trace_sha256']}


class DirectoryCache:
    def __init__(self, snapshot: dict[str, Any]):
        self.snapshot = snapshot
        self.entries = tuple(bytes.fromhex(entry['name_hex']) for entry in snapshot['entries'] if entry['inode'] != 0)
        self.cursor = 0
        self.complete = snapshot['errno'] != 0
        # True is positive membership; False is a cached impossibility.
        self.members: dict[bytes, bool] = {}

    def probe(self, wanted: bytes | None, strings: StringCache) -> bool:
        if wanted == b'':
            return self.snapshot['errno'] == 0
        if wanted is not None and wanted in self.members:
            return self.members[wanted]
        if self.complete:
            return False
        for index in range(self.cursor, len(self.entries)):
            name = self.entries[index]
            self.cursor = index + 1
            strings.add(name)
            self.members[name] = True
            if name == wanted:
                # A hit at the last available entry has not yet observed EOF.
                return True
        self.complete = True
        return False

    def impossible(self, name: bytes) -> None:
        self.members[name.split(b'\0', 1)[0]] = False

    def counts(self) -> tuple[int, int, bool]:
        files = sum(self.members.values())
        return files, len(self.members) - files, self.complete


def replay(dump: dict[str, Any], inputs: PublicInputs, *,
           strings: StringCache | None = None) -> tuple[StringCache, dict[str, DirectoryCache]]:
    if strings is None:
        strings = StringCache()
    epochs: dict[str, str] = {}
    directories: dict[str, DirectoryCache] = {}
    for step in dump['steps']:
        kind = step[0]
        if kind == 'add' and len(step) == 2:
            strings.add(bytes.fromhex(step[1]))
        elif kind == 'defaults' and len(step) == 1:
            for path in inputs.config.default_paths:
                strings.add(path)
        elif kind == 'open' and len(step) == 5:
            _, owner, alias, epoch, exists = step
            if epoch not in ('before', 'after'):
                raise h.HarnessError('unqualified public directory input epoch')
            snapshots = inputs.before if epoch == 'before' else inputs.after
            if type(exists) is not bool or (snapshots[alias]['errno'] == 0) != exists:
                raise h.HarnessError('public directory input changed a declared open outcome')
            directories[owner] = DirectoryCache(snapshots[alias])
            epochs[owner] = epoch
        elif kind == 'alias' and len(step) == 3:
            _, owner, alias = step
            snapshots = inputs.before if epochs[owner] == 'before' else inputs.after
            owner_dir, alias_dir = directories[owner].snapshot, snapshots[alias]
            fields = ('errno', 'device', 'inode')
            if any(owner_dir.get(field) != alias_dir.get(field) for field in fields):
                raise h.HarnessError('declared directory aliases do not name the same physical cache')
        elif kind == 'probe' and len(step) == 4:
            _, owner, wanted, outcome = step
            if type(outcome) is not bool:
                raise h.HarnessError('directory probe needs an independently qualified membership outcome')
            name = None if wanted is None else bytes.fromhex(wanted)
            if directories[owner].probe(name, strings) != outcome:
                raise h.HarnessError('public directory input changed a declared probe outcome')
        elif kind == 'impossible' and len(step) == 3:
            _, owner, name = step
            directories[owner].impossible(bytes.fromhex(name))
        else:
            raise h.HarnessError('unknown or malformed database input replay action: ' + repr(step))
    return strings, directories


DUMP_OPEN = re.compile(rb'(?m)^# Make data base, printed on @CTIME_OPEN(?:_[0-9]+)?@\n')
DUMP_CLOSE = re.compile(rb'(?m)^# Finished Make data base on @CTIME_CLOSE(?:_[0-9]+)?@\n')
DIR_ROW = re.compile(rb'(?m)^(# ([^\n]*) \(device @DEVICE_[0-9]+@, inode @INODE_[0-9]+@\): )'
                     rb'(?:No|[0-9]+) files, (?:no|[0-9]+) impossibilities(?: so far)?\.\n')
DIR_TOTAL = re.compile(rb'(?m)^# (?:No|[0-9]+) files, (?:no|[0-9]+) impossibilities in ([0-9]+) directories\.\n')
CACHE_START = re.compile(rb'(?m)^# (?:strcache buffers: |No strcache buffers\n)')


def render_directory_fields(body: bytes, dump: dict[str, Any], directories: dict[str, DirectoryCache]) -> bytes:
    start = body.find(b'\n# Directories\n')
    if start < 0:
        raise h.HarnessError('database model lacks its original directory section')
    total = DIR_TOTAL.search(body, start)
    if total is None:
        raise h.HarnessError('database model lacks its original directory totals')
    rows = body[start:total.start()]
    roles = dump['directory_roles']
    seen = set()
    files = impossibilities = 0

    def render_row(match: re.Match[bytes]) -> bytes:
        nonlocal files, impossibilities
        alias = os.fsdecode(match[2])
        if alias in seen or alias not in roles:
            raise h.HarnessError('directory row lacks a unique declared cache owner')
        seen.add(alias)
        count, impossible, complete = directories[roles[alias]].counts()
        files += count; impossibilities += impossible
        return (match[1] + f'{count or "No"} files, {impossible or "no"} impossibilities'.encode()
                + (b'.\n' if complete else b' so far.\n'))

    rows = DIR_ROW.sub(render_row, rows)
    if seen != set(roles):
        raise h.HarnessError('declared cache owners disagree with original directory row inventory')
    summary = (f'# {files or "No"} files, {impossibilities or "no"} impossibilities in '.encode()
               + total[1] + b' directories.\n')
    return body[:start] + rows + summary + body[total.end():]


def render_model(template: bytes, count: int, inputs: PublicInputs) -> bytes:
    """Construct every expected byte from frozen literals and declared inputs.

    Regexes here parse only the checksummed *expected template*. No observed
    stdout is used to select a role, a count, a slot, or an expected replacement.
    """
    if hashlib.sha256(template).hexdigest() != inputs.plan['template_sha256']:
        raise h.HarnessError('portable plan belongs to different original template bytes')
    opens, closes = list(DUMP_OPEN.finditer(template)), list(DUMP_CLOSE.finditer(template))
    if len(opens) != count or len(closes) != count or len(inputs.plan['dumps']) != count:
        raise h.HarnessError('portable dump inventory disagrees with original template')
    rendered = []
    offset = 0
    for opening, closing, dump in zip(opens, closes, inputs.plan['dumps']):
        if not offset <= opening.start() < opening.end() <= closing.start():
            raise h.HarnessError('portable model has crossing dump boundaries')
        rendered.append(template[offset:opening.end()])
        body = template[opening.end():closing.start()]
        strings, directories = replay(dump, inputs)
        includedirs = dump.get('include_line')
        if includedirs is not None:
            original = bytes.fromhex(includedirs['original_hex'])
            if not original.startswith(b'.INCLUDE_DIRS := ') or body.count(b'\n' + original) != 1:
                raise h.HarnessError('portable include operand lacks a unique original variable field')
            value = b' '.join(inputs.config.default_paths).replace(b'$', b'$$')
            if b'\n' in value or b'\0' in value or re.search(rb'@[A-Z_0-9]+@', value):
                raise h.HarnessError('unsupported configured include operand spelling')
            prefix, suffix = bytes.fromhex(includedirs['prefix_hex']), bytes.fromhex(includedirs['suffix_hex'])
            body = body.replace(b'\n' + original, b'\n' + prefix + value + suffix, 1)
        body = render_directory_fields(body, dump, directories)
        starts = list(CACHE_START.finditer(body))
        if len(starts) != 1 or starts[0].start() == 0 or body[starts[0].start() - 1] != 10:
            raise h.HarnessError('portable model lacks one original cache footer')
        rendered.append(body[:starts[0].start() - 1] + strings.render(inputs.witness, dump['rounding']))
        offset = closing.start()
    rendered.append(template[offset:])
    return b''.join(rendered)


@dataclass(frozen=True)
class Preparation:
    config: BuildConfiguration
    witness: PublicWitness
    plans: dict[str, dict[str, Any]]

    @classmethod
    def load(cls, profile: dict[str, Any], path: Path, directory: Path) -> 'Preparation':
        data = json.loads(path.read_bytes())
        if data.get('schema_version') != 1 or not data['models']:
            raise h.HarnessError('unsupported portable database input plan inventory')
        return cls(configuration(profile), PublicWitness.compile(directory), data['models'])

    def plan(self, case_id: str, template: bytes, dump_count: int) -> dict[str, Any] | None:
        if dump_count == 0:
            return None
        if case_id not in self.plans:
            raise h.HarnessError('missing independent database input plan: ' + case_id)
        plan = self.plans[case_id]
        if hashlib.sha256(template).hexdigest() != plan['template_sha256'] or len(plan['dumps']) != dump_count:
            raise h.HarnessError('portable input plan does not match selected original model: ' + case_id)
        return plan

    def capture(self, cwd: Path, plan: dict[str, Any]) -> dict[str, dict[str, Any]]:
        names = plan['capture_paths']
        if not names:
            return {}
        if len(names) != len(set(names)) or len(names) > 64:
            raise h.HarnessError('invalid public directory path inventory')
        for name in names:
            path = Path(name)
            if not name or path.is_absolute() or '..' in path.parts or '\0' in name:
                raise h.HarnessError('public database directory escaped the fixture root')
        # Keep a trailing slash in the request, although it does not affect
        # stat identity. It is a distinct logical database spelling.
        values = self.witness.directories([os.fsencode(cwd) + b'/' + os.fsencode(name) for name in names])
        return dict(zip(names, values, strict=True))
