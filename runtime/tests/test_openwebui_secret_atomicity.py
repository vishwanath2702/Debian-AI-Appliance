import importlib.util
import multiprocessing
import os
import stat
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

SOURCE = (
    Path(__file__).resolve().parents[1]
    / "daia-openwebui-rag-entrypoint.py"
)


def load_module():
    spec = importlib.util.spec_from_file_location(
        "daia_openwebui_atomicity", SOURCE
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def process_worker(secret_path, start_event, result_queue):
    try:
        module = load_module()
        start_event.wait(timeout=15)
        with patch.dict(
            os.environ,
            {"WEBUI_SECRET_KEY_FILE": secret_path},
            clear=True,
        ):
            module.ensure_webui_secret_file()
            result_queue.put(("ok", os.environ["WEBUI_SECRET_KEY"]))
    except BaseException as error:
        result_queue.put(("error", repr(error)))


class SecretAtomicityTests(unittest.TestCase):
    def test_independent_processes_publish_one_secret(self):
        ctx = multiprocessing.get_context("spawn")

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "secret"
            start_event = ctx.Event()
            result_queue = ctx.Queue()
            workers = [
                ctx.Process(
                    target=process_worker,
                    args=(str(path), start_event, result_queue),
                )
                for _ in range(6)
            ]

            try:
                for worker in workers:
                    worker.start()

                start_event.set()

                results = [
                    result_queue.get(timeout=30)
                    for _ in workers
                ]

                for worker in workers:
                    worker.join(timeout=15)

                self.assertTrue(
                    all(worker.exitcode == 0 for worker in workers),
                    [(worker.pid, worker.exitcode) for worker in workers],
                )

                self.assertTrue(
                    all(status == "ok" for status, _ in results),
                    results,
                )

                values = [value for _, value in results]
                self.assertEqual(len(set(values)), 1)
                self.assertEqual(
                    path.read_text(encoding="ascii"),
                    values[0],
                )
                self.assertEqual(
                    stat.S_IMODE(path.stat().st_mode),
                    0o600,
                )
                self.assertEqual(
                    sorted(p.name for p in Path(directory).iterdir()),
                    ["secret"],
                )
            finally:
                for worker in workers:
                    if worker.is_alive():
                        worker.terminate()
                    worker.join(timeout=5)
                result_queue.close()
                result_queue.join_thread()

    def test_publication_failure_preserves_existing_secret(self):
        module = load_module()

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "secret"
            original = b"q" * 48
            path.write_bytes(original)
            path.chmod(0o600)

            with patch.dict(
                os.environ,
                {"WEBUI_SECRET_KEY_FILE": str(path)},
                clear=True,
            ):
                module.ensure_webui_secret_file()

            self.assertEqual(path.read_bytes(), original)
            self.assertEqual(
                sorted(p.name for p in Path(directory).iterdir()),
                ["secret"],
            )

    def test_link_failure_removes_private_temporary_file(self):
        module = load_module()

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "secret"

            with patch.dict(
                os.environ,
                {"WEBUI_SECRET_KEY_FILE": str(path)},
                clear=True,
            ):
                with patch.object(
                    module.os,
                    "link",
                    side_effect=OSError("injected link failure"),
                ):
                    with self.assertRaises(OSError):
                        module.ensure_webui_secret_file()

            self.assertFalse(path.exists())
            self.assertEqual(list(Path(directory).iterdir()), [])

    def test_existing_symlink_is_not_replaced(self):
        module = load_module()

        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "target"
            target.write_text("preserve", encoding="utf-8")
            link = Path(directory) / "secret"
            link.symlink_to(target)

            with patch.dict(
                os.environ,
                {"WEBUI_SECRET_KEY_FILE": str(link)},
                clear=True,
            ):
                with self.assertRaises(OSError):
                    module.ensure_webui_secret_file()

            self.assertTrue(link.is_symlink())
            self.assertEqual(
                target.read_text(encoding="utf-8"),
                "preserve",
            )


if __name__ == "__main__":
    unittest.main()
