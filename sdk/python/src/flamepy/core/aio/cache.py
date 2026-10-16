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

Async object-cache transport and operations.

The wire format and client-side version cache are shared with the synchronous
namespace. Channels are owned by the event loop that created them.
"""

import asyncio
import contextlib
import logging
import os
import tempfile
import threading
from typing import Any, AsyncIterator, Awaitable, Iterable, List, Optional
from urllib.parse import urlparse

import grpc
import pyarrow as pa

from flamepy.core import cache as common
from flamepy.core.cache import Deserializer, FetchMode, FetchResult, Object, ObjectKey, ObjectRef, Patch, Ref, ValueRef
from flamepy.core.types import FlameClientCache, FlameClientTls
from flamepy.proto import cache_pb2, cache_pb2_grpc

logger = logging.getLogger(__name__)

# Explicit close() is required before the owning loop exits. A channel can
# retain its loop, so weak loop keys would not make forgotten channels safe.
_clients: dict[asyncio.AbstractEventLoop, dict[tuple[str, Optional[str], Optional[str], Optional[str], Optional[str]], tuple[grpc.aio.Channel, Any]]] = {}
_clients_lock = threading.Lock()


def _reset_clients_after_fork() -> None:
    global _clients, _clients_lock
    _clients = {}
    _clients_lock = threading.Lock()


if hasattr(os, "register_at_fork"):
    os.register_at_fork(after_in_child=_reset_clients_after_fork)


def _create_client(location: str, tls_config: Optional[FlameClientTls], target: Optional[str], *, dedicated: bool = False):
    parsed = urlparse(location)
    if parsed.scheme not in ("grpc", "grpcs", "grpc+tls", "grpcs-proxy") or not parsed.netloc:
        raise ValueError(f"Invalid object cache endpoint: {location}")
    options = list(common.GRPC_OPTIONS)
    if dedicated:
        # Without a local subchannel pool, channels to one target share a TCP
        # connection, which would defeat per-part channels.
        options.append(("grpc.use_local_subchannel_pool", 1))
    if parsed.scheme in ("grpcs", "grpc+tls", "grpcs-proxy"):
        if tls_config is not None:
            tls_config.validate_identity()
        if parsed.scheme == "grpcs-proxy":
            common._validate_proxy_endpoint(location)
        roots = None
        if tls_config and tls_config.ca_file:
            with open(tls_config.ca_file, "rb") as source:
                roots = source.read()
        private_key = certificate_chain = None
        if tls_config and tls_config.cert_file:
            with open(tls_config.key_file, "rb") as source:
                private_key = source.read()
            with open(tls_config.cert_file, "rb") as source:
                certificate_chain = source.read()
        credentials = grpc.ssl_channel_credentials(root_certificates=roots, private_key=private_key, certificate_chain=certificate_chain)
        channel = grpc.aio.secure_channel(parsed.netloc, credentials, options=options)
    else:
        channel = grpc.aio.insecure_channel(parsed.netloc, options=options)
    stub = cache_pb2_grpc.ObjectCacheServiceStub(channel)
    if target is not None:
        stub = _RoutedCacheStub(stub, target)
    return channel, stub


class _RoutedCacheStub:
    """Add the owner route to proxy RPCs without changing TLS authority."""

    def __init__(self, stub: Any, target: str):
        self._stub = stub
        self._target = target

    def __getattr__(self, name: str):
        rpc = getattr(self._stub, name)

        def invoke(*args, **kwargs):
            metadata = list(kwargs.pop("metadata", ()) or ())
            metadata.append(("x-flame-object-cache", self._target))
            return rpc(*args, metadata=metadata, **kwargs)

        return invoke


def _get_client(endpoint: str, tls_config: Optional[FlameClientTls] = None):
    location, target = common._resolve_cache_endpoint(endpoint)
    key = (location, target, tls_config.ca_file if tls_config else None, tls_config.cert_file if tls_config else None, tls_config.key_file if tls_config else None)
    with _clients_lock:
        pool = _clients.setdefault(asyncio.get_running_loop(), {})
        if key not in pool:
            pool[key] = _create_client(location, tls_config, target)
        return pool[key][1]


@contextlib.asynccontextmanager
async def _dedicated_client(endpoint: str, tls_config: Optional[FlameClientTls]):
    """A client on its own channel and connection, closed on exit."""
    location, target = common._resolve_cache_endpoint(endpoint)
    channel, stub = _create_client(location, tls_config, target, dedicated=True)
    try:
        yield stub
    finally:
        await channel.close()


async def _gather_or_cancel(coroutines: Iterable[Awaitable[Any]]) -> None:
    """Run coroutines concurrently; on the first failure cancel and await the rest."""
    tasks = [asyncio.ensure_future(coroutine) for coroutine in coroutines]
    try:
        await asyncio.gather(*tasks)
    except BaseException:
        for task in tasks:
            task.cancel()
        await asyncio.gather(*tasks, return_exceptions=True)
        raise


async def close() -> None:
    """Close this event loop's cache channels before the loop stops."""
    with _clients_lock:
        pool = _clients.pop(asyncio.get_running_loop(), {})
    for channel, _ in pool.values():
        await channel.close()


