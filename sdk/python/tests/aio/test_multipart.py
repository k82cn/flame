"""Multipart objects against an in-memory cache; no cluster needed."""

import contextlib
from types import SimpleNamespace

import grpc
import pytest

from flamepy.core import cache as cache_module
from flamepy.core.aio import cache as aio_cache
from flamepy.core.cache import ObjectRef, _cache_lock, _object_cache
from flamepy.proto import cache_pb2

ENDPOINT = "grpc://cache:9090"


class FakeCache:
    """Just enough of ObjectCacheService to store, read and delete objects."""

    def __init__(self):
        self.objects = {}  # key -> (version, data_type, bytes)
        self.fail_put_after = None  # fail the Put of the Nth part, 1-based
        self.puts = 0
        self.chunk_sizes = []

    async def Put(self, requests, timeout=None):  # noqa: N802
        header = None
        data = bytearray()
        async for request in requests:
            if header is None:
                header = request.header
            else:
                self.chunk_sizes.append(len(request.data))
                data.extend(request.data)
        self.puts += 1
        if self.fail_put_after is not None and self.puts == self.fail_put_after:
            raise RuntimeError("write failed")
        key = f"{header.key}/{self.puts}"
        self.objects[key] = (1, header.data_type, bytes(data))
        return cache_pb2.CacheObjectMetadata(endpoint=ENDPOINT, key=key, version=1, size=len(data), data_type=header.data_type)

    async def Get(self, request):  # noqa: N802
        if request.key not in self.objects:
            raise _not_found()
        version, data_type, data = self.objects[request.key]
        if request.client_version == version:
            yield cache_pb2.CacheGetResponse(header=cache_pb2.CacheGetHeader(mode=cache_pb2.CACHE_GET_MODE_NOT_MODIFIED, version=version))
            return
        yield cache_pb2.CacheGetResponse(header=cache_pb2.CacheGetHeader(mode=cache_pb2.CACHE_GET_MODE_FULL, version=version, data_type=data_type))
        for start in range(0, len(data), cache_module._UPLOAD_CHUNK_SIZE):
            chunk = cache_pb2.CacheGetChunk(kind=cache_pb2.CACHE_CHUNK_KIND_BASE, version=version, data=data[start : start + cache_module._UPLOAD_CHUNK_SIZE])
            yield cache_pb2.CacheGetResponse(chunk=chunk)

    async def GetMetadata(self, request):  # noqa: N802
        if request.key not in self.objects:
            raise _not_found()
        version, data_type, data = self.objects[request.key]
        return cache_pb2.CacheObjectMetadata(endpoint=ENDPOINT, key=request.key, version=version, size=len(data), data_type=data_type)

    async def Delete(self, request):  # noqa: N802
        for key in [key for key in self.objects if key == request.key or key.startswith(request.key + "/")]:
            del self.objects[key]
        return cache_pb2.CacheDeleteResponse()


class _NotFound(grpc.aio.AioRpcError):
    def __init__(self):
        super().__init__(grpc.StatusCode.NOT_FOUND, grpc.aio.Metadata(), grpc.aio.Metadata(), "not found")


def _not_found():
    return _NotFound()


@pytest.fixture
def fake(monkeypatch):
    fake = FakeCache()
    dedicated = []

    @contextlib.asynccontextmanager
    async def dedicated_client(endpoint, tls_config):
        dedicated.append(endpoint)
        yield fake

    monkeypatch.setattr(cache_module, "_get_cached_context", lambda: SimpleNamespace(cache=None))
    monkeypatch.setattr(aio_cache, "_configured_endpoint", lambda: (ENDPOINT, None))
    monkeypatch.setattr(aio_cache, "_get_client", lambda endpoint, tls_config=None: fake)
    monkeypatch.setattr(aio_cache, "_dedicated_client", dedicated_client)
    fake.dedicated = dedicated
    with _cache_lock:
        _object_cache.clear()
    yield fake
    with _cache_lock:
        _object_cache.clear()


def _payload(size):
    return bytes(i % 251 for i in range(size))


class TestManifest:
    def test_part_ranges(self):
        assert cache_module._part_ranges(10, 4) == [(0, 4), (4, 8), (8, 10)]
        assert cache_module._part_ranges(8, 4) == [(0, 4), (4, 8)]

    def test_encode_decode_roundtrip(self):
        parts = [(ObjectRef("grpc://a:1", "app/s/1", 1), 4), (ObjectRef("grpc://b:2", "app/s/2", 3), 2)]
        data_type, size, decoded = cache_module._decode_manifest(cache_module._encode_manifest("raw", 6, parts))
        assert (data_type, size) == ("raw", 6)
        assert [(ref.endpoint, ref.key, ref.version, part_size) for ref, part_size in decoded] == [("grpc://a:1", "app/s/1", 1, 4), ("grpc://b:2", "app/s/2", 3, 2)]

    def test_rejects_malformed_and_inconsistent_manifests(self):
        with pytest.raises(ValueError, match="Invalid multipart manifest"):
            cache_module._decode_manifest(b"not bson")
        with pytest.raises(ValueError, match="do not add up"):
            cache_module._decode_manifest(cache_module._encode_manifest("raw", 7, [(ObjectRef("grpc://a:1", "app/s/1", 1), 4)]))


