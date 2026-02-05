"""
Copyright 2025 The Flame Authors.
Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at
    http://www.apache.org/licenses/LICENSE-2.0
Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
"""

import logging
import os
import shutil
from abc import ABC, abstractmethod
from urllib.parse import urlparse

import requests

from flamepy.core.types import FlameError, FlameErrorCode

logger = logging.getLogger(__name__)


class StorageBackend(ABC):
    """Abstract base class for storage backends.

    This interface provides a unified way to upload and delete packages
    from different storage systems (file, HTTP, etc.).
    """

    @abstractmethod
    def upload(self, local_path: str, filename: str) -> str:
        """Upload a package file to storage.

        Args:
            local_path: Path to the local package file
            filename: Name of the file in storage

        Returns:
            The full URL/path to the uploaded package

        Raises:
            FlameError: If upload fails
        """
        pass

    @abstractmethod
    def delete(self, filename: str) -> None:
        """Delete a package file from storage.

        Args:
            filename: Name of the file to delete

        Note:
            This method should not raise exceptions for non-existent files,
            as cleanup operations should be idempotent.
        """
        pass

    @abstractmethod
    def download(self, filename: str, local_path: str) -> None:
        """Download a package file from storage to local path.

        Args:
            filename: Name of the file in storage
            local_path: Path where the file should be saved locally

        Raises:
            FlameError: If download fails
        """
        pass


class FileStorage(StorageBackend):
    """File-based storage backend using local filesystem."""

    def __init__(self, storage_base: str):
        """Initialize file storage backend.

        Args:
            storage_base: Base storage URL (e.g., "file:///path/to/storage")

        Raises:
            FlameError: If storage directory doesn't exist
        """
        parsed_url = urlparse(storage_base)
        if parsed_url.scheme != "file":
            raise FlameError(FlameErrorCode.INVALID_CONFIG, f"Invalid file storage URL: {storage_base}")

        self._storage_dir = parsed_url.path

        # Ensure the storage directory exists
        if not os.path.exists(self._storage_dir):
            raise FlameError(FlameErrorCode.INVALID_CONFIG, f"Storage directory does not exist: {self._storage_dir}")

    def upload(self, local_path: str, filename: str) -> str:
        """Upload a package file to local filesystem storage.

        Args:
            local_path: Path to the local package file
            filename: Name of the file in storage

        Returns:
            The full file:// URL to the uploaded package

        Raises:
            FlameError: If upload fails
        """
        dest_path = os.path.join(self._storage_dir, filename)

        # Check if package already exists
        if os.path.exists(dest_path):
            logger.debug(f"Package already exists at {dest_path}, skipping upload")
        else:
            try:
                shutil.copy2(local_path, dest_path)
                logger.debug(f"Copied package to {dest_path}")
            except Exception as e:
                raise FlameError(FlameErrorCode.INTERNAL, f"Failed to copy package to storage: {str(e)}")

        # Return the full URL
        return f"file://{dest_path}"

    def delete(self, filename: str) -> None:
        """Delete a package file from local filesystem storage.

        Args:
            filename: Name of the file to delete
        """
        dest_path = os.path.join(self._storage_dir, filename)

        if os.path.exists(dest_path):
            try:
                os.remove(dest_path)
                logger.debug(f"Removed package from storage: {dest_path}")
            except Exception as e:
                logger.error(f"Error removing package from storage: {e}", exc_info=True)

    def download(self, filename: str, local_path: str) -> None:
        """Download a package file from local filesystem storage.

        Args:
            filename: Name of the file in storage
            local_path: Path where the file should be saved locally

        Raises:
            FlameError: If download fails
        """
        source_path = os.path.join(self._storage_dir, filename)

        if not os.path.exists(source_path):
            raise FlameError(FlameErrorCode.INTERNAL, f"File not found in storage: {source_path}")

        try:
            # Ensure the destination directory exists
            dest_dir = os.path.dirname(local_path)
            if dest_dir and not os.path.exists(dest_dir):
                os.makedirs(dest_dir, exist_ok=True)

            shutil.copy2(source_path, local_path)
            logger.debug(f"Downloaded package from {source_path} to {local_path}")
        except Exception as e:
            raise FlameError(FlameErrorCode.INTERNAL, f"Failed to download package from storage: {str(e)}")