def _configured_endpoint():
    cache_config = common._get_cached_context().cache
    if cache_config is None:
        raise ValueError("Cache configuration not found")
    if isinstance(cache_config, str):
        endpoint, tls = cache_config, None
    elif isinstance(cache_config, FlameClientCache):
        endpoint, tls = cache_config.endpoint, cache_config.tls
    else:
        endpoint, tls = cache_config.get("endpoint"), None
    if not endpoint:
        raise ValueError("Cache endpoint not configured")
    return endpoint, tls


async def _write_remote(
    client: Any,
    key: str,
    data_type: str,
    chunks: AsyncIterator[bytes],
    *,
    patch: bool = False,
    timeout: Optional[int] = None,
) -> ObjectRef:
    async def requests():
        header = cache_pb2.CacheWriteHeader(key=key, data_type=data_type)
        yield cache_pb2.CacheWriteRequest(header=header)
        async for chunk in chunks:
            yield cache_pb2.CacheWriteRequest(data=chunk)

    rpc = client.Patch if patch else client.Put
    metadata = await rpc(requests(), timeout=timeout)
    return ObjectRef(endpoint=metadata.endpoint, key=metadata.key, version=metadata.version)


async def _byte_chunks(data: Any):
    view = memoryview(data)
    for start in range(0, len(view), common._UPLOAD_CHUNK_SIZE):
        yield view[start : start + common._UPLOAD_CHUNK_SIZE].tobytes()


async def _delete_refs(refs: List[ObjectRef], tls: Optional[FlameClientTls], parallelism: int) -> None:
    """Delete objects by full key, each on its own owner over its own channel."""
    semaphore = asyncio.Semaphore(parallelism)

    async def delete(ref: ObjectRef) -> None:
        async with semaphore:
            async with _dedicated_client(ref.endpoint, tls) as client:
                await client.Delete(cache_pb2.CacheDeleteRequest(key=ref.key))

    await _gather_or_cancel(delete(ref) for ref in refs)


async def _put_multipart(prefix: str, data_type: str, data: bytes, endpoint: str, tls: Optional[FlameClientTls], part_size: int, parallelism: int) -> ObjectRef:
    """Write parts concurrently, each on its own channel, then the manifest last."""
    view = memoryview(data)
    ranges = common._part_ranges(len(view), part_size)
    refs: List[Optional[ObjectRef]] = [None] * len(ranges)
    semaphore = asyncio.Semaphore(parallelism)

    async def write_part(index: int) -> None:
        start, end = ranges[index]
        async with semaphore:
            async with _dedicated_client(endpoint, tls) as client:
                refs[index] = await _write_remote(client, prefix, common._TYPE_RAW, _byte_chunks(view[start:end]))

    try:
        await _gather_or_cancel(write_part(index) for index in range(len(ranges)))
        manifest = common._encode_manifest(data_type, len(view), [(ref, end - start) for ref, (start, end) in zip(refs, ranges)])
        return await _write_remote(_get_client(endpoint, tls), prefix, common._TYPE_MULTIPART, _byte_chunks(manifest))
    except BaseException:
        # Parts without a manifest are unreachable; do not leave them behind.
        written = [ref for ref in refs if ref is not None]
        try:
            await _delete_refs(written, tls, parallelism)
        except Exception as exc:
            logger.warning("put_object: failed to clean up %d parts: %s", len(written), exc)
        raise


