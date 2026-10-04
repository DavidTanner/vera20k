"""Native/production comparison boundaries, using the retained real captures."""

import copy
import gzip
import json
from pathlib import Path
import unittest
from unittest.mock import patch

from tools.procedural_drawing_oracle import rally_production as production


class UnitMoveProductionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        archive = Path(__file__).resolve().parents[1] / 'procedural_drawing_oracle' / 'unit-move-production-validation'
        cls.cases = production.action_cases()
        cls.captures, cls.hashes = [], []
        for name in cls.cases:
            raw = gzip.decompress((archive / name / 'child-output/capture.json.gz').read_bytes())
            pixels = gzip.decompress((archive / name / 'child-output/frame.bgra.gz').read_bytes())
            cls.captures.append((json.loads(raw), pixels))
            cls.hashes.append(production.digest(raw))
        cls.receipt = production.document(archive / 'receipt.json')

    def test_retained_native_stores_and_real_input_controls(self):
        self.assertEqual(production.compare_action(self.captures, self.hashes),
                         self.receipt['comparison'])

    def test_original_pixel_mismatch_cannot_pass_on_receipt_hashes_alone(self):
        cases = copy.deepcopy(self.cases)
        cases['unit-move-active-v1']['pixels'][0][2] ^= 1
        with patch.object(production, 'action_cases', return_value=cases):
            with self.assertRaisesRegex(ValueError, 'original opaque store differs'):
                production.compare_action(self.captures, self.hashes)

    def test_captured_source_must_match_original_input(self):
        captures = copy.deepcopy(self.captures)
        index = list(self.cases).index('unit-move-active-v1')
        production.action_actor(captures[index][0]['observations']['frames'][-1])['physical_leptons'][0] += 256
        with self.assertRaisesRegex(ValueError, 'native source differs'):
            production.compare_action(captures, self.hashes)

    def test_unrelated_observed_consumer_change_is_not_a_line_difference(self):
        captures = copy.deepcopy(self.captures)
        index = list(self.cases).index('unit-move-reselect-v1')
        production.action_actor(captures[index][0]['observations']['frames'][-1], 1375)['health'] -= 1
        with self.assertRaisesRegex(ValueError, 'unexplained state delta'):
            production.compare_action(captures, self.hashes)

    def test_final_candidate_rechecks_all_pixels_and_observed_boundaries(self):
        captures = self.captures + copy.deepcopy(self.captures)
        hashes = self.hashes * 2
        result = production.compare_family('unit-move', captures, hashes)
        self.assertEqual(len(result['final_candidate']), 12)
        candidate, pixels = captures[12]
        changed = bytearray(pixels)
        changed[0] ^= 1
        captures[12] = candidate, bytes(changed)
        with self.assertRaisesRegex(ValueError, 'candidate frame differs'):
            production.compare_family('unit-move', captures, hashes)

    def test_final_candidate_cannot_hide_a_state_change_behind_equal_pixels(self):
        candidates = copy.deepcopy(self.captures)
        production.action_actor(candidates[0][0]['observations']['frames'][-1], 1375)['health'] -= 1
        with self.assertRaisesRegex(ValueError, 'candidate observations differs'):
            production.compare_family('unit-move', self.captures + candidates, self.hashes * 2)


if __name__ == '__main__':
    unittest.main()
