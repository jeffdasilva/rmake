# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Negative controls for independently prepared exact database expectations."""
from dataclasses import replace
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

import conformance as h
import database_expected as expected
import database_output as database
import database_numeric as numeric


def directory(*names, inode=123):
    return {'errno': 0, 'device': 5, 'inode': inode,
            'entries': [{'name_hex': name.hex(), 'inode': 1000 + index, 'type': 8}
                        for index, name in enumerate(names)]}


class DatabaseExpectedTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory(prefix='rmake-database-expected-units-')
        cls.addClassCleanup(cls.temporary.cleanup)
        cls.root = Path(cls.temporary.name)
        cls.witness = expected.PublicWitness.compile(cls.root / 'witness')

    def test_hash_visible_identity_and_first_nominal_extent(self):
        # These are independent full-key fingerprints from the retained rich
        # witness, including a prefix longer than one complete mix round.
        self.assertEqual(expected.string_hash(b'.SUFFIXES'), 3058110371)
        self.assertEqual(expected.string_hash(b'/usr/local/include'), 3175264479)
        self.assertEqual(expected.string_hash(b'/usr/include'), 1583546682)
        cache = expected.StringCache()
        self.assertTrue(cache.add(b'search\0'))
        self.assertFalse(cache.add(b'search'))
        self.assertEqual(cache.state(), expected.CacheState((b'search',), 1, 8, 2, 0))
        with self.assertRaisesRegex(h.HarnessError, 'nominal span'):
            cache.add(b'absent', 7)

    def test_early_search_stop_and_later_eof_are_different_states(self):
        cache = expected.StringCache()
        dirs = expected.DirectoryCache(directory(b'.', b'..', b'input'))
        self.assertTrue(dirs.probe(b'input', cache))
        self.assertEqual(dirs.counts(), (3, 0, False))
        self.assertEqual(cache.state().keys, (b'.', b'..', b'input'))
        self.assertTrue(dirs.probe(b'input', cache))
        self.assertEqual(cache.state().lookups, 3)
        self.assertFalse(dirs.probe(b'absent', cache))
        self.assertEqual(dirs.counts(), (3, 0, True))
        dirs.impossible(b'absent')
        self.assertEqual(dirs.counts(), (3, 1, True))
        self.assertFalse(dirs.probe(b'absent', cache))
        self.assertEqual(cache.state().lookups, 3)

    def test_same_public_entries_can_have_distinct_prefix_counts(self):
        first = expected.StringCache(); second = expected.StringCache()
        initial = expected.DirectoryCache(directory(b'.', b'input', b'nput', b'..'))
        changed = expected.DirectoryCache(directory(b'.', b'nput', b'input', b'..'))
        self.assertTrue(initial.probe(b'input', first))
        self.assertTrue(changed.probe(b'input', second))
        self.assertEqual(initial.counts(), (2, 0, False))
        self.assertEqual(changed.counts(), (3, 0, False))
        self.assertEqual(first.state().storage, 8)
        self.assertEqual(second.state().storage, 13)

    def test_alias_and_probe_plan_refuse_inconsistent_public_inputs(self):
        config = expected.BuildConfiguration('unit', (), b'/p', (b'/p',), ())
        before = {'.': directory(b'input'), 'alias': directory(b'input', inode=999)}
        inputs = expected.PublicInputs(config, before, before, self.witness, {})
        actions = [['open', 'one', '.', 'before', True], ['alias', 'one', 'alias']]
        with self.assertRaisesRegex(h.HarnessError, 'same physical cache'):
            expected.replay({'steps': actions}, inputs)
        actions[-1] = ['probe', 'one', b'missing'.hex(), True]
        with self.assertRaisesRegex(h.HarnessError, 'probe outcome'):
            expected.replay({'steps': actions}, inputs)

    def test_actual_libc_witness_returns_open_identity_raw_names_and_rounding(self):
        root = self.root / 'raw-directory'; root.mkdir()
        (root / 'entry').touch()
        invalid = root / 'bad-\udcff'; invalid.touch()
        state, absent = self.witness.directories([bytes(root), bytes(root / 'missing')])
        self.assertEqual((state['device'], state['inode']), (root.stat().st_dev, root.stat().st_ino))
        self.assertEqual({bytes.fromhex(entry['name_hex']) for entry in state['entries']},
                         {b'.', b'..', b'entry', b'bad-\xff'})
        self.assertNotEqual(absent['errno'], 0)
        self.assertEqual(self.witness.numeric(1, 8, 'nearest', 'rounded'), '12')
        self.assertEqual(self.witness.numeric(1, 8, 'upward', 'rounded'), '13')
        self.assertEqual(self.witness.numeric(1, 8, 'downward', 'rounded'), '12')
        self.assertEqual(self.witness.numeric(1, 8, 'upward', 'truncated'), '12')
        with self.assertRaisesRegex(h.HarnessError, 'numeric witness input'):
            self.witness.numeric(1, 0, 'nearest', 'truncated')

    def test_sealed_plans_bind_to_every_original_literal_and_missing_fails(self):
        plans = json.loads(database.EXPECTED_INPUTS.read_bytes())['models']
        config = expected.BuildConfiguration('unit', (), b'/p', (b'/p',), ())
        prep = expected.Preparation(config, self.witness, plans)
        for model in database.models('all') + numeric.models():
            plan = prep.plan(model.id, model.stdout, model.dump_count)
            self.assertIsNotNone(plan)
            self.assertRegex(plan['trusted_trace_sha256'], r'^[0-9a-f]{64}$')
        with self.assertRaisesRegex(h.HarnessError, 'missing independent'):
            prep.plan('missing', b'x', 1)
        with self.assertRaisesRegex(h.HarnessError, 'does not match selected'):
            prep.plan('database-minimal', b'changed', 1)
        self.assertIsNone(prep.plan('no-dump', b'', 0))

    def test_agreeing_engines_cannot_hide_a_wrong_dynamic_counter(self):
        original = next(model for model in database.models('all') if model.id == 'database-minimal')
        plan = json.loads(database.EXPECTED_INPUTS.read_bytes())['models'][original.id]
        config = expected.BuildConfiguration('unit', (), b'/independent/short/include',
                                             (b'/independent/short/include', b'/usr/local/include', b'/usr/include'), ())
        captures = {'.': directory(b'.', b'..', b'seen', b'Makefile')}
        inputs = expected.PublicInputs(config, captures, captures, self.witness, plan)
        prepared = expected.render_model(original.stdout, original.dump_count, inputs)
        self.assertIn(b'.INCLUDE_DIRS := /independent/short/include /usr/local/include /usr/include\n', prepared)
        self.assertNotIn(b'/home/jdasilva', prepared)
        stdout = prepared.replace(b'@CTIME_OPEN@', b'Thu Jan  1 00:00:00 1970').replace(
            b'@CTIME_CLOSE@', b'Thu Jan  1 00:00:00 1970').replace(b'@DEVICE_0@', b'5').replace(b'@INODE_0@', b'123')
        stdout = database.render_paths(stdout, self.root)
        physical = {'.': {'device': 5, 'inode': 123, 'mode': 0o40755}}
        good = database.Execution(h.Observation(stdout, original.stderr, original.expected_status, None, {}, False),
                                  (0.0, 0.0), physical, physical, (0, 0), inputs)
        self.assertTrue(database.assess(original, self.root, {}, [good, good, good])['passed'])
        altered = stdout.replace(b'# strcache performance: lookups = ', b'# strcache performance: lookups = 9', 1)
        self.assertNotEqual(stdout, altered)
        bad = replace(good, observation=replace(good.observation, stdout=altered))
        rejected = database.assess(original, self.root, {}, [bad, bad, bad])
        self.assertFalse(rejected['passed'])
        self.assertTrue(all(rejected['expected_errors']))
        self.assertEqual(hashlib.sha256(original.stdout).hexdigest(), plan['template_sha256'])