async def put_object(
    key_prefix: str,
    obj: Any,
    *,
    part_size: Optional[int] = None,
    parallelism: int = common._MULTIPART_PARALLELISM,
) -> ObjectRef:
    object_key = ObjectKey.from_prefix(key_prefix)
    part_size = common._MULTIPART_PART_SIZE if part_size is None else part_size
    if part_size <= 0 or parallelism <= 0:
        raise ValueError("part_size and parallelism must be positive")
    endpoint, tls = _configured_endpoint()
    data_type, data = await asyncio.to_thread(common._encode_object_data, obj)
    if len(data) > part_size:
        ref = await _put_multipart(object_key.to_prefix(), data_type, data, endpoint, tls, part_size, parallelism)
    else:
        ref = await _write_remote(_get_client(endpoint, tls), object_key.to_prefix(), data_type, _byte_chunks(data))
    logger.debug("put_object: key=%s, version=%s", ref.key, ref.version)
    return ref


async def _read_get_parts(responses: Any):
    responses = responses.__aiter__()
    try:
        first = await responses.__anext__()
    except StopAsyncIteration:
        first = None
    if first is None:
        raise ValueError("Cache Get returned no header")
    if first.WhichOneof("payload") != "header":
        raise ValueError("Cache Get must start with a header")
    header = first.header
    parts = []
    kind = version = None
    data = bytearray()
    async for response in responses:
        if response.WhichOneof("payload") != "chunk":
            raise ValueError("Unexpected Cache Get header after data")
        chunk = response.chunk
        if kind is None:
            kind, version = chunk.kind, chunk.version
        elif (chunk.kind, chunk.version) != (kind, version):
            parts.append((kind, version, bytes(data)))
            kind, version = chunk.kind, chunk.version
            data.clear()
        data.extend(chunk.data)
    if kind is not None:
        parts.append((kind, version, bytes(data)))
    return header, parts


def _decode_parts(header: Any, parts: list) -> Optional[FetchResult]:
    if header.mode == cache_pb2.CACHE_GET_MODE_NOT_MODIFIED:
        if parts:
            raise ValueError("NOT_MODIFIED response included data")
        return None
    if header.mode not in (cache_pb2.CACHE_GET_MODE_FULL, cache_pb2.CACHE_GET_MODE_PATCHES):
        raise ValueError(f"Invalid Cache Get mode: {header.mode}")
    data_type, compression = common._type_and_compression(header.data_type)
    if header.mode == cache_pb2.CACHE_GET_MODE_FULL:
        if not parts or parts[0][0] != cache_pb2.CACHE_CHUNK_KIND_BASE:
            raise ValueError("Full object response must start with a base")
        base = common._deserialize_object_data(data_type, common._decompress_data(parts[0][2], compression))
        patch_parts = parts[1:]
        mode = FetchMode.FULL
    else:
        base = None
        patch_parts = parts
        mode = FetchMode.PATCHES
    if any(kind != cache_pb2.CACHE_CHUNK_KIND_PATCH for kind, _, _ in patch_parts):
        raise ValueError("Cache Get response has an unexpected part kind")
    versions = [version for _, version, _ in patch_parts]
    if versions != sorted(set(versions)):
        raise ValueError("Patch response versions must be unique and increasing")
    patches = [Patch(version=version, data=common._deserialize_object_data(data_type, common._decompress_data(data, compression))) for _, version, data in patch_parts]
    return FetchResult(mode=mode, version=header.version, base=base, patches=patches)


