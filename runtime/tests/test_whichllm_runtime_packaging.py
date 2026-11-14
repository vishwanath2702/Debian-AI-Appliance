import importlib.util
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "build" / "prepare-whichllm-runtime.py"


def load_helper():
    spec = importlib.util.spec_from_file_location(
        "daia_whichllm_runtime_packager", SCRIPT
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class WhichLLMRuntimePackagingTests(unittest.TestCase):
    def test_rejects_missing_source(self):
        helper = load_helper()
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            with self.assertRaises(ValueError):
                helper.prepare(base / "missing", base / "out")

    def test_rejects_existing_destination(self):
        helper = load_helper()
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            source = base / "source"
            source.mkdir()
            destination = base / "destination"
            destination.mkdir()
            with self.assertRaises(ValueError):
                helper.prepare(source, destination)

    def test_patches_exact_upstream_contract(self):
        helper = load_helper()
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            source = base / "source"
            cli = source / "whichllm" / "cli.py"
            cli.parent.mkdir(parents=True)
            cli.write_text(
                "import typer\n"
                + "".join(
                    "x = typer.Option(4096, "
                    "click_type=CONTEXT_LENGTH)\n"
                    for _ in range(4)
                )
            )
            compatibility = (
                source / "whichllm" / "engine" / "compatibility.py"
            )
            compatibility.parent.mkdir(parents=True)
            compatibility.write_text(
                "def memory(gpu, vram_bytes, usable_ram):\n"
                "    if gpu.shared_memory and vram_bytes < 2 * _GiB "
                "and not gpu.vram_overridden:\n"
                "        return usable_ram\n"
                "    return vram_bytes\n"
            )
            destination = base / "destination"
            helper.prepare(source, destination)
            patched = (
                destination / "whichllm" / "cli.py"
            ).read_text()
            self.assertEqual(
                patched.count("parser=_daia_parse_context_length"), 4
            )
            self.assertNotIn("click_type=CONTEXT_LENGTH", patched)
            self.assertIn(
                "def _daia_parse_context_length(value)", patched
            )
            self.assertIn("click_type=CONTEXT_LENGTH", cli.read_text())


    def test_integrated_gpu_patch_preserves_other_vendors(self):
        helper = load_helper()
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            path = base / "whichllm" / "engine" / "compatibility.py"
            path.parent.mkdir(parents=True)
            path.write_text(
                "def memory(gpu, vram_bytes, usable_ram):\n"
                "    if gpu.shared_memory and vram_bytes < 2 * _GiB "
                "and not gpu.vram_overridden:\n"
                "        return usable_ram\n"
                "    return vram_bytes\n"
            )
            helper.patch_integrated_gpu_compatibility(base)
            patched = path.read_text()
            self.assertIn("if gpu.vendor == 'intel':", patched)
            self.assertIn("return usable_ram", patched)
            self.assertEqual(patched.count("return 0"), 1)

    def test_rejects_changed_gpu_contract(self):
        helper = load_helper()
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            path = base / "whichllm" / "engine" / "compatibility.py"
            path.parent.mkdir(parents=True)
            path.write_text("def unrelated():\n    return None\n")
            with self.assertRaises(ValueError):
                helper.patch_integrated_gpu_compatibility(base)


if __name__ == "__main__":
    unittest.main()
