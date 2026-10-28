# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Independent anchors for initial source expansion and transfer ownership."""
import dataclasses
from pathlib import Path
import tomllib
import unittest
import conformance
ROOT = Path(__file__).resolve().parents[1]

class InitialExpansionTests(unittest.TestCase):
    def case(self, name):
        return conformance.load_case(ROOT/'tests/cases'/('initial-flags-'+name))
    def out(self, name):
        return bytes.fromhex(self.case(name).expected['stdout_hex'])
    def err(self, name):
        return bytes.fromhex(self.case(name).expected['stderr_hex'])

    def test_both_sources_profiles_and_effects_are_required(self):
        family=next(r for r in tomllib.loads((ROOT/'tests/requirements.toml').read_text())['requirements'] if r['id']=='cli.initial-expansion')
        self.assertEqual(family['fixture_count'],111)
        self.assertEqual(family['candidate_required'],111)
        self.assertEqual(len(set(family['cases'])),111)
        for source in ('makeflags','gnumakeflags'):
            for profile in ('defaults','r','big-r','r-big-r'):
                for effect in ('reference','nested','info','file','eval'):
                    self.assertEqual(self.case(f'{source}-{profile}-{effect}').candidate,'required')
            for kind in ('recursive','simple','append'):
                self.assertEqual(self.case(source+'-assignment-'+kind).candidate,'required')

    def test_source_order_clearing_and_command_priority_have_exact_anchors(self):
        self.assertEqual(self.out('source-order'),b'M=[one]/[command line]\nF=[one]\n')
        self.assertEqual(self.out('source-order-cli'),b'M=[one]\nF=[CLI]\n')
        self.assertEqual(self.out('cleared-gnu'),b'G\nM=[]/[environment]/[simple]\n')
        self.assertEqual(self.out('environment-override-clear'),b'M=[environment override]/[]\n[1]\n')
        self.assertEqual(self.out('mutate-makeflags'),b'G\n[undefined]\n')
        for source in ('makeflags','gnumakeflags'):
            self.assertEqual(self.err(source+'-warning-position-before'), b"make: warning: undefined variable 'MISSING'\n")
            self.assertEqual(self.err(source+'-warning-position-after'), b'')
            self.assertEqual(self.err(source+'-warning-position-cli'), b'')
        for source in ('makeflags','gnumakeflags'):
            self.assertEqual(self.out(source+'-assignment-recursive'),b'ENV\nBEFORE=[]\nX=[value]/[command line]\nAFTER=[late]\n')
            self.assertIn(b'BEFORE=[early]',self.out(source+'-assignment-simple'))
            self.assertEqual(self.out(source+'-literal-shell'),b'[value]/[E]\n')
            self.assertEqual(self.out(source+'-literal-tilde'),b'[before]/[after]\n')

    def test_raw_values_early_context_and_real_locations_remain_distinct(self):
        for source in ('makeflags','gnumakeflags'):
            self.assertEqual(self.out(source+'-early-defaults'),b'EARLY=[undefined]/[undefined]/[-c]\nFILE=[undefined]/[file]\n')
            self.assertEqual(self.out(source+'-early-loaded-undef'),b'[undefined]\n')
            self.assertEqual(self.err(source+'-error-include'),b'early.mk:1: *** INCLUDED.  Stop.\n')
            for suffix in ('error','error-help','error-version'):
                self.assertEqual(self.err(source+'-'+suffix),b'make: *** EARLY.  Stop.\n')
            for suffix,raw in (('raw-file',b'\xfe'),('raw-shell',b'\xff')):
                case=self.case(source+'-'+suffix);good=conformance.Observation(self.out(source+'-'+suffix),b'',0,None,{})
                self.assertEqual(conformance.expectation_errors(case,good),[])
                self.assertTrue(conformance.expectation_errors(case,dataclasses.replace(good,stdout=good.stdout.replace(raw,b'?'))))

    def test_restart_recursion_cwd_and_merged_order_have_negative_controls(self):
        for source in ('makeflags','gnumakeflags'):
            for mode in ('serial','parallel','pipe','sync'):
                self.assertEqual(self.out(source+'-restart-'+mode),b'[]/[]/[1]\n[1]/[1]/[2]\n')
            self.assertEqual(self.out(source+'-recur'),b'ENV\nP=[early]\nC=[]\n')
            self.assertEqual(self.out(source+'-cwd'),b'EARLY=[root]\nLATE=[sub]\n')
        for mode in ('none','line','target','recurse'):
            self.assertEqual(self.case('merged-'+mode).output,'merged')
            self.assertEqual(self.out('merged-'+mode),b'G\nmake: GW\nM\nmake: MW\nFILE\n')

if __name__=='__main__':unittest.main()