async def _read_part_into(responses: Any, view: memoryview, expected_version: int) -> None:
    """Stream one part into `view`, which must be exactly the part's size."""
    offset = 0
    header = None
    async for response in responses:
        payload = response.WhichOneof("payload")
        if header is None:
            if payload != "header":
                raise ValueError("Cache Get must start with a header")
            header = response.header
            if header.mode != cache_pb2.CACHE_GET_MODE_FULL or header.version != expected_version:
                raise ValueError(f"Multipart part changed: expected full version {expected_version}, got mode {header.mode} version {header.version}")
            continue
        if payload != "chunk" or response.chunk.kind != cache_pb2.CACHE_CHUNK_KIND_BASE:
            raise ValueError("Multipart part must consist of base chunks only")
        end = offset + len(response.chunk.data)
        if end > len(view):
            raise ValueError(f"Multipart part is larger than the {len(view)} bytes in the manifest")
        view[offset:end] = response.chunk.data
        offset = end
    if header is None:
        raise ValueError("Cache Get returned no header")
    if offset != len(view):
        raise ValueError(f"Multipart part is {offset} bytes, manifest says {len(view)}")


async def _assemble_multipart(header: Any, parts: list, tls: Optional[FlameClientTls], parallelism: int = common._MULTIPART_PARALLELISM) -> tuple[Any, list]:
    """Fetch a manifest's parts into one pre-sized buffer and describe it as a plain full object."""
    if header.mode != cache_pb2.CACHE_GET_MODE_FULL or len(parts) != 1 or parts[0][0] != cache_pb2.CACHE_CHUNK_KIND_BASE:
        raise ValueError("A multipart manifest must be a full object without patches")
    data_type, total_size, entries = common._decode_manifest(parts[0][2])
    buffer = bytearray(total_size)
    view = memoryview(buffer)
    semaphore = asyncio.Semaphore(parallelism)

    async def fetch_part(ref: ObjectRef, start: int, size: int) -> None:
        async with semaphore:
            async with _dedicated_client(ref.endpoint, tls) as client:
                await _read_part_into(client.Get(cache_pb2.CacheGetRequest(key=ref.key, client_version=0)), view[start : start + size], ref.version)

    start = 0
    jobs = []
    for ref, size in entries:
        jobs.append(fetch_part(ref, start, size))
        start += size
    await _gather_or_cancel(jobs)
    full = cache_pb2.CacheGetHeader(mode=cache_pb2.CACHE_GET_MODE_FULL, version=header.version, data_type=data_type)
    return full, [(cache_pb2.CACHE_CHUNK_KIND_BASE, header.version, buffer)]


async def _fetch_object_data(ref: ObjectRef, cached_version: int) -> Optional[FetchResult]:
    tls = common._get_cache_tls_config()
    client = _get_client(ref.endpoint, tls)
    header, parts = await _read_get_parts(client.Get(cache_pb2.CacheGetRequest(key=ref.key, client_version=cached_version)))
    if header.data_type == common._TYPE_MULTIPART:
        header, parts = await _assemble_multipart(header, parts, tls)
    return await asyncio.to_thread(_decode_parts, header, parts)