class HttpStorage(StorageBackend):
    """HTTP-based storage backend using HTTP/HTTPS."""

    def __init__(self, storage_base: str, upload_timeout: int = 300, delete_timeout: int = 60, download_timeout: int = 300):
        """Initialize HTTP storage backend.

        Args:
            storage_base: Base storage URL (e.g., "http://storage.example.com/packages/")
            upload_timeout: Timeout in seconds for upload operations (default: 300)
            delete_timeout: Timeout in seconds for delete operations (default: 60)
            download_timeout: Timeout in seconds for download operations (default: 300)
        """
        parsed_url = urlparse(storage_base)
        if parsed_url.scheme not in ("http", "https"):
            raise FlameError(FlameErrorCode.INVALID_CONFIG, f"Invalid HTTP storage URL: {storage_base}")

        # Ensure storage_base ends with '/' for consistent URL construction
        self._storage_base = storage_base.rstrip("/") + "/"
        self._upload_timeout = upload_timeout
        self._delete_timeout = delete_timeout
        self._download_timeout = download_timeout

    def upload(self, local_path: str, filename: str) -> str:
        """Upload a package file to HTTP storage.

        Args:
            local_path: Path to the local package file
            filename: Name of the file in storage

        Returns:
            The full HTTP/HTTPS URL to the uploaded package

        Raises:
            FlameError: If upload fails
        """
        upload_url = f"{self._storage_base}{filename}"

        try:
            # Read the package file
            with open(local_path, "rb") as f:
                file_data = f.read()

            # Upload via PUT request
            response = requests.put(upload_url, data=file_data, timeout=self._upload_timeout)

            # Check if upload was successful
            if response.status_code in (200, 201, 204):
                logger.debug(f"Uploaded package to {upload_url}")
                return upload_url
            else:
                raise FlameError(
                    FlameErrorCode.INTERNAL,
                    f"Failed to upload package to {upload_url}: HTTP {response.status_code} - {response.text}",
                )
        except requests.exceptions.RequestException as e:
            raise FlameError(FlameErrorCode.INTERNAL, f"Failed to upload package to HTTP storage: {str(e)}")

    def delete(self, filename: str) -> None:
        """Delete a package file from HTTP storage.

        Args:
            filename: Name of the file to delete
        """
        delete_url = f"{self._storage_base}{filename}"

        try:
            response = requests.delete(delete_url, timeout=self._delete_timeout)
            if response.status_code in (200, 204, 404):
                logger.debug(f"Removed package from storage: {delete_url}")
            else:
                logger.warning(f"Failed to delete package from {delete_url}: HTTP {response.status_code}")
        except requests.exceptions.RequestException as e:
            logger.warning(f"Error deleting package from HTTP storage: {e}")

    def download(self, filename: str, local_path: str) -> None:
        """Download a package file from HTTP storage.

        Args:
            filename: Name of the file in storage
            local_path: Path where the file should be saved locally

        Raises:
            FlameError: If download fails
        """
        download_url = f"{self._storage_base}{filename}"

        try:
            # Download the file
            response = requests.get(download_url, timeout=self._download_timeout, stream=True)

            # Check if download was successful
            if response.status_code != 200:
                raise FlameError(
                    FlameErrorCode.INTERNAL,
                    f"Failed to download package from {download_url}: HTTP {response.status_code} - {response.text}",
                )

            # Ensure the destination directory exists
            dest_dir = os.path.dirname(local_path)
            if dest_dir and not os.path.exists(dest_dir):
                os.makedirs(dest_dir, exist_ok=True)

            # Write the file to local path
            with open(local_path, "wb") as f:
                for chunk in response.iter_content(chunk_size=8192):
                    if chunk:
                        f.write(chunk)

            logger.debug(f"Downloaded package from {download_url} to {local_path}")
        except requests.exceptions.RequestException as e:
            raise FlameError(FlameErrorCode.INTERNAL, f"Failed to download package from HTTP storage: {str(e)}")


def create_storage_backend(storage_base: str) -> StorageBackend:
    """Create a storage backend instance based on the storage URL scheme.

    Args:
        storage_base: Storage base URL (e.g., "file:///path" or "http://host/path")

    Returns:
        StorageBackend instance

    Raises:
        FlameError: If the storage scheme is not supported
    """
    parsed_url = urlparse(storage_base)

    if parsed_url.scheme == "file":
        return FileStorage(storage_base)
    elif parsed_url.scheme in ("http", "https"):
        return HttpStorage(storage_base)
    else:
        raise FlameError(
            FlameErrorCode.INVALID_CONFIG,
            f"Unsupported storage scheme: {parsed_url.scheme}. Supported schemes: file://, http://, https://",
        )
