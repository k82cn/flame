import os
import tarfile
import tempfile
from datetime import datetime, timezone

import pyarrow as pa  # type: ignore

import flamepy.runner.storage as storage
from flamepy.runner.storage import FileStorage, HttpStorage, create_storage_backend


def test_file_storage_basic(tmp_path):
    storage_dir = tmp_path
    base = f"file://{storage_dir}"
    fs = FileStorage(base)
    # Create a dummy local file to upload
    local = tmp_path / "pkg.txt"
    local.write_text("data")
    url = fs.upload(str(local), "pkg.txt")
    assert url.startswith("file://")
    # Download back to another path
    dest = tmp_path / "out.txt"
    fs.download("pkg.txt", str(dest))
    assert dest.exists()


def test_create_storage_backend_factory(tmp_path):
    path = tmp_path / "storage"
    path.mkdir()
    back = create_storage_backend(f"file://{path}")
    assert isinstance(back, FileStorage)