async def get_object(ref: Ref, deserializer: Optional[Deserializer] = None) -> Any:
    if isinstance(ref, ValueRef):
        return ref.value if deserializer is None else await asyncio.to_thread(deserializer, ref.value, [])
    if not isinstance(ref, ObjectRef):
        raise TypeError(f"Expected ValueRef or ObjectRef, got {type(ref).__name__}")
    ObjectKey.from_key(ref.key)
    cache_key = (ref.endpoint, ref.key)
    cached = None if ref.version == 0 else common._cache_get(cache_key)
    cached_version = cached.version if cached else 0
    result = await _fetch_object_data(ref, cached_version)
    if result is None:
        cached = common._cache_get(cache_key) if cached_version else None
        if cached is None:
            raise ValueError(f"Object not found: {ref.key}")
    elif result.mode == FetchMode.FULL:
        cached = Object(version=result.version, data=result.base, patches=result.patches)
        cached = common._cache_put(cache_key, cached)
    elif result.mode == FetchMode.PATCHES:
        cached = common._cache_apply_patches(cache_key, cached_version, result.version, result.patches)
        if cached is None:
            full = await _fetch_object_data(ref, 0)
            if full is None or full.mode != FetchMode.FULL:
                raise ValueError(f"Object not found: {ref.key}")
            cached = Object(version=full.version, data=full.base, patches=full.patches)
            cached = common._cache_put(cache_key, cached)
    else:
        raise ValueError(f"Unexpected object fetch mode: {result.mode}")
    if deserializer is None:
        return cached.data
    return await asyncio.to_thread(common._materialize_object, cached, deserializer)


async def update_object(ref: ObjectRef, new_obj: Any) -> ObjectRef:
    ObjectKey.from_key(ref.key)
    client = _get_client(ref.endpoint, common._get_cache_tls_config())
    try:
        metadata = await client.GetMetadata(cache_pb2.CacheGetMetadataRequest(key=ref.key))
        _reject_multipart(metadata.data_type, ref.key, "update")
    except grpc.aio.AioRpcError as exc:
        if exc.code() != grpc.StatusCode.NOT_FOUND:
            raise
    data_type, data = await asyncio.to_thread(common._encode_object_data, new_obj)
    updated = await _write_remote(client, ref.key, data_type, _byte_chunks(data))
    common._cache_remove((ref.endpoint, ref.key))
    return updated


def _reject_multipart(data_type: str, key: str, operation: str) -> None:
    if data_type == common._TYPE_MULTIPART:
        raise ValueError(f"Cannot {operation} multipart object {key}: its parts would be orphaned or corrupted; delete it and put a new object")


async def patch_object(ref: ObjectRef, delta: Any) -> ObjectRef:
    ObjectKey.from_key(ref.key)
    client = _get_client(ref.endpoint, common._get_cache_tls_config())
    metadata = await client.GetMetadata(cache_pb2.CacheGetMetadataRequest(key=ref.key))
    _reject_multipart(metadata.data_type, ref.key, "patch")
    stored_type, compression = common._type_and_compression(metadata.data_type)
    data_type, data = await asyncio.to_thread(common._serialize_object_data, delta)
    if data_type != stored_type:
        raise ValueError(f"Patch data type {data_type!r} does not match cached object type {stored_type!r}")
    data = await asyncio.to_thread(common._compress_data, data, compression)
    updated = await _write_remote(client, ref.key, metadata.data_type, _byte_chunks(data), patch=True)
    common._cache_remove((ref.endpoint, ref.key))
    return updated


async def _delete_multipart_parts(client: Any, object_key: ObjectKey, tls: Optional[FlameClientTls]) -> None:
    """Deleting a manifest by key also deletes its parts, which may live on other instances.

    Deleting a prefix needs no such step: the parts share it.
    """
    try:
        metadata = await client.GetMetadata(cache_pb2.CacheGetMetadataRequest(key=str(object_key)))
    except grpc.aio.AioRpcError as exc:
        if exc.code() == grpc.StatusCode.NOT_FOUND:
            return
        raise
    if metadata.data_type != common._TYPE_MULTIPART:
        return
    _, parts = await _read_get_parts(client.Get(cache_pb2.CacheGetRequest(key=str(object_key), client_version=0)))
    _, _, entries = common._decode_manifest(parts[0][2])
    await _delete_refs([ref for ref, _ in entries], tls, common._MULTIPART_PARALLELISM)


