"""Guards on historical/current publication evidence, without native replay."""
import copy
import json
import unittest

from tools.spatial_oracle._factory_infantry_output import fixture, runtime as rt, saved


class PublicationPhaseEvidenceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.meta = rt.metadata()['publication_phase']
        cls.replays = {order: rt.read_pinned(relative)
                       for order, relative in cls.meta['compatibility_receipts'].items()}
        # The old receipts retain their sealed input closure. The executed
        # compatibility wrappers bind the current callers independently.
        cls.receipts = {
            'historical': {order: rt.read_pinned(row['receipt'])
                           for order, row in cls.meta['controls'].items()},
            'current': {order: replay['full_original_publication_control']
                        for order, replay in cls.replays.items()},
        }

    def control_receipts(self, *branches):
        # Copy only branches a rejection control mutates. The complete warmed
        # native rounds stay shared and are never written.
        for origin, controls in self.receipts.items():
            for order, original in controls.items():
                receipt = dict(original)
                for name in branches:
                    receipt[name] = copy.deepcopy(original[name])
                yield origin, order, receipt

    def test_both_complete_original_receipts_pass(self):
        self.assertEqual(set(self.replays), set(self.meta['controls']))
        for origin, order, receipt in self.control_receipts():
            with self.subTest(origin=origin, order=order):
                result = saved.compare_publication_phase(
                    order, receipt, historical=origin == 'historical')
                self.assertTrue(result['complete_original_observations_equal'])
                self.assertEqual(result['complete_warmed_prefix_rounds'], 216)
                self.assertEqual(result['complete_normalized_original_sha256'],
                                 self.meta['controls'][order]['complete_normalized_original_sha256'])
                if origin == 'current':
                    replay = self.replays[order]
                    self.assertEqual(replay['status'], 'PASS')
                    self.assertEqual(replay['control'], 'publication_' + order)
                    self.assertEqual(result, replay['comparison'])

    def test_fixture_preserves_historical_serialization_and_current_selection(self):
        raw = (rt.REPO_ROOT / self.meta['rust_fixture']['path']).read_bytes()
        for origin, receipts in self.receipts.items():
            with self.subTest(origin=origin):
                selected = fixture.publication_phase_local_fixture(
                    receipts, self.meta['local_sources'], self.meta['parent_manifest_sha256'])
                self.assertEqual(json.loads(raw), selected)
                if origin == 'historical':
                    self.assertEqual(raw, json.dumps(selected, indent=2).encode())
                before, after = (selected['cases'][name] for name in ('before_strip', 'after_strip'))
                self.assertEqual(before['executed_event_types'], [30, 11])
                self.assertEqual(after['executed_event_types'], [11, 30])
                self.assertTrue(before['placed']['archive_is_set'])
                self.assertFalse(after['placed']['archive_is_set'])

    def test_reject_receipt_relabelled_to_the_other_source_origin(self):
        for origin, order, receipt in self.control_receipts():
            with self.subTest(origin=origin, order=order):
                with self.assertRaisesRegex(ValueError, 'Publication source driver differs'):
                    saved.compare_publication_phase(
                        order, receipt, historical=origin != 'historical')

    def test_reject_changed_source_claims_before_complete_comparison(self):
        claims = (
            (None, 'driver_sha256', 'Publication source driver differs'),
            ('caller_adaptation', 'original_owner_sha256', 'Publication initialized caller identity differs'),
            ('caller_adaptation', 'derived_caller_ast_sha256', 'Publication caller adaptation differs'),
            ('shared_helpers', 'profile_sha256', 'Publication shared-helper identity differs'),
            ('full_original_initialized_first_place', 'driver_sha256',
             'Publication embedded initialized source identity differs'),
            ('full_original_initialized_first_place', 'shared_helpers',
             'Publication embedded initialized source identity differs'),
        )
        for origin, order, original in self.control_receipts():
            for branch, name, error in claims:
                with self.subTest(origin=origin, order=order, branch=branch, field=name):
                    receipt = dict(original)
                    parent = receipt
                    if branch is not None:
                        parent = receipt[branch] = dict(original[branch])
                    value = parent[name]
                    parent[name] = (dict(value, profile_sha256='0' * 64)
                                    if isinstance(value, dict) else '0' * 64)
                    with self.assertRaisesRegex(ValueError, error):
                        saved.compare_publication_phase(
                            order, receipt, historical=origin == 'historical')

    def test_reject_native_code_change(self):
        for origin, order, receipt in self.control_receipts('control_state'):
            with self.subTest(origin=origin, order=order):
                receipt['control_state']['native_code_unchanged'] = False
                with self.assertRaisesRegex(ValueError, 'Original publication code changed'):
                    saved.validate_publication_phase_receipt(
                        receipt, self.meta['controls'][order], historical=origin == 'historical')

    def test_reject_truncated_complete_rng_buffer(self):
        for origin, order, receipt in self.control_receipts('complete_rng_buffers'):
            with self.subTest(origin=origin, order=order):
                row = next(iter(receipt['complete_rng_buffers'].values()))
                row['bytes'] = row['bytes'][:-2]
                with self.assertRaisesRegex(ValueError, 'Incomplete publication RNG buffer'):
                    saved.validate_publication_phase_receipt(
                        receipt, self.meta['controls'][order], historical=origin == 'historical')

    def test_reject_unwitnessed_strip_return(self):
        for origin, order, receipt in self.control_receipts('original_calls'):
            with self.subTest(origin=origin, order=order):
                row = next(row for row in receipt['original_calls'] if row['kind'] == 'factory_take_changed')
                del row['result']
                with self.assertRaisesRegex(ValueError, 'takeChanged return/write witness missing'):
                    saved.validate_publication_phase_receipt(
                        receipt, self.meta['controls'][order], historical=origin == 'historical')

    def test_reject_reversed_actual_event_order(self):
        for origin, order, receipt in self.control_receipts('original_calls'):
            with self.subTest(origin=origin, order=order):
                rows = [row for row in receipt['original_calls'] if row['kind'] == 'event_execute']
                rows[0]['event_bytes'], rows[1]['event_bytes'] = rows[1]['event_bytes'], rows[0]['event_bytes']
                with self.assertRaisesRegex(ValueError, 'publication event order differs'):
                    saved.validate_publication_phase_receipt(
                        receipt, self.meta['controls'][order], historical=origin == 'historical')

    def test_reject_missing_class_unlimbo_return(self):
        for origin, order, receipt in self.control_receipts('original_calls'):
            with self.subTest(origin=origin, order=order):
                row = next(row for row in receipt['original_calls'] if row['kind'] == 'infantry_unlimbo')
                del row['result']
                with self.assertRaisesRegex(ValueError, 'Infantry Unlimbo return missing'):
                    saved.validate_publication_phase_receipt(
                        receipt, self.meta['controls'][order], historical=origin == 'historical')

    def test_complete_comparison_preserves_unselected_observation(self):
        for origin, order, receipt in self.control_receipts('original_calls'):
            with self.subTest(origin=origin, order=order):
                receipt['original_calls'][0]['edx'] ^= 1
                with self.assertRaisesRegex(ValueError, 'Complete original publication observations differ'):
                    saved.compare_publication_phase(
                        order, receipt, historical=origin == 'historical')


if __name__ == '__main__':
    unittest.main()
