# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Prove the random-seed validator rejects corruption outside its narrow field."""
import base64
import dataclasses
import unittest

import conformance as h
import shuffle_modes as s


class ShuffleModesTests(unittest.TestCase):
    def observation(self, seed=1):
        order = s.leaf_order(seed) if seed else [b'b', b'c', b'a']
        seen = b'anchor\n' + b'\n'.join(order) + b'\nall\n'
        return h.Observation(
            b'SEED=[--shuffle=' + str(seed).encode() + b']\nALL=[a b c]|[a b c]\n',
            b'', 0, None, {'work/seen': {'content': base64.b64encode(seen).decode()}})

    def test_fixed_seed_is_independently_modeled(self):
        self.assertEqual(s.validate(self.observation(), False), 1)
        changed = self.observation()
        changed.filesystem['work/seen']['content'] = base64.b64encode(b'anchor\na\na\nc\nall\n').decode()
        with self.assertRaises(AssertionError):
            s.validate(changed, False)

    def test_metadata_automatic_order_status_streams_and_bounds_are_required(self):
        observation = self.observation()
        mutations = ({'stdout': observation.stdout.replace(b'ALL=[a b c]', b'ALL=[c b a]')},
                     {'stdout': observation.stdout.replace(b'=1', b'=4294967296')},
                     {'returncode': 2}, {'stderr': b'unexpected\n'},
                     {'limit': 'timeout'}, {'leftover_processes': True})
        for changes in mutations:
            with self.subTest(changes=changes), self.assertRaises(AssertionError):
                s.validate(dataclasses.replace(observation, **changes), False)

    def test_zero_seed_has_permutation_properties_without_false_replay_credit(self):
        self.assertEqual(s.validate(self.observation(0), True), 0)
        with self.assertRaises(AssertionError):
            s.validate(self.observation(1), True)


if __name__ == '__main__':
    unittest.main()
