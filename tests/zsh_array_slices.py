"""Regressions for ranked records and capped history; no terminal required."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SEP = "\x1f"


class ArraySlicesTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="rfig-slices-")
        self.addCleanup(self.temp.cleanup)
        self.home = Path(self.temp.name)
        self.config = self.home / ".config/rfig"
        self.config.mkdir(parents=True)

    def run_zsh(self, script):
        result = subprocess.run(
            ["/bin/zsh", "-dfc", 'source "$1"\n' + script, "zsh", str(ROOT / "rfig.zsh")],
            env=dict(os.environ, HOME=str(self.home), ZDOTDIR=str(self.home)),
            cwd=self.home,
            capture_output=True,
            text=True,
            timeout=10,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        # zsh can report an arithmetic error and still exit successfully.
        self.assertEqual(result.stderr, "", result.stderr)
        return result.stdout.split("\0")[:-1]

    def test_ranking_preserves_records_and_stable_ties(self):
        labels = ["alpha", "bravo two", "charlie*", "delta'quote", "echo中文", "foxtrot", "golf", "hotel"]
        records = [f"{label}\t{label}\tdescription {i}\targument" for i, label in enumerate(labels)]
        for weights in (
            [], [1], [0] * 8, list(range(8)), list(range(8, 0, -1)),
            [4, 0, 0, 2, 1, 2, 0, 0],  # Multi-element suffix, then another scored insertion.
            [4, 3, 0, 2, 1, 2, 0, 0],  # Multi-element prefix.
            [4, 3, 0, 0, 2, 1, 2, 0],  # Both slices, plus scored and unscored ties.
        ):
            with self.subTest(weights=weights):
                (self.config / "usage.log").write_text("".join(
                    f"C{SEP}cd{SEP}{label}\n" * weight
                    for label, weight in zip(labels, weights)
                ))
                selected = records[:len(weights)]
                (self.home / "records").write_text("\n".join(selected))
                actual = self.run_zsh('''records=( "${(@f)$(<records)}" )
[[ -s records ]] || records=()
_rfig_rank_group cd "${records[@]}"
printf '%s\\0' "$#reply" "${reply[@]}"
''')
                expected = sorted(zip(selected, weights), key=lambda item: -item[1])
                self.assertEqual(actual, [str(len(selected))] + [record for record, _ in expected])

    def test_history_load_keeps_last_1000_lines_separate(self):
        lines = [f"echo record {i} 'two words' * 中文" for i in range(1002)]
        (self.config / "history.tsv").write_text("".join(
            f"{self.home.resolve()}{SEP}{line}\n/other-dir{SEP}unrelated\n" for line in lines
        ))
        actual = self.run_zsh('''printf '%s\\0' "$#_rfig_history_lines" "${_rfig_history_lines[@]}"
BUFFER='echo record 1000'
CURSOR=${#BUFFER}
_rfig_history_suggest
printf '%s\\0' "$_rfig_ghost"
''')
        self.assertEqual(actual, ["1000"] + lines[-1000:] + [" 'two words' * 中文"])

    def test_history_record_keeps_last_1000_lines_separate(self):
        lines = [f"echo record {i} 'two words' * 中文" for i in range(1002)]
        (self.config / "history.tsv").write_text("".join(
            f"{self.home.resolve()}{SEP}{line}\n" for line in lines[:1000]
        ))
        (self.home / "new-lines").write_text("\n".join(lines[1000:]) + "\n")
        actual = self.run_zsh('''while IFS= read -r line; do
  _rfig_history_record "$line"
done < new-lines
printf '%s\\0' "$#_rfig_history_lines" "${_rfig_history_lines[@]}"
''')
        self.assertEqual(actual, ["1000"] + lines[-1000:])
        self.assertEqual((self.config / "history.tsv").read_text().splitlines(), [
            f"{self.home.resolve()}{SEP}{line}" for line in lines
        ])


if __name__ == "__main__":
    unittest.main()
