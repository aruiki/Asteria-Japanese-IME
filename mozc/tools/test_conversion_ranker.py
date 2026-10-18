import math
import unittest

from conversion_ranker import evaluate, examples, fit, loss_gradient


def observed(candidates=None):
    return dict(index="train-1", reading="はしをつかう", references=["reference"], final="other",
                source_groups=["corpus-train"], split="train", candidates=candidates or [
                    dict(text="other", zenz="-4", llm="-5"),
                    dict(text="reference", zenz="-1", llm="-2")])


class ListwiseRankingTest(unittest.TestCase):
    def test_actual_competitor_group_is_learned_without_changing_reference(self):
        data, _ = examples([observed()])
        weights = fit(data, 0.01, steps=100)
        self.assertTrue(all(math.isfinite(w) and w >= 0 for w in weights))
        self.assertEqual(evaluate(data, weights)["after"], 1)

    def test_post_ranking_surface_change_is_not_reported_as_ranking_improvement(self):
        data, _ = examples([observed()])
        report = evaluate(data, [1, 0.7])
        self.assertEqual(report["improved"], 0)
        self.assertEqual(report["regressed"], 0)
        self.assertEqual(report["pool_vs_final_surface_changes"], 1)
        self.assertEqual(report["observed_final_correct"], 0)

    def test_nonfinite_shortlisted_candidate_is_not_given_a_fake_score(self):
        row = observed([dict(text="reference", zenz="-1", llm="-inf"),
                        dict(text="other", zenz="-2", llm="-3")])
        with self.assertRaises(ValueError):
            examples([row])

    def test_nan_or_inconsistent_duplicate_score_is_rejected(self):
        for candidates in ([dict(text="reference", zenz="nan", llm="1")],
                           observed()["candidates"] + [dict(text="reference", zenz="-3", llm="-9")]):
            with self.assertRaises(ValueError):
                examples([observed(candidates)])

    def test_common_score_offset_does_not_change_loss_or_gradient(self):
        data, _ = examples([observed()])
        loss, gradient = loss_gradient(data[0], [0.4, 0.7], [1, 1])
        shifted = dict(data[0], x=[[v[0] + 100, v[1] - 400] for v in data[0]["x"]])
        new_loss, new_gradient = loss_gradient(shifted, [0.4, 0.7], [1, 1])
        self.assertAlmostEqual(loss, new_loss)
        self.assertEqual(gradient, new_gradient)

    def test_multiple_acceptable_references_are_not_trained_against_each_other(self):
        row = observed(); row["references"].append("other")
        with self.assertRaises(ValueError):
            examples([row])


if __name__ == "__main__":
    unittest.main()