async def delete_objects(key_prefix: str) -> None:
    object_key = ObjectKey.from_path(key_prefix)
    endpoint, tls = _configured_endpoint()
    client = _get_client(endpoint, tls)
    if object_key.object_id is not None:
        await _delete_multipart_parts(client, object_key, tls)
    await client.Delete(cache_pb2.CacheDeleteRequest(key=str(object_key)))
    common._cache_remove_matching(object_key)


async def upload_object(key_or_prefix: str, file_path: str, endpoint: Optional[str] = None) -> ObjectRef:
    if not await asyncio.to_thread(os.path.exists, file_path):
        raise FileNotFoundError(f"File not found: {file_path}")
    object_key = ObjectKey.from_path(key_or_prefix)
    if object_key.is_all_sessions():
        raise ValueError(f"Invalid key format: {key_or_prefix}")
    if endpoint is None:
        endpoint, tls = _configured_endpoint()
    else:
        try:
            config = common._get_cached_context().cache
        except Exception:
            config = None
        tls = config.tls if isinstance(config, FlameClientCache) else None

    async def chunks():
        with open(file_path, "rb") as source:
            while data := await asyncio.to_thread(source.read, common._UPLOAD_CHUNK_SIZE):
                yield data

    try:
        return await _write_remote(_get_client(endpoint, tls), str(object_key), common._TYPE_RAW, chunks(), timeout=300)
    except Exception as exc:
        raise ValueError(f"Failed to upload file to cache server: {exc}") from exc


async def download_object(ref: ObjectRef, dest_path: str) -> None:
    ObjectKey.from_key(ref.key)
    client = _get_client(ref.endpoint, common._get_cache_tls_config())
    dest_dir = os.path.dirname(dest_path)
    if dest_dir:
        await asyncio.to_thread(os.makedirs, dest_dir, exist_ok=True)
    try:
        responses = client.Get(cache_pb2.CacheGetRequest(key=ref.key, client_version=0)).__aiter__()
        try:
            first = await responses.__anext__()
        except StopAsyncIteration:
            first = None
        if first is None or first.WhichOneof("payload") != "header" or first.header.mode != cache_pb2.CACHE_GET_MODE_FULL:
            raise ValueError("Expected full object response")
        data_type, compression = common._type_and_compression(first.header.data_type)
        saw_base = False
        with open(dest_path, "wb") as output:
            if compression == common._COMPRESSION_NONE:
                async for response in responses:
                    if response.WhichOneof("payload") != "chunk" or response.chunk.kind != cache_pb2.CACHE_CHUNK_KIND_BASE or data_type != common._TYPE_RAW:
                        raise ValueError("download_object expected raw base chunks only")
                    saw_base = True
                    await asyncio.to_thread(output.write, response.chunk.data)
            else:
                with tempfile.TemporaryFile() as compressed:
                    async for response in responses:
                        if response.WhichOneof("payload") != "chunk" or response.chunk.kind != cache_pb2.CACHE_CHUNK_KIND_BASE or data_type != common._TYPE_RAW:
                            raise ValueError("download_object expected raw base chunks only")
                        saw_base = True
                        await asyncio.to_thread(compressed.write, response.chunk.data)

                    def decompress_file():
                        compressed.seek(0)
                        with pa.CompressedInputStream(pa.input_stream(compressed), common._COMPRESSION_ZSTD) as stream:
                            while data := stream.read(common._UPLOAD_CHUNK_SIZE):
                                output.write(data)

                    await asyncio.to_thread(decompress_file)
        if not saw_base:
            raise ValueError("Cache Get response omitted the base")
    except BaseException as exc:
        if os.path.exists(dest_path):
            await asyncio.to_thread(os.remove, dest_path)
        if not isinstance(exc, Exception):
            raise
        raise ValueError(f"Failed to download file from cache server: {exc}") from exc


__all__ = ["Ref", "ValueRef", "ObjectRef", "ObjectKey", "put_object", "get_object", "update_object", "patch_object", "delete_objects", "upload_object", "download_object", "close"]
