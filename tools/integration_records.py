# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Lossless integration records with shared, verified filesystem byte blobs."""
from __future__ import annotations

import base64
import gzip
import hashlib
import json
from pathlib import Path
import re

import conformance as h

ENCODING = 'integration-base64-cas-v1'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def json_bytes(value):
    return json.dumps(value, indent=2).encode() + b'\n'


def persist(directory, label, value):
    """Retain all observation bytes; deduplicate file contents, never comparisons."""
    if not re.fullmatch(r'[a-zA-Z0-9._-]+', label):
        raise h.HarnessError('invalid integration record label')
    blobs = directory / 'blobs'
    blobs.mkdir(exist_ok=True)

    def encode(item):
        if isinstance(item, list):
            return [encode(child) for child in item]
        if not isinstance(item, dict):
            return item
        result = {}
        for key, child in item.items():
            if key == 'content' and item.get('type') == 'file':
                if not isinstance(child, str):
                    raise h.HarnessError('filesystem content is not base64 bytes')
                data = base64.b64decode(child, validate=True)
                identity = digest(data)
                if len(data) != item['size'] or identity != item['sha256']:
                    raise h.HarnessError('filesystem content digest/size differs')
                path = blobs / (identity + '.gz')
                if path.exists():
                    if digest(gzip.decompress(path.read_bytes())) != identity:
                        raise h.HarnessError('integration byte blob changed')
                else:
                    path.write_bytes(gzip.compress(data, mtime=0))
                result[key] = {'encoding': ENCODING, 'sha256': identity}
            else:
                result[key] = encode(child)
        return result

    original = json_bytes(value)
    stored = json_bytes(encode(value))
    packed = gzip.compress(stored, mtime=0)
    path = directory / (label + '.json.gz')
    path.write_bytes(packed)
    return {'path': str(path), 'sha256': digest(packed), 'encoding': ENCODING,
            'stored_sha256': digest(stored), 'stored_bytes': len(stored),
            'raw_sha256': digest(original), 'raw_bytes': len(original)}


def restore(record, base=None):
    """Verify every referenced byte and reconstruct the original complete JSON."""
    path = Path(record['path'])
    if base is not None:
        if path.is_absolute() or '..' in path.parts:
            raise h.HarnessError('integration record escapes its bundle')
        path = base / path
    packed = path.read_bytes()
    if digest(packed) != record['sha256']:
        raise h.HarnessError('integration record digest changed')
    stored = gzip.decompress(packed)
    if record.get('encoding') != ENCODING:
        original = stored
    else:
        if digest(stored) != record['stored_sha256'] or len(stored) != record['stored_bytes']:
            raise h.HarnessError('integration stored record changed')

        def decode(item):
            if isinstance(item, list):
                return [decode(child) for child in item]
            if not isinstance(item, dict):
                return item
            result = {}
            for key, child in item.items():
                if key == 'content' and item.get('type') == 'file':
                    if not isinstance(child, dict) or child.get('encoding') != ENCODING or set(child) != {'encoding', 'sha256'} or not re.fullmatch(r'[0-9a-f]{64}', child['sha256']):
                        raise h.HarnessError('invalid integration blob reference')
                    data = gzip.decompress((path.parent / 'blobs' / (child['sha256'] + '.gz')).read_bytes())
                    if len(data) > h.MAX_FILE_BYTES or digest(data) != child['sha256']:
                        raise h.HarnessError('integration byte blob digest changed')
                    result[key] = base64.b64encode(data).decode('ascii')
                else:
                    result[key] = decode(child)
            return result

        original = json_bytes(decode(json.loads(stored)))
    if digest(original) != record['raw_sha256'] or len(original) != record['raw_bytes']:
        raise h.HarnessError('integration original record changed')
    return original
