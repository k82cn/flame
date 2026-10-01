import importlib.util
from pathlib import Path
import tempfile
import unittest


spec = importlib.util.spec_from_file_location(
    "migrate_workspaces", Path(__file__).with_name("migrate_workspaces.py")
)
migration = importlib.util.module_from_spec(spec)
spec.loader.exec_module(migration)


class MigrationTests(unittest.TestCase):
    def test_dry_run_and_apply_preserve_default_application(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for app in ("default", "other"):
                session = root / app / "session"
                session.mkdir(parents=True)
                (session / "object.bin").write_bytes(app.encode())

            self.assertEqual(migration.migrate(root), ["default", "other"])
            self.assertFalse((root / migration.MARKER).exists())
            migration.migrate(root, apply=True)
            for app in ("default", "other"):
                self.assertEqual(
                    (root / "default" / app / "session" / "object.bin").read_bytes(),
                    app.encode(),
                )
            self.assertTrue((root / migration.MARKER).exists())
            with self.assertRaises(ValueError):
                migration.migrate(root, apply=True)

    def test_rejects_unsafe_paths_and_partial_stage(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "bad..app").mkdir()
            with self.assertRaises(ValueError):
                migration.inventory(root)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / migration.STAGE).mkdir()
            with self.assertRaises(ValueError):
                migration.inventory(root)


if __name__ == "__main__":
    unittest.main()