class TestMultipartRoundTrip:
    @pytest.mark.asyncio
    async def test_small_object_is_one_ordinary_object(self, fake):
        ref = await aio_cache.put_object("app/session", b"x" * 10, part_size=100)
        assert fake.objects[ref.key][1] != cache_module._TYPE_MULTIPART
        assert len(fake.objects) == 1

    @pytest.mark.asyncio
    async def test_large_object_is_split_and_reassembled(self, fake):
        data = _payload(10_000)
        _, encoded = cache_module._encode_object_data(data)
        ref = await aio_cache.put_object("app/session", data, part_size=3000, parallelism=2)

        assert fake.objects[ref.key][1] == cache_module._TYPE_MULTIPART
        parts = [value for key, value in fake.objects.items() if key != ref.key]
        assert len(parts) == -(-len(encoded) // 3000)
        assert all(data_type == "raw" for _, data_type, _ in parts)
        assert b"".join(part for _, _, part in parts) == encoded
        assert len(fake.dedicated) == len(parts)  # every part on its own channel

        assert await aio_cache.get_object(ref) == data

    @pytest.mark.asyncio
    async def test_only_the_assembled_object_enters_the_client_cache(self, fake):
        ref = await aio_cache.put_object("app/session", _payload(10_000), part_size=3000)
        await aio_cache.get_object(ref)
        with _cache_lock:
            assert list(_object_cache) == [(ref.endpoint, ref.key)]

    @pytest.mark.asyncio
    async def test_second_get_is_served_from_client_cache(self, fake):
        data = _payload(10_000)
        ref = await aio_cache.put_object("app/session", data, part_size=3000)
        await aio_cache.get_object(ref)
        before = len(fake.dedicated)
        assert await aio_cache.get_object(ref) == data
        assert len(fake.dedicated) == before

    @pytest.mark.asyncio
    async def test_deleting_by_manifest_key_deletes_parts(self, fake):
        ref = await aio_cache.put_object("app/session", _payload(10_000), part_size=3000)
        assert len(fake.objects) > 1
        await aio_cache.delete_objects(ref.key)
        assert fake.objects == {}


class TestMultipartVerification:
    async def _put(self, fake):
        ref = await aio_cache.put_object("app/session", _payload(10_000), part_size=3000)
        manifest = fake.objects[ref.key]
        _, _, entries = cache_module._decode_manifest(manifest[2])
        return ref, entries

    @pytest.mark.asyncio
    async def test_part_size_mismatch_fails(self, fake):
        ref, entries = await self._put(fake)
        key = entries[1][0].key
        version, data_type, data = fake.objects[key]
        fake.objects[key] = (version, data_type, data[:-1])
        with pytest.raises(ValueError, match="manifest says"):
            await aio_cache.get_object(ref)
        with _cache_lock:
            assert not _object_cache

    @pytest.mark.asyncio
    async def test_oversized_part_fails(self, fake):
        ref, entries = await self._put(fake)
        key = entries[0][0].key
        version, data_type, data = fake.objects[key]
        fake.objects[key] = (version, data_type, data + b"x")
        with pytest.raises(ValueError, match="larger than"):
            await aio_cache.get_object(ref)

    @pytest.mark.asyncio
    async def test_part_version_mismatch_fails(self, fake):
        ref, entries = await self._put(fake)
        key = entries[0][0].key
        _, data_type, data = fake.objects[key]
        fake.objects[key] = (2, data_type, data)
        with pytest.raises(ValueError, match="Multipart part changed"):
            await aio_cache.get_object(ref)

    @pytest.mark.asyncio
    async def test_missing_part_fails(self, fake):
        ref, entries = await self._put(fake)
        del fake.objects[entries[2][0].key]
        with pytest.raises(grpc.aio.AioRpcError):
            await aio_cache.get_object(ref)


class TestMultipartMutationAndOptions:
    @pytest.mark.asyncio
    async def test_patch_and_update_are_rejected(self, fake):
        ref = await aio_cache.put_object("app/session", _payload(10_000), part_size=3000)
        with pytest.raises(ValueError, match="multipart"):
            await aio_cache.patch_object(ref, b"delta")
        with pytest.raises(ValueError, match="multipart"):
            await aio_cache.update_object(ref, b"new")

    @pytest.mark.asyncio
    @pytest.mark.parametrize("options", [{"part_size": 0}, {"part_size": -1}, {"parallelism": 0}, {"parallelism": -2}])
    async def test_invalid_options_raise(self, fake, options):
        with pytest.raises(ValueError):
            await aio_cache.put_object("app/session", b"data", **options)
        assert fake.objects == {}

    @pytest.mark.asyncio
    async def test_failed_part_write_deletes_written_parts(self, fake):
        fake.fail_put_after = 3
        with pytest.raises(RuntimeError, match="write failed"):
            await aio_cache.put_object("app/session", _payload(10_000), part_size=3000, parallelism=1)
        assert fake.objects == {}

    @pytest.mark.asyncio
    async def test_failed_manifest_write_deletes_parts(self, fake):
        _, encoded = cache_module._encode_object_data(_payload(10_000))
        fake.fail_put_after = -(-len(encoded) // 3000) + 1
        with pytest.raises(RuntimeError, match="write failed"):
            await aio_cache.put_object("app/session", _payload(10_000), part_size=3000)
        assert fake.objects == {}


class TestChunking:
    def test_chunk_size_is_four_mib(self):
        assert cache_module._UPLOAD_CHUNK_SIZE == 4 * 1024 * 1024
        options = dict(cache_module.GRPC_OPTIONS)
        assert options["grpc.http2.lookahead_bytes"] == 16 * 1024 * 1024
        assert options["grpc.http2.write_buffer_size"] == 16 * 1024 * 1024

    @pytest.mark.asyncio
    async def test_single_object_uses_chunks_up_to_limit(self, fake):
        data = _payload(cache_module._UPLOAD_CHUNK_SIZE + 5)
        await aio_cache.put_object("app/session", data)
        assert fake.chunk_sizes
        assert max(fake.chunk_sizes) == cache_module._UPLOAD_CHUNK_SIZE
