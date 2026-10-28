"""依存成果物生成の開発用回帰テスト。"""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("artifacts", Path(__file__).with_name("dependency-artifacts.py"))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class LicenseTests(unittest.TestCase):
    def test_寛容な選択肢を選ぶ(self):
        self.assertEqual(module.license_choice("MIT OR LGPL-2.1-or-later")[1], "MIT")
        self.assertEqual(module.license_choice("(MIT OR Apache-2.0) AND Unicode-3.0")[1], "(MIT) AND (Unicode-3.0)")
        self.assertEqual(module.license_choice("MIT/Apache-2.0")[0], "MIT OR Apache-2.0")

    def test_不明なライセンスと構文を拒否する(self):
        for value in ["MIT AND GPL-3.0-only", "GPL-3.0-only", "MIT OR", "(MIT", "MIT garbage", "", "MIT WITH Unknown"]:
            with self.subTest(value=value), self.assertRaises(ValueError):
                module.license_choice(value)


if __name__ == "__main__":
    unittest.main()
