"""DAIA offline recommendation integration smoke tests."""
import importlib.util
import os
import unittest
from pathlib import Path
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
from PyQt6.QtWidgets import QApplication
GUI = Path(__file__).resolve().parents[1] / "daia-model-manager.py"
SPEC = importlib.util.spec_from_file_location("daia_whichllm_gui", GUI)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)
class RecommendationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])
    def test_initial_state(self):
        window = MODULE.ModelManager()
        try:
            installed = window.centralWidget().widget(0)
            self.assertEqual(installed.recommendation_table.rowCount(), 0)
            self.assertTrue(installed.recommend_button.isEnabled())
            self.assertEqual([window.centralWidget().tabText(i) for i in range(3)],
                             ["Installed", "Import", "Export"])
        finally:
            window.close()
if __name__ == "__main__":
    unittest.main()
