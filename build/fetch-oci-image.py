#!/usr/bin/env python3
import sys
"""Offline-safe OCI image reference parsing primitives.

Network acquisition requires explicit CLI invocation.
"""

from dataclasses import dataclass
import re


class ImageReferenceError(ValueError):
    """An image reference is malformed or unsupported."""


@dataclass(frozen=True)
class ImageReference:
    registry: str
    repository: str
    tag: str
    digest: str | None = None


_COMPONENT = re.compile(
    r"[a-z0-9]+(?:(?:[._]|__|-+)[a-z0-9]+)*\Z"
)

_TAG = re.compile(r"[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}\Z")


@dataclass(frozen=True)
class DockerHubTokenRequest:
    endpoint: str
    parameters: tuple[tuple[str, str], ...]


def build_docker_hub_token_request(
    reference: ImageReference,
) -> DockerHubTokenRequest:
    """Construct a read-only Docker Hub token request without network access."""

    if not isinstance(reference, ImageReference):
        raise ImageReferenceError("Invalid image reference")

    if reference.registry != "registry-1.docker.io":
        raise ImageReferenceError("Unsupported registry")

    if reference.digest is not None:
        reference_value = reference.repository + "@" + reference.digest
    else:
        reference_value = reference.repository + ":" + reference.tag

    canonical = parse_image_reference(reference_value)

    if canonical.repository != reference.repository:
        raise ImageReferenceError("Noncanonical repository")

    return DockerHubTokenRequest(
        endpoint="https://auth.docker.io/token",
        parameters=(
            ("service", "registry.docker.io"),
            ("scope", "repository:" + reference.repository + ":pull"),
        ),
    )


class DockerHubTokenError(ValueError):
    """Invalid Docker Hub authentication response."""


def validate_docker_hub_token_response(payload: object) -> str:
    """Validate a decoded Docker Hub token response without HTTP access."""

    if not isinstance(payload, dict):
        raise DockerHubTokenError("Token response must be a JSON object")

    token = payload.get("token")

    if not isinstance(token, str):
        raise DockerHubTokenError("Missing or invalid bearer token")

    if not token or len(token) > 16384:
        raise DockerHubTokenError("Invalid bearer token length")

    if not token.isascii() or any(
        character.isspace() or ord(character) < 33 or ord(character) > 126
        for character in token
    ):
        raise DockerHubTokenError("Invalid bearer token characters")

    if payload.get("access_token") is not None:
        access_token = payload["access_token"]
        if not isinstance(access_token, str) or access_token != token:
            raise DockerHubTokenError("Conflicting authentication tokens")

    return token


class OCIManifestError(ValueError):
    """Malformed or unsupported OCI manifest metadata."""


SUPPORTED_DESCRIPTOR_MEDIA_TYPES = frozenset({
    "application/vnd.oci.image.index.v1+json",
    "application/vnd.oci.image.manifest.v1+json",
    "application/vnd.oci.image.config.v1+json",
    "application/vnd.oci.image.layer.v1.tar+gzip",
    "application/vnd.docker.distribution.manifest.list.v2+json",
    "application/vnd.docker.distribution.manifest.v2+json",
    "application/vnd.docker.container.image.v1+json",
    "application/vnd.docker.image.rootfs.diff.tar.gzip",
})


@dataclass(frozen=True)
class OCIDescriptor:
    media_type: str
    digest: str
    size: int


def validate_oci_descriptor(payload: object) -> OCIDescriptor:
    """Validate an OCI descriptor without fetching referenced content."""

    if not isinstance(payload, dict):
        raise OCIManifestError("Descriptor must be a JSON object")

    media_type = payload.get("mediaType")
    digest = payload.get("digest")
    size = payload.get("size")

    if (
        not isinstance(media_type, str)
        or media_type not in SUPPORTED_DESCRIPTOR_MEDIA_TYPES
    ):
        raise OCIManifestError("Unsupported descriptor media type")

    if (
        not isinstance(digest, str)
        or re.fullmatch(r"sha256:[0-9a-f]{64}", digest) is None
    ):
        raise OCIManifestError("Invalid SHA-256 descriptor digest")

    if type(size) is not int or size < 0 or size > (1 << 63) - 1:
        raise OCIManifestError("Invalid descriptor size")

    return OCIDescriptor(
        media_type=media_type,
        digest=digest,
        size=size,
    )


INDEX_MEDIA_TYPES = frozenset({
    "application/vnd.oci.image.index.v1+json",
    "application/vnd.docker.distribution.manifest.list.v2+json",
})

IMAGE_MANIFEST_MEDIA_TYPES = frozenset({
    "application/vnd.oci.image.manifest.v1+json",
    "application/vnd.docker.distribution.manifest.v2+json",
})


def select_linux_amd64_manifest(payload: object) -> OCIDescriptor:
    """Select one unambiguous linux/amd64 manifest from an image index."""

    if not isinstance(payload, dict):
        raise OCIManifestError("Image index must be a JSON object")

    if payload.get("schemaVersion") != 2 or type(
        payload.get("schemaVersion")
    ) is not int:
        raise OCIManifestError("Unsupported image index schema")

    if payload.get("mediaType") not in INDEX_MEDIA_TYPES:
        raise OCIManifestError("Unsupported image index media type")

    manifests = payload.get("manifests")

    if not isinstance(manifests, list) or not manifests:
        raise OCIManifestError("Image index has no manifests")

    selected = []
    runtime_digests = set()
    attestations = []

    for entry in manifests:
        descriptor = validate_oci_descriptor(entry)

        if descriptor.media_type not in IMAGE_MANIFEST_MEDIA_TYPES:
            raise OCIManifestError("Unsupported indexed manifest type")

        platform = entry.get("platform")

        if not isinstance(platform, dict):
            raise OCIManifestError("Missing manifest platform")

        os_name = platform.get("os")
        architecture = platform.get("architecture")

        if not isinstance(os_name, str) or not isinstance(
            architecture, str
        ):
            raise OCIManifestError("Invalid manifest platform")

        if os_name == "unknown" or architecture == "unknown":
            if os_name != "unknown" or architecture != "unknown":
                raise OCIManifestError("Invalid attestation platform")

            if set(platform) != {"os", "architecture"}:
                raise OCIManifestError("Unexpected attestation platform fields")

            annotations = entry.get("annotations")
            if not isinstance(annotations, dict):
                raise OCIManifestError("Missing attestation annotations")

            if annotations.get(
                "vnd.docker.reference.type"
            ) != "attestation-manifest":
                raise OCIManifestError("Unsupported unknown-platform manifest")

            reference = annotations.get("vnd.docker.reference.digest")
            if (
                not isinstance(reference, str)
                or len(reference) != 71
                or not reference.startswith("sha256:")
                or any(char not in "0123456789abcdef" for char in reference[7:])
            ):
                raise OCIManifestError("Invalid attestation reference")

            attestations.append(reference)
            continue

        runtime_digests.add(descriptor.digest)

        if os_name == "linux" and architecture == "amd64":
            if platform.get("variant") not in (None, ""):
                raise OCIManifestError("Unsupported amd64 variant")

            if platform.get("os.version") not in (None, ""):
                raise OCIManifestError("Unsupported OS version constraint")

            if platform.get("os.features") not in (None, []):
                raise OCIManifestError("Unsupported OS feature constraints")

            if platform.get("features") not in (None, []):
                raise OCIManifestError("Unsupported CPU feature constraints")

            selected.append(descriptor)

    if any(reference not in runtime_digests for reference in attestations):
        raise OCIManifestError("Attestation references unknown image")

    if len(selected) != 1:
        raise OCIManifestError(
            "Expected exactly one linux/amd64 image manifest"
        )

    return selected[0]


CONFIG_MEDIA_TYPES = frozenset({
    "application/vnd.oci.image.config.v1+json",
    "application/vnd.docker.container.image.v1+json",
})

LAYER_MEDIA_TYPES = frozenset({
    "application/vnd.oci.image.layer.v1.tar+gzip",
    "application/vnd.docker.image.rootfs.diff.tar.gzip",
})


@dataclass(frozen=True)
class OCIImageManifest:
    config: OCIDescriptor
    layers: tuple[OCIDescriptor, ...]
    total_blob_bytes: int


def validate_oci_image_manifest(payload: object) -> OCIImageManifest:
    """Validate a gzip-layer image manifest without downloading blobs."""

    if not isinstance(payload, dict):
        raise OCIManifestError("Image manifest must be a JSON object")

    schema = payload.get("schemaVersion")
    if type(schema) is not int or schema != 2:
        raise OCIManifestError("Unsupported image manifest schema")

    if payload.get("mediaType") not in IMAGE_MANIFEST_MEDIA_TYPES:
        raise OCIManifestError("Unsupported image manifest media type")

    config = validate_oci_descriptor(payload.get("config"))

    if config.media_type not in CONFIG_MEDIA_TYPES:
        raise OCIManifestError("Unsupported image configuration type")

    raw_layers = payload.get("layers")

    if not isinstance(raw_layers, list) or not raw_layers:
        raise OCIManifestError("Image manifest has no layers")

    layers = []
    total = config.size

    for raw_layer in raw_layers:
        layer = validate_oci_descriptor(raw_layer)

        if layer.media_type not in LAYER_MEDIA_TYPES:
            raise OCIManifestError("Unsupported image layer type")

        if total > (1 << 63) - 1 - layer.size:
            raise OCIManifestError("Aggregate image size overflow")

        total += layer.size
        layers.append(layer)

    return OCIImageManifest(
        config=config,
        layers=tuple(layers),
        total_blob_bytes=total,
    )


@dataclass(frozen=True)
class OCIImageConfig:
    architecture: str
    os: str
    diff_ids: tuple[str, ...]


def validate_oci_image_config(
    payload: object,
    expected_layer_count: int,
) -> OCIImageConfig:
    """Validate linux/amd64 image configuration and rootfs metadata."""

    if not isinstance(payload, dict):
        raise OCIManifestError("Image configuration must be a JSON object")

    if type(expected_layer_count) is not int or expected_layer_count < 1:
        raise OCIManifestError("Invalid expected layer count")

    architecture = payload.get("architecture")
    os_name = payload.get("os")

    if architecture != "amd64" or os_name != "linux":
        raise OCIManifestError("Unsupported image configuration platform")

    rootfs = payload.get("rootfs")

    if not isinstance(rootfs, dict):
        raise OCIManifestError("Missing image rootfs metadata")

    if rootfs.get("type") != "layers":
        raise OCIManifestError("Unsupported rootfs type")

    diff_ids = rootfs.get("diff_ids")

    if not isinstance(diff_ids, list):
        raise OCIManifestError("Invalid rootfs diff IDs")

    if len(diff_ids) != expected_layer_count:
        raise OCIManifestError("Rootfs layer count mismatch")

    for digest in diff_ids:
        if (
            not isinstance(digest, str)
            or re.fullmatch(r"sha256:[0-9a-f]{64}", digest) is None
        ):
            raise OCIManifestError("Invalid rootfs diff ID")

    return OCIImageConfig(
        architecture=architecture,
        os=os_name,
        diff_ids=tuple(diff_ids),
    )


import hashlib


class OCIBlobIntegrityError(ValueError):
    """Blob contents do not match their expected descriptor."""


def verify_oci_blob_stream(
    reader,
    expected_digest: str,
    expected_size: int,
    chunk_size: int = 1024 * 1024,
) -> int:
    """Verify a binary blob stream using bounded reads and SHA-256."""

    if (
        not isinstance(expected_digest, str)
        or re.fullmatch(r"sha256:[0-9a-f]{64}", expected_digest) is None
    ):
        raise OCIBlobIntegrityError("Invalid expected blob digest")

    if (
        type(expected_size) is not int
        or expected_size < 0
        or expected_size > (1 << 63) - 1
    ):
        raise OCIBlobIntegrityError("Invalid expected blob size")

    if type(chunk_size) is not int or not 1 <= chunk_size <= 1024 * 1024:
        raise OCIBlobIntegrityError("Invalid verification chunk size")

    digest = hashlib.sha256()
    total = 0

    while True:
        chunk = reader.read(chunk_size)

        if not isinstance(chunk, bytes):
            raise OCIBlobIntegrityError("Blob reader returned non-byte data")

        if not chunk:
            break

        if len(chunk) > chunk_size:
            raise OCIBlobIntegrityError("Blob reader exceeded chunk size")

        if total > expected_size - len(chunk):
            raise OCIBlobIntegrityError("Blob exceeds expected size")

        digest.update(chunk)
        total += len(chunk)

    if total != expected_size:
        raise OCIBlobIntegrityError("Blob size mismatch")

    if "sha256:" + digest.hexdigest() != expected_digest:
        raise OCIBlobIntegrityError("Blob SHA-256 mismatch")

    return total


def verify_oci_manifest_bytes(
    manifest: bytes,
    expected_digest: str,
    expected_size: int,
    response_digest: str | None = None,
) -> int:
    """Verify raw manifest bytes against a trusted SHA-256 digest."""

    if not isinstance(manifest, bytes):
        raise OCIBlobIntegrityError("Manifest must contain bytes")

    if response_digest is not None:
        if (
            not isinstance(response_digest, str)
            or re.fullmatch(r"sha256:[0-9a-f]{64}", response_digest) is None
            or response_digest != expected_digest
        ):
            raise OCIBlobIntegrityError(
                "Registry manifest digest header mismatch"
            )

    import io

    return verify_oci_blob_stream(
        io.BytesIO(manifest),
        expected_digest,
        expected_size,
    )


MAX_OCI_MANIFEST_BYTES = 16 * 1024 * 1024


def read_verified_oci_manifest(
    reader,
    expected_digest: str,
    expected_size: int,
    response_digest: str | None = None,
) -> bytes:
    """Read a bounded manifest and verify its trusted SHA-256 digest."""

    if type(expected_size) is not int or not (
        0 <= expected_size <= MAX_OCI_MANIFEST_BYTES
    ):
        raise OCIBlobIntegrityError("Invalid manifest size")

    if (
        not isinstance(expected_digest, str)
        or re.fullmatch(r"sha256:[0-9a-f]{64}", expected_digest) is None
    ):
        raise OCIBlobIntegrityError("Invalid expected manifest digest")

    if response_digest is not None and (
        not isinstance(response_digest, str)
        or response_digest != expected_digest
    ):
        raise OCIBlobIntegrityError(
            "Registry manifest digest header mismatch"
        )

    chunks = []
    remaining = expected_size

    while remaining:
        chunk = reader.read(min(remaining, 65536))

        if not isinstance(chunk, bytes):
            raise OCIBlobIntegrityError(
                "Manifest reader returned non-byte data"
            )

        if not chunk:
            raise OCIBlobIntegrityError("Truncated manifest response")

        if len(chunk) > min(remaining, 65536):
            raise OCIBlobIntegrityError(
                "Manifest reader exceeded requested size"
            )

        chunks.append(chunk)
        remaining -= len(chunk)

    extra = reader.read(1)

    if not isinstance(extra, bytes):
        raise OCIBlobIntegrityError(
            "Manifest reader returned non-byte data"
        )

    if extra:
        raise OCIBlobIntegrityError(
            "Manifest response exceeds expected size"
        )

    manifest = b"".join(chunks)

    verify_oci_manifest_bytes(
        manifest,
        expected_digest,
        expected_size,
        response_digest=response_digest,
    )

    return manifest


import zlib




def assemble_verified_oci_archive(
    *,
    manifest,
    config_bytes,
    compressed_layers,
    runtime_reference,
    destination,
    max_uncompressed_layer_bytes=1 << 32,
):
    """Assemble a verified OCI/Docker archive without overwriting output."""

    import io
    import json
    import tarfile
    import tempfile
    from pathlib import Path

    image = validate_oci_image_manifest(manifest)

    if not isinstance(config_bytes, bytes):
        raise OCIManifestError("Image configuration must be bytes")

    if len(config_bytes) != image.config.size:
        raise OCIManifestError("Image configuration size mismatch")

    if (
        "sha256:" + hashlib.sha256(config_bytes).hexdigest()
        != image.config.digest
    ):
        raise OCIManifestError("Image configuration digest mismatch")

    try:
        config_payload = json.loads(config_bytes)
    except (ValueError, UnicodeDecodeError) as error:
        raise OCIManifestError("Invalid image configuration JSON") from error

    config = validate_oci_image_config(
        config_payload, len(image.layers)
    )

    if runtime_reference != "pgvector/pgvector:pg17":
        raise OCIManifestError("Unsupported runtime reference")

    if (
        type(max_uncompressed_layer_bytes) is not int
        or not 0 <= max_uncompressed_layer_bytes <= (1 << 63) - 1
    ):
        raise OCIManifestError("Invalid decompression limit")

    destination = Path(destination)

    if destination.name != "pgvector.tar":
        raise OCIStagingError("Unexpected archive destination")

    parent = str(destination.parent)
    directory_fd = open_oci_staging_directory(parent)

    def encoded(value):
        return json.dumps(
            value, sort_keys=True, separators=(",", ":")
        ).encode("utf-8")

    def add_bytes(archive, name, data):
        member = tarfile.TarInfo(name)
        member.size = len(data)
        member.mode = 0o644
        member.mtime = 0
        member.uid = 0
        member.gid = 0
        archive.addfile(member, io.BytesIO(data))

    def add_file(archive, name, source_file):
        source_file.seek(0, os.SEEK_END)
        size = source_file.tell()
        source_file.seek(0)

        member = tarfile.TarInfo(name)
        member.size = size
        member.mode = 0o644
        member.mtime = 0
        member.uid = 0
        member.gid = 0
        archive.addfile(member, source_file)

    temporary_name = ".oci-archive-" + secrets.token_hex(16)
    created = False

    try:
        if (
            len(compressed_layers) != len(image.layers)
        ):
            raise OCIManifestError("Compressed layer count mismatch")

        flags = (
            os.O_WRONLY | os.O_CREAT | os.O_EXCL
            | os.O_NOFOLLOW | os.O_CLOEXEC
        )

        archive_fd = os.open(
            temporary_name, flags, 0o600, dir_fd=directory_fd
        )
        created = True

        try:
            with os.fdopen(archive_fd, "wb") as output:
                with tarfile.open(fileobj=output, mode="w|") as archive:
                    converted_layers = []
                    docker_layers = []
                    published_layer_sizes = {}

                    for index, (descriptor, reader) in enumerate(
                        zip(image.layers, compressed_layers)
                    ):
                        layer_fd = os.open(
                            ".",
                            os.O_RDWR | os.O_TMPFILE | os.O_CLOEXEC,
                            0o600,
                            dir_fd=directory_fd,
                        )
                        with os.fdopen(layer_fd, "w+b") as layer_file:
                            size = stream_verified_oci_layer_to_tar(
                                reader,
                                layer_file,
                                descriptor.digest,
                                descriptor.size,
                                config.diff_ids[index],
                                max_uncompressed_layer_bytes,
                            )

                            digest = config.diff_ids[index]
                            name = "blobs/sha256/" + digest[7:]

                            previous_size = published_layer_sizes.get(digest)

                            if previous_size is None:
                                add_file(archive, name, layer_file)
                                published_layer_sizes[digest] = size
                            elif previous_size != size:
                                raise OCIBlobIntegrityError(
                                    "Repeated layer digest has inconsistent size"
                                )

                            converted_layers.append({
                                "mediaType":
                                    "application/vnd.oci.image.layer.v1.tar",
                                "digest": digest,
                                "size": size,
                            })
                            docker_layers.append(name)

                    config_name = (
                        "blobs/sha256/" + image.config.digest[7:]
                    )
                    add_bytes(archive, config_name, config_bytes)

                    converted_manifest = encoded({
                        "schemaVersion": 2,
                        "mediaType":
                            "application/vnd.oci.image.manifest.v1+json",
                        "config": {
                            "mediaType":
                                "application/vnd.oci.image.config.v1+json",
                            "digest": image.config.digest,
                            "size": len(config_bytes),
                        },
                        "layers": converted_layers,
                    })

                    converted_digest = (
                        "sha256:"
                        + hashlib.sha256(converted_manifest).hexdigest()
                    )
                    converted_name = (
                        "blobs/sha256/" + converted_digest[7:]
                    )

                    add_bytes(
                        archive, converted_name, converted_manifest
                    )

                    add_bytes(archive, "oci-layout", encoded({
                        "imageLayoutVersion": "1.0.0"
                    }))

                    add_bytes(archive, "index.json", encoded({
                        "schemaVersion": 2,
                        "mediaType":
                            "application/vnd.oci.image.index.v1+json",
                        "manifests": [{
                            "mediaType":
                                "application/vnd.oci.image.manifest.v1+json",
                            "digest": converted_digest,
                            "size": len(converted_manifest),
                            "platform": {
                                "architecture": "amd64",
                                "os": "linux",
                            },
                            "annotations": {
                                "io.containerd.image.name":
                                    runtime_reference,
                                "org.opencontainers.image.ref.name":
                                    "pg17",
                            },
                        }],
                    }))

                    add_bytes(archive, "manifest.json", encoded([{
                        "Config": config_name,
                        "RepoTags": [runtime_reference],
                        "Layers": docker_layers,
                    }]))

                    add_bytes(archive, "repositories", encoded({
                        "pgvector/pgvector": {
                            "pg17": config.diff_ids[-1][7:]
                        }
                    }))

                output.flush()
                os.fsync(output.fileno())

        except BaseException:
            raise

        os.link(
            temporary_name,
            destination.name,
            src_dir_fd=directory_fd,
            dst_dir_fd=directory_fd,
            follow_symlinks=False,
        )

        try:
            os.unlink(temporary_name, dir_fd=directory_fd)
            created = False
            os.fsync(directory_fd)
        except OSError as error:
            raise OCIPublicationDurabilityError(
                str(destination)
            ) from error

        return destination

    finally:
        if created:
            try:
                os.unlink(temporary_name, dir_fd=directory_fd)
            except FileNotFoundError:
                pass

        os.close(directory_fd)



def stream_verified_oci_layer_to_tar(
    reader,
    writer,
    expected_blob_digest: str,
    expected_blob_size: int,
    expected_diff_id: str,
    max_uncompressed_bytes: int,
    chunk_size: int = 65536,
) -> int:
    """Stream one verified gzip layer as uncompressed tar bytes."""

    for value in (expected_blob_digest, expected_diff_id):
        if (
            not isinstance(value, str)
            or re.fullmatch(r"sha256:[0-9a-f]{64}", value) is None
        ):
            raise OCIBlobIntegrityError("Invalid OCI layer digest")

    if (
        type(expected_blob_size) is not int
        or not 0 <= expected_blob_size <= (1 << 63) - 1
    ):
        raise OCIBlobIntegrityError("Invalid compressed layer size")

    if (
        type(max_uncompressed_bytes) is not int
        or not 0 <= max_uncompressed_bytes <= (1 << 63) - 1
    ):
        raise OCIBlobIntegrityError("Invalid uncompressed layer limit")

    if type(chunk_size) is not int or not 1 <= chunk_size <= 1048576:
        raise OCIBlobIntegrityError("Invalid chunk size")

    compressed_hash = hashlib.sha256()
    uncompressed_hash = hashlib.sha256()
    compressed_total = 0
    uncompressed_total = 0
    decompressor = zlib.decompressobj(16 + zlib.MAX_WBITS)

    try:
        while True:
            chunk = reader.read(chunk_size)

            if not isinstance(chunk, bytes) or len(chunk) > chunk_size:
                raise OCIBlobIntegrityError("Invalid layer reader output")

            if not chunk:
                break

            compressed_total += len(chunk)

            if compressed_total > expected_blob_size:
                raise OCIBlobIntegrityError("Compressed layer exceeds size")

            compressed_hash.update(chunk)

            if decompressor.eof:
                raise OCIBlobIntegrityError("Trailing gzip data")

            pending = chunk

            while pending:
                remaining = max_uncompressed_bytes - uncompressed_total
                output = decompressor.decompress(
                    pending,
                    min(remaining + 1, chunk_size),
                )

                if len(output) > remaining:
                    raise OCIBlobIntegrityError(
                        "Uncompressed layer exceeds limit"
                    )

                if output:
                    view = memoryview(output)

                    while view:
                        written = writer.write(view)

                        if (
                            type(written) is not int
                            or written <= 0
                            or written > len(view)
                        ):
                            raise OSError("Layer writer made no progress")

                        view = view[written:]

                    uncompressed_hash.update(output)
                    uncompressed_total += len(output)

                if decompressor.eof:
                    if decompressor.unused_data:
                        raise OCIBlobIntegrityError("Trailing gzip data")
                    break

                pending = decompressor.unconsumed_tail

        if not decompressor.eof:
            raise OCIBlobIntegrityError("Incomplete gzip layer")

    except zlib.error as error:
        raise OCIBlobIntegrityError("Corrupt gzip layer") from error

    if compressed_total != expected_blob_size:
        raise OCIBlobIntegrityError("Compressed layer size mismatch")

    if compressed_hash.hexdigest() != expected_blob_digest[7:]:
        raise OCIBlobIntegrityError("Compressed layer SHA-256 mismatch")

    if uncompressed_hash.hexdigest() != expected_diff_id[7:]:
        raise OCIBlobIntegrityError("Uncompressed layer SHA-256 mismatch")

    return uncompressed_total



def verify_oci_gzip_diff_id(
    reader,
    expected_diff_id: str,
    max_uncompressed_bytes: int,
    chunk_size: int = 65536,
) -> int:
    """Verify a single gzip member against its uncompressed SHA-256."""

    if (
        not isinstance(expected_diff_id, str)
        or re.fullmatch(r"sha256:[0-9a-f]{64}", expected_diff_id) is None
    ):
        raise OCIBlobIntegrityError("Invalid expected diff ID")

    if (
        type(max_uncompressed_bytes) is not int
        or max_uncompressed_bytes < 0
        or max_uncompressed_bytes > (1 << 63) - 1
    ):
        raise OCIBlobIntegrityError("Invalid uncompressed size limit")

    if type(chunk_size) is not int or not 1 <= chunk_size <= 1024 * 1024:
        raise OCIBlobIntegrityError("Invalid gzip chunk size")

    decompressor = zlib.decompressobj(wbits=16 + zlib.MAX_WBITS)
    digest = hashlib.sha256()
    total = 0

    try:
        while True:
            compressed = reader.read(chunk_size)

            if not isinstance(compressed, bytes):
                raise OCIBlobIntegrityError("Gzip reader returned non-byte data")

            if len(compressed) > chunk_size:
                raise OCIBlobIntegrityError("Gzip reader exceeded chunk size")

            if not compressed:
                break

            if decompressor.eof:
                raise OCIBlobIntegrityError("Trailing gzip data")

            pending = compressed

            while pending:
                remaining = max_uncompressed_bytes - total
                output = decompressor.decompress(
                    pending,
                    min(remaining + 1, chunk_size),
                )

                if len(output) > remaining:
                    raise OCIBlobIntegrityError(
                        "Uncompressed layer exceeds size limit"
                    )

                digest.update(output)
                total += len(output)

                if decompressor.eof:
                    if decompressor.unused_data:
                        raise OCIBlobIntegrityError("Trailing gzip data")
                    break

                next_pending = decompressor.unconsumed_tail

                if not next_pending:
                    break

                pending = next_pending

        if not decompressor.eof:
            raise OCIBlobIntegrityError("Incomplete gzip stream")

    except zlib.error as error:
        raise OCIBlobIntegrityError("Corrupt gzip stream") from error

    if "sha256:" + digest.hexdigest() != expected_diff_id:
        raise OCIBlobIntegrityError("Uncompressed layer SHA-256 mismatch")

    return total


def verify_oci_layer_stream(
    reader,
    expected_blob_digest: str,
    expected_blob_size: int,
    expected_diff_id: str,
    max_uncompressed_bytes: int,
    chunk_size: int = 65536,
) -> tuple[int, int]:
    """Verify compressed and uncompressed SHA-256 in one stream pass."""

    for value, label in (
        (expected_blob_digest, "blob digest"),
        (expected_diff_id, "diff ID"),
    ):
        if (
            not isinstance(value, str)
            or re.fullmatch(r"sha256:[0-9a-f]{64}", value) is None
        ):
            raise OCIBlobIntegrityError(f"Invalid expected {label}")

    for value, label in (
        (expected_blob_size, "blob size"),
        (max_uncompressed_bytes, "uncompressed size limit"),
    ):
        if (
            type(value) is not int
            or value < 0
            or value > (1 << 63) - 1
        ):
            raise OCIBlobIntegrityError(f"Invalid {label}")

    if type(chunk_size) is not int or not 1 <= chunk_size <= 1024 * 1024:
        raise OCIBlobIntegrityError("Invalid layer chunk size")

    compressed_digest = hashlib.sha256()
    uncompressed_digest = hashlib.sha256()
    decompressor = zlib.decompressobj(wbits=16 + zlib.MAX_WBITS)

    compressed_total = 0
    uncompressed_total = 0

    try:
        while True:
            chunk = reader.read(chunk_size)

            if not isinstance(chunk, bytes):
                raise OCIBlobIntegrityError("Layer reader returned non-byte data")

            if len(chunk) > chunk_size:
                raise OCIBlobIntegrityError("Layer reader exceeded chunk size")

            if not chunk:
                break

            if compressed_total > expected_blob_size - len(chunk):
                raise OCIBlobIntegrityError("Compressed layer exceeds expected size")

            compressed_digest.update(chunk)
            compressed_total += len(chunk)

            if decompressor.eof:
                raise OCIBlobIntegrityError("Trailing gzip data")

            pending = chunk

            while pending:
                remaining = max_uncompressed_bytes - uncompressed_total

                output = decompressor.decompress(
                    pending,
                    min(remaining + 1, chunk_size),
                )

                if len(output) > remaining:
                    raise OCIBlobIntegrityError(
                        "Uncompressed layer exceeds size limit"
                    )

                uncompressed_digest.update(output)
                uncompressed_total += len(output)

                if decompressor.eof:
                    if decompressor.unused_data:
                        raise OCIBlobIntegrityError("Trailing gzip data")
                    break

                next_pending = decompressor.unconsumed_tail

                if not next_pending:
                    break

                pending = next_pending

        if not decompressor.eof:
            raise OCIBlobIntegrityError("Incomplete gzip stream")

    except zlib.error as error:
        raise OCIBlobIntegrityError("Corrupt gzip stream") from error

    if compressed_total != expected_blob_size:
        raise OCIBlobIntegrityError("Compressed layer size mismatch")

    if "sha256:" + compressed_digest.hexdigest() != expected_blob_digest:
        raise OCIBlobIntegrityError("Compressed layer SHA-256 mismatch")

    if "sha256:" + uncompressed_digest.hexdigest() != expected_diff_id:
        raise OCIBlobIntegrityError("Uncompressed layer SHA-256 mismatch")

    return compressed_total, uncompressed_total


import os
import stat


class OCIStagingError(ValueError):
    """Unsafe OCI staging filesystem boundary."""


def validate_oci_staging_directory(path: str) -> None:
    """Validate an existing private staging directory without modifying it."""

    if not isinstance(path, str) or not path.startswith("/"):
        raise OCIStagingError("Staging path must be absolute")

    if path == "/" or os.path.normpath(path) != path:
        raise OCIStagingError("Staging path must be normalized and non-root")

    components = path.split("/")[1:]

    if not components or any(
        component in ("", ".", "..") for component in components
    ):
        raise OCIStagingError("Invalid staging path components")

    flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
    directory_fd = os.open("/", flags)

    try:
        for component in components:
            next_fd = os.open(component, flags, dir_fd=directory_fd)
            os.close(directory_fd)
            directory_fd = next_fd

        metadata = os.fstat(directory_fd)

        if not stat.S_ISDIR(metadata.st_mode):
            raise OCIStagingError("Staging path is not a directory")

        if metadata.st_uid != os.geteuid():
            raise OCIStagingError("Staging directory ownership mismatch")

        if stat.S_IMODE(metadata.st_mode) != 0o700:
            raise OCIStagingError("Staging directory must have mode 0700")

    except OSError as error:
        raise OCIStagingError("Unsafe staging directory path") from error

    finally:
        os.close(directory_fd)


def open_oci_staging_directory(path: str) -> int:
    """Open a private staging directory; caller owns returned FD."""

    if not isinstance(path, str) or not path.startswith("/"):
        raise OCIStagingError("Staging path must be absolute")

    if path == "/" or os.path.normpath(path) != path:
        raise OCIStagingError("Staging path must be normalized and non-root")

    components = path.split("/")[1:]

    if not components or any(
        component in ("", ".", "..") for component in components
    ):
        raise OCIStagingError("Invalid staging path components")

    flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
    directory_fd = -1

    try:
        directory_fd = os.open("/", flags)

        for component in components:
            metadata = os.fstat(directory_fd)

            if (
                not stat.S_ISDIR(metadata.st_mode)
                or metadata.st_mode & 0o022
            ):
                raise OCIStagingError("Unsafe staging ancestor")

            next_fd = os.open(component, flags, dir_fd=directory_fd)
            os.close(directory_fd)
            directory_fd = next_fd

        metadata = os.fstat(directory_fd)

        if not stat.S_ISDIR(metadata.st_mode):
            raise OCIStagingError("Staging path is not a directory")

        if metadata.st_uid != os.geteuid():
            raise OCIStagingError("Staging directory ownership mismatch")

        if stat.S_IMODE(metadata.st_mode) != 0o700:
            raise OCIStagingError("Staging directory must have mode 0700")

        result = directory_fd
        directory_fd = -1
        return result

    except OSError as error:
        raise OCIStagingError("Unsafe staging directory path") from error

    finally:
        if directory_fd >= 0:
            os.close(directory_fd)


import secrets


class OCIPublicationDurabilityError(OSError):
    """Blob was published, but directory durability was not confirmed."""

    def __init__(self, destination: str):
        self.destination = destination
        super().__init__(
            "OCI blob published but durability not confirmed: "
            + destination
        )


class _VerifiedLayerReader:
    """Forward verified layer bytes into a private staging file."""

    def __init__(self, reader, file_fd):
        self.reader = reader
        self.file_fd = file_fd

    def read(self, size):
        data = self.reader.read(size)

        if not isinstance(data, bytes):
            return data

        if len(data) > size:
            return data

        view = memoryview(data)

        while view:
            written = os.write(self.file_fd, view)

            if written <= 0:
                raise OSError("Layer staging write made no progress")

            view = view[written:]

        return data


def publish_verified_oci_layer(
    directory_fd: int,
    reader,
    expected_blob_digest: str,
    expected_blob_size: int,
    expected_diff_id: str,
    max_uncompressed_bytes: int,
    chunk_size: int = 65536,
) -> str:
    """Verify a gzip layer and publish its unnamed staging inode."""

    directory = os.fstat(directory_fd)

    if (
        not stat.S_ISDIR(directory.st_mode)
        or directory.st_uid != os.geteuid()
        or stat.S_IMODE(directory.st_mode) != 0o700
    ):
        raise OCIStagingError("Unsafe staging directory descriptor")

    flags = os.O_TMPFILE | os.O_WRONLY | os.O_CLOEXEC

    file_fd = os.open(
        ".",
        flags,
        0o600,
        dir_fd=directory_fd,
    )

    try:
        forwarding_reader = _VerifiedLayerReader(reader, file_fd)

        verify_oci_layer_stream(
            forwarding_reader,
            expected_blob_digest,
            expected_blob_size,
            expected_diff_id,
            max_uncompressed_bytes,
            chunk_size,
        )

        os.fsync(file_fd)

        destination = "sha256-" + expected_blob_digest[7:]

        publish_unnamed_oci_file(
            file_fd,
            directory_fd,
            destination,
        )

        try:
            os.fsync(directory_fd)
        except OSError as error:
            raise OCIPublicationDurabilityError(
                destination
            ) from error

        return destination

    finally:
        os.close(file_fd)


def publish_verified_oci_blob(
    directory_fd: int,
    reader,
    expected_digest: str,
    expected_size: int,
    chunk_size: int = 65536,
) -> str:
    """Verify and atomically publish a blob without replacing existing data."""

    if (
        not isinstance(expected_digest, str)
        or re.fullmatch(r"sha256:[0-9a-f]{64}", expected_digest) is None
    ):
        raise OCIBlobIntegrityError("Invalid blob digest")

    if (
        type(expected_size) is not int
        or expected_size < 0
        or expected_size > (1 << 63) - 1
    ):
        raise OCIBlobIntegrityError("Invalid blob size")

    if (
        type(chunk_size) is not int
        or chunk_size < 1
        or chunk_size > 1048576
    ):
        raise OCIBlobIntegrityError("Invalid chunk size")

    directory = os.fstat(directory_fd)

    if (
        not stat.S_ISDIR(directory.st_mode)
        or directory.st_uid != os.geteuid()
        or stat.S_IMODE(directory.st_mode) != 0o700
    ):
        raise OCIStagingError("Unsafe staging directory descriptor")

    destination = "sha256-" + expected_digest[7:]
    temporary = ".oci-blob-" + secrets.token_hex(16)

    flags = (
        os.O_WRONLY | os.O_CREAT | os.O_EXCL
        | os.O_NOFOLLOW | os.O_CLOEXEC
    )

    temporary_created = False
    published = False

    try:
        file_fd = os.open(
            temporary, flags, 0o600, dir_fd=directory_fd
        )
        temporary_created = True

        try:
            digest = hashlib.sha256()
            copied = 0

            while True:
                data = reader.read(chunk_size)

                if not isinstance(data, bytes):
                    raise OCIBlobIntegrityError("Reader returned non-bytes")

                if len(data) > chunk_size:
                    raise OCIBlobIntegrityError("Reader exceeded chunk size")

                if not data:
                    break

                copied += len(data)

                if copied > expected_size:
                    raise OCIBlobIntegrityError("Blob exceeds expected size")

                digest.update(data)
                view = memoryview(data)

                while view:
                    count = os.write(file_fd, view)

                    if count <= 0:
                        raise OSError("Blob write made no progress")

                    view = view[count:]

            if copied != expected_size:
                raise OCIBlobIntegrityError("Blob size mismatch")

            if digest.hexdigest() != expected_digest[7:]:
                raise OCIBlobIntegrityError("Blob SHA-256 mismatch")

            os.fsync(file_fd)

        finally:
            os.close(file_fd)

        os.link(
            temporary,
            destination,
            src_dir_fd=directory_fd,
            dst_dir_fd=directory_fd,
            follow_symlinks=False,
        )
        published = True

        try:
            os.unlink(temporary, dir_fd=directory_fd)
            temporary_created = False
            os.fsync(directory_fd)
        except OSError as error:
            raise OCIPublicationDurabilityError(
                destination
            ) from error

        return destination

    finally:
        if temporary_created:
            try:
                os.unlink(temporary, dir_fd=directory_fd)
            except FileNotFoundError:
                pass



def open_verified_oci_staged_blob(
    directory_fd: int,
    expected_digest: str,
):
    """Open a private staged blob; contents require independent verification."""
    digest = validate_trusted_oci_digest(expected_digest)

    directory = os.fstat(directory_fd)

    if (
        not stat.S_ISDIR(directory.st_mode)
        or directory.st_uid != os.geteuid()
        or stat.S_IMODE(directory.st_mode) != 0o700
    ):
        raise OCIStagingError("Unsafe staging directory descriptor")

    filename = "sha256-" + digest[7:]

    flags = (
        os.O_RDONLY
        | os.O_NOFOLLOW
        | os.O_CLOEXEC
        | os.O_NONBLOCK
    )

    file_fd = os.open(
        filename,
        flags,
        dir_fd=directory_fd,
    )

    try:
        metadata = os.fstat(file_fd)

        if (
            not stat.S_ISREG(metadata.st_mode)
            or metadata.st_uid != os.geteuid()
            or stat.S_IMODE(metadata.st_mode) != 0o600
            or metadata.st_nlink != 1
        ):
            raise OCIStagingError("Unsafe staged OCI blob")

        return os.fdopen(file_fd, "rb")

    except BaseException:
        os.close(file_fd)
        raise



def validate_trusted_oci_digest(value: str) -> str:
    """Validate a separately supplied OCI SHA-256 trust anchor.

    The caller must establish the digest's provenance independently.
    A registry-provided digest header is not a trusted pin.
    """
    if (
        not isinstance(value, str)
        or re.fullmatch(r"sha256:[0-9a-f]{64}", value) is None
    ):
        raise ImageReferenceError(
            "A trusted OCI SHA-256 manifest digest is required"
        )

    return value


@dataclass(frozen=True)
class OCIAcquisitionPlan:
    registry: str
    repository: str
    runtime_reference: str
    manifest_reference: str
    expected_digest: str


def plan_pgvector_oci_acquisition(
    runtime_reference: str,
    trusted_digest: str,
) -> OCIAcquisitionPlan:
    """Plan digest-pinned pgvector acquisition without network access."""

    if (
        not isinstance(runtime_reference, str)
        or runtime_reference != "pgvector/pgvector:pg17"
    ):
        raise ImageReferenceError(
            "Unsupported pgvector runtime image reference"
        )

    expected_digest = validate_trusted_oci_digest(trusted_digest)

    runtime = parse_image_reference(runtime_reference)

    if (
        runtime.registry != "registry-1.docker.io"
        or runtime.repository != "pgvector/pgvector"
        or runtime.tag != "pg17"
        or runtime.digest is not None
    ):
        raise ImageReferenceError(
            "Unexpected pgvector runtime image identity"
        )

    manifest_reference = runtime.repository + "@" + expected_digest
    pinned = parse_image_reference(manifest_reference)

    if (
        pinned.registry != runtime.registry
        or pinned.repository != runtime.repository
        or pinned.digest != expected_digest
    ):
        raise ImageReferenceError(
            "OCI acquisition reference mismatch"
        )

    return OCIAcquisitionPlan(
        registry=pinned.registry,
        repository=pinned.repository,
        runtime_reference=runtime_reference,
        manifest_reference=manifest_reference,
        expected_digest=expected_digest,
    )


@dataclass(frozen=True)
class OCIHTTPRequest:
    url: str
    headers: tuple[tuple[str, str], ...]
    auth_scheme: str | None


def _validate_pgvector_acquisition_plan(
    plan: OCIAcquisitionPlan,
) -> OCIAcquisitionPlan:
    """Reject externally constructed or modified acquisition plans."""

    if not isinstance(plan, OCIAcquisitionPlan):
        raise ImageReferenceError("Invalid OCI acquisition plan")

    canonical = plan_pgvector_oci_acquisition(
        plan.runtime_reference,
        plan.expected_digest,
    )

    if plan != canonical:
        raise ImageReferenceError("Noncanonical OCI acquisition plan")

    return canonical


def build_oci_manifest_request(
    plan: OCIAcquisitionPlan,
) -> OCIHTTPRequest:
    """Plan an HTTPS request for the trusted manifest digest."""

    canonical = _validate_pgvector_acquisition_plan(plan)

    return OCIHTTPRequest(
        url=(
            "https://registry-1.docker.io/v2/"
            + canonical.repository
            + "/manifests/"
            + canonical.expected_digest
        ),
        headers=(
            (
                "Accept",
                "application/vnd.oci.image.index.v1+json, "
                "application/vnd.docker.distribution.manifest.list.v2+json, "
                "application/vnd.oci.image.manifest.v1+json, "
                "application/vnd.docker.distribution.manifest.v2+json",
            ),
        ),
        auth_scheme="Bearer",
    )


def build_oci_blob_request(
    plan: OCIAcquisitionPlan,
    digest: str,
) -> OCIHTTPRequest:
    """Plan an HTTPS request for a SHA-256-addressed OCI blob."""

    canonical = _validate_pgvector_acquisition_plan(plan)
    verified_digest = validate_trusted_oci_digest(digest)

    return OCIHTTPRequest(
        url=(
            "https://registry-1.docker.io/v2/"
            + canonical.repository
            + "/blobs/"
            + verified_digest
        ),
        headers=(),
        auth_scheme="Bearer",
    )


def build_oci_token_http_request(
    plan: OCIAcquisitionPlan,
) -> OCIHTTPRequest:
    """Plan an anonymous Docker Hub token request."""

    from urllib.parse import urlencode

    canonical = _validate_pgvector_acquisition_plan(plan)
    reference = parse_image_reference(
        canonical.manifest_reference
    )
    token_request = build_docker_hub_token_request(reference)

    return OCIHTTPRequest(
        url=(
            token_request.endpoint
            + "?"
            + urlencode(token_request.parameters)
        ),
        headers=(),
        auth_scheme=None,
    )


def validate_oci_redirect(
    source_url: str,
    destination_url: str,
    request_kind: str,
    redirect_count: int = 1,
) -> None:
    """Authorize one digest-bound Docker Hub blob CDN redirect.

    This validates URLs only. The transport must independently prevent
    registry bearer credentials from reaching the CDN.
    """
    from urllib.parse import urlsplit, parse_qsl

    if request_kind not in ("token", "manifest", "blob"):
        raise ImageReferenceError("Unsupported OCI redirect kind")

    if type(redirect_count) is not int or redirect_count != 1:
        raise ImageReferenceError("OCI redirect chain is unsupported")

    if type(source_url) is not str or type(destination_url) is not str:
        raise ImageReferenceError("Invalid OCI redirect URL")

    if request_kind != "blob":
        raise ImageReferenceError("OCI token/manifest redirects are forbidden")

    if (
        not source_url.isascii()
        or not destination_url.isascii()
        or len(source_url) > 512
        or len(destination_url) > 8192
        or any(ord(c) < 33 or ord(c) > 126 for c in source_url)
        or any(ord(c) < 33 or ord(c) > 126 for c in destination_url)
        or "\\\\" in source_url
        or "\\\\" in destination_url
    ):
        raise ImageReferenceError("Unsafe OCI redirect URL")

    prefix = (
        "https://registry-1.docker.io/v2/"
        "pgvector/pgvector/blobs/sha256:"
    )

    if not source_url.startswith(prefix):
        raise ImageReferenceError("Noncanonical OCI blob source")

    digest = source_url[len(prefix):]

    if not re.fullmatch(r"[0-9a-f]{64}", digest):
        raise ImageReferenceError("Invalid OCI blob source digest")

    try:
        parsed = urlsplit(destination_url)
    except ValueError as exc:
        raise ImageReferenceError("Invalid OCI CDN destination") from exc

    if (
        parsed.scheme != "https"
        or parsed.netloc != "production.cloudfront.docker.com"
        or parsed.fragment
    ):
        raise ImageReferenceError("Unapproved OCI CDN destination")

    expected_path = (
        "/registry-v2/docker/registry/v2/blobs/"
        f"sha256/{digest[:2]}/{digest}/data"
    )

    if parsed.path != expected_path:
        raise ImageReferenceError("OCI CDN blob digest/path mismatch")

    if (
        not parsed.query
        or "%" in parsed.query
        or "\\\\" in parsed.query
    ):
        raise ImageReferenceError("Invalid OCI CDN signed query")

    try:
        pairs = parse_qsl(
            parsed.query,
            keep_blank_values=True,
            strict_parsing=True,
            max_num_fields=3,
        )
    except ValueError as exc:
        raise ImageReferenceError("Invalid OCI CDN signed query") from exc

    parameters = dict(pairs)

    if (
        len(pairs) != 3
        or len(parameters) != 3
        or set(parameters) != {"Expires", "Key-Pair-Id", "Signature"}
        or not all(parameters.values())
        or not re.fullmatch(r"[0-9]+", parameters["Expires"])
        or not re.fullmatch(
            r"[A-Za-z0-9_-]+", parameters["Key-Pair-Id"]
        )
        or not re.fullmatch(
            r"[A-Za-z0-9_~=-]+", parameters["Signature"]
        )
    ):
        raise ImageReferenceError("Invalid OCI CDN signature parameters")


def validate_oci_http_response(
    status: int,
    headers: dict,
    max_bytes: int,
) -> int:
    """Validate bounded, uncompressed OCI HTTP response metadata.

    The transport must independently enforce the byte limit while
    reading the response body; headers alone cannot prove integrity.
    """

    if type(status) is not int or status != 200:
        raise OCIManifestError("Unexpected OCI HTTP response status")

    if type(max_bytes) is not int or max_bytes < 0:
        raise OCIManifestError("Invalid OCI HTTP response limit")

    from http.client import HTTPMessage

    if not isinstance(headers, (dict, HTTPMessage)):
        raise OCIManifestError("Invalid OCI HTTP response headers")

    normalized = {}

    for key, value in headers.items():
        if not isinstance(key, str) or not isinstance(value, str):
            raise OCIManifestError("Invalid OCI HTTP header")

        if re.fullmatch(
            r"[!#$%&'*+.^_`|~0-9A-Za-z-]+",
            key,
            flags=re.ASCII,
        ) is None:
            raise OCIManifestError("Invalid OCI HTTP header name")

        if any(
            ord(character) < 32 or ord(character) == 127
            for character in value
        ):
            raise OCIManifestError("Invalid OCI HTTP header value")

        name = key.lower()

        if name in normalized:
            raise OCIManifestError("Duplicate OCI HTTP header")

        normalized[name] = value

    transfer_encoding = normalized.get("transfer-encoding")

    if transfer_encoding is not None:
        if transfer_encoding.lower() != "chunked":
            raise OCIManifestError(
                "Unsupported OCI HTTP Transfer-Encoding"
            )

        if "content-length" in normalized:
            raise OCIManifestError(
                "Ambiguous OCI HTTP response framing"
            )

    if "upgrade" in normalized:
        raise OCIManifestError(
            "OCI HTTP protocol upgrade is unsupported"
        )

    connection = normalized.get("connection", "")

    if "upgrade" in (
        token.strip().lower()
        for token in connection.split(",")
    ):
        raise OCIManifestError(
            "OCI HTTP connection upgrade is unsupported"
        )

    encoding = normalized.get("content-encoding", "identity")

    if encoding.lower() != "identity":
        raise OCIManifestError(
            "Compressed OCI HTTP response is unsupported"
        )

    length = normalized.get("content-length")

    if transfer_encoding == "chunked" or (
        transfer_encoding is not None
        and transfer_encoding.lower() == "chunked"
    ):
        return -1

    if length is None or re.fullmatch(r"[0-9]+", length) is None:
        raise OCIManifestError("Invalid OCI HTTP Content-Length")

    if len(length) > 20:
        raise OCIManifestError("OCI HTTP Content-Length is too large")

    size = int(length)

    if size > max_bytes:
        raise OCIManifestError("OCI HTTP response exceeds byte limit")

    return size






def fetch_docker_hub_anonymous_token(
    plan: OCIAcquisitionPlan,
    connection_factory=None,
) -> str:
    """Obtain a bounded, validated anonymous Docker Hub bearer token."""
    import json

    request = build_oci_token_http_request(plan)

    if connection_factory is None:
        response = fetch_oci_https_response(
            request.url,
            max_bytes=65536,
        )
    else:
        response = fetch_oci_https_response(
            request.url,
            max_bytes=65536,
            connection_factory=connection_factory,
        )

    try:
        payload = json.loads(response)
    except (ValueError, UnicodeDecodeError, TypeError) as error:
        raise DockerHubTokenError(
            "Invalid Docker Hub token response JSON"
        ) from error

    return validate_docker_hub_token_response(payload)



def resolve_pgvector_oci_manifest(
    plan: OCIAcquisitionPlan,
    bearer_token: str | None = None,
    connection_factory=None,
) -> dict:
    """Resolve a trusted OCI index to one verified linux/amd64 manifest."""
    import json

    _validate_pgvector_acquisition_plan(plan)

    base_url = (
        "https://registry-1.docker.io/v2/"
        "pgvector/pgvector/manifests/"
    )

    index_bytes = fetch_verified_oci_manifest(
        base_url + plan.expected_digest,
        trusted_digest=plan.expected_digest,
        max_bytes=MAX_OCI_MANIFEST_BYTES,
        connection_factory=connection_factory,
        bearer_token=bearer_token,
    )

    try:
        index = json.loads(index_bytes)
    except (ValueError, UnicodeDecodeError) as error:
        raise OCIManifestError("Invalid OCI image index JSON") from error

    selected = select_linux_amd64_manifest(index)

    manifest_bytes = fetch_verified_oci_manifest(
        base_url + selected.digest,
        trusted_digest=selected.digest,
        max_bytes=selected.size,
        connection_factory=connection_factory,
        bearer_token=bearer_token,
    )

    if len(manifest_bytes) != selected.size:
        raise OCIManifestError("Selected manifest size mismatch")

    try:
        manifest = json.loads(manifest_bytes)
    except (ValueError, UnicodeDecodeError) as error:
        raise OCIManifestError("Invalid OCI image manifest JSON") from error

    validate_oci_image_manifest(manifest)

    return manifest


def fetch_verified_oci_manifest(
    url: str,
    trusted_digest: str,
    max_bytes: int,
    connection_factory=None,
    bearer_token: str | None = None,
) -> bytes:
    """Fetch and verify one digest-pinned OCI manifest.

    The trusted digest must originate outside the registry response.
    This function performs no publication or persistent writes.
    """
    from urllib.parse import urlsplit

    expected_digest = validate_trusted_oci_digest(trusted_digest)

    if type(max_bytes) is not int or not (
        0 <= max_bytes <= MAX_OCI_MANIFEST_BYTES
    ):
        raise OCIManifestError("Invalid OCI manifest byte limit")

    expected_url = (
        "https://registry-1.docker.io/v2/"
        "pgvector/pgvector/manifests/"
        + expected_digest
    )

    if not isinstance(url, str) or url != expected_url:
        raise OCIManifestError(
            "OCI manifest URL does not match trusted digest"
        )

    manifest = fetch_oci_https_response(
        url,
        max_bytes=max_bytes,
        connection_factory=connection_factory,
        bearer_token=bearer_token,
    )

    verify_oci_manifest_bytes(
        manifest,
        expected_digest,
        len(manifest),
    )

    return manifest



class BoundedOCIHTTPReader:
    """Expose exactly the declared HTTP response bytes to a consumer.

    The caller owns the underlying response and HTTPS connection.
    """

    def __init__(self, reader, declared_size: int):
        if (
            type(declared_size) is not int
            or declared_size < 0
            or declared_size > (1 << 63) - 1
        ):
            raise OCIManifestError("Invalid OCI HTTP declared length")

        self._reader = reader
        self._remaining = declared_size

    def read(self, size: int) -> bytes:
        if type(size) is not int or size < 0:
            raise OCIManifestError("Invalid OCI HTTP read size")

        if size == 0 or self._remaining == 0:
            return b""

        requested = min(size, self._remaining)
        chunk = self._reader.read(requested)

        if not isinstance(chunk, bytes):
            raise OCIManifestError("OCI HTTP reader returned non-bytes")

        if len(chunk) > requested:
            raise OCIManifestError(
                "OCI HTTP reader exceeded requested size"
            )

        if not chunk:
            raise OCIManifestError("Truncated OCI HTTP response")

        self._remaining -= len(chunk)
        return chunk





def open_verified_oci_blob_readers(
    directory_fd: int,
    descriptors,
):
    """Hold verified OCI blob readers open through caller consumption."""
    from contextlib import ExitStack, contextmanager

    @contextmanager
    def verified_readers():
        with ExitStack() as stack:
            readers = []

            for descriptor in descriptors:
                try:
                    expected_digest, expected_size = descriptor
                except (TypeError, ValueError) as error:
                    raise OCIManifestError(
                        "Invalid staged OCI blob descriptor"
                    ) from error

                reader = stack.enter_context(
                    open_verified_oci_staged_blob(
                        directory_fd,
                        expected_digest,
                    )
                )

                verify_oci_blob_stream(
                    reader,
                    expected_digest,
                    expected_size,
                )

                reader.seek(0)
                readers.append(reader)

            yield tuple(readers)

    return verified_readers()



def stage_pgvector_oci_blobs(
    plan: OCIAcquisitionPlan,
    manifest: dict,
    directory_fd: int,
    bearer_token: str | None = None,
    connection_factory=None,
) -> tuple[str, ...]:
    """Stage digest-verified OCI config and layers in descriptor order."""
    _validate_pgvector_acquisition_plan(plan)

    image = validate_oci_image_manifest(manifest)

    descriptors = (image.config,) + image.layers

    sizes_by_digest = {}

    for descriptor in descriptors:
        previous_size = sizes_by_digest.get(descriptor.digest)

        if (
            previous_size is not None
            and previous_size != descriptor.size
        ):
            raise OCIManifestError(
                "Conflicting sizes for identical OCI blob digest"
            )

        sizes_by_digest[descriptor.digest] = descriptor.size

    filenames_by_digest = {}
    staged = []

    for descriptor in descriptors:
        filename = filenames_by_digest.get(descriptor.digest)

        if filename is None:
            try:
                existing = open_verified_oci_staged_blob(
                    directory_fd,
                    descriptor.digest,
                )
            except FileNotFoundError:
                existing = None

            if existing is not None:
                with existing:
                    verify_oci_blob_stream(
                        existing,
                        descriptor.digest,
                        descriptor.size,
                    )

                filename = "sha256-" + descriptor.digest[7:]

            else:
                request = build_oci_blob_request(
                    plan,
                    descriptor.digest,
                )

                filename = fetch_and_publish_oci_https_blob(
                    request.url,
                    trusted_digest=descriptor.digest,
                    expected_size=descriptor.size,
                    directory_fd=directory_fd,
                    connection_factory=connection_factory,
                    bearer_token=bearer_token,
                )

            filenames_by_digest[descriptor.digest] = filename

        staged.append(filename)

    return tuple(staged)



def assemble_staged_pgvector_oci_image(
    plan: OCIAcquisitionPlan,
    manifest: dict,
    staging_directory,
    bearer_token: str | None = None,
    connection_factory=None,
):
    """Stage and assemble a digest-verified offline pgvector image."""
    from pathlib import Path

    _validate_pgvector_acquisition_plan(plan)

    trusted_manifest = resolve_pgvector_oci_manifest(
        plan,
        bearer_token=bearer_token,
        connection_factory=connection_factory,
    )

    if manifest != trusted_manifest:
        raise OCIManifestError(
            "Caller manifest differs from trusted OCI resolution"
        )

    image = validate_oci_image_manifest(trusted_manifest)

    directory = Path(staging_directory)

    directory_fd = open_oci_staging_directory(str(directory))

    try:
        stage_pgvector_oci_blobs(
            plan,
            trusted_manifest,
            directory_fd,
            bearer_token=bearer_token,
            connection_factory=connection_factory,
        )

        descriptors = tuple(
            (descriptor.digest, descriptor.size)
            for descriptor in (image.config,) + image.layers
        )

        with open_verified_oci_blob_readers(
            directory_fd,
            descriptors,
        ) as readers:
            config_bytes = readers[0].read()

            return assemble_verified_oci_archive(
                manifest=trusted_manifest,
                config_bytes=config_bytes,
                compressed_layers=readers[1:],
                runtime_reference=plan.runtime_reference,
                destination=directory / "pgvector.tar",
            )

    finally:
        os.close(directory_fd)



def fetch_and_publish_oci_https_blob(
    url: str,
    trusted_digest: str,
    expected_size: int,
    directory_fd: int,
    connection_factory=None,
    bearer_token: str | None = None,
) -> str:
    """Fetch a digest-pinned blob, allowing one isolated CDN redirect."""
    import http.client
    import ssl
    from urllib.parse import urlsplit

    digest = validate_trusted_oci_digest(trusted_digest)

    canonical_url = (
        "https://registry-1.docker.io/v2/"
        "pgvector/pgvector/blobs/" + digest
    )

    if url != canonical_url:
        raise OCIManifestError(
            "OCI blob URL does not match trusted digest"
        )

    if (
        type(expected_size) is not int
        or expected_size < 0
        or expected_size > (1 << 63) - 1
    ):
        raise OCIManifestError("Invalid OCI blob descriptor size")

    if bearer_token is not None:
        if (
            not isinstance(bearer_token, str)
            or not bearer_token
            or len(bearer_token) > 16384
            or any(
                ord(character) < 33 or ord(character) > 126
                for character in bearer_token
            )
        ):
            raise OCIManifestError("Invalid OCI bearer token")

    context = ssl.create_default_context()
    factory = connection_factory or http.client.HTTPSConnection
    connections = []

    try:
        registry = factory(
            "registry-1.docker.io",
            timeout=30,
            context=context,
        )
        connections.append(registry)

        headers = {"Accept-Encoding": "identity"}

        if bearer_token is not None:
            headers["Authorization"] = "Bearer " + bearer_token

        registry.request(
            "GET",
            "/v2/pgvector/pgvector/blobs/" + digest,
            body=None,
            headers=headers,
        )

        response = registry.getresponse()

        if response.status == 307:
            from http.client import HTTPMessage

            if not isinstance(response.headers, (dict, HTTPMessage)):
                raise OCIManifestError("Invalid OCI redirect headers")

            normalized_headers = {}

            for key, value in response.headers.items():
                if not isinstance(key, str) or not isinstance(value, str):
                    raise OCIManifestError("Invalid OCI redirect header")

                if re.fullmatch(
                    r"[!#$%&'*+.^_`|~0-9A-Za-z-]+",
                    key,
                    flags=re.ASCII,
                ) is None:
                    raise OCIManifestError("Invalid OCI redirect header name")

                if any(
                    ord(character) < 32 or ord(character) == 127
                    for character in value
                ):
                    raise OCIManifestError("Invalid OCI redirect header value")

                name = key.lower()

                if name in normalized_headers:
                    raise OCIManifestError("Duplicate OCI redirect header")

                normalized_headers[name] = value

            if any(name in normalized_headers for name in (
                "transfer-encoding",
                "upgrade",
                "proxy-authenticate",
                "proxy-authorization",
            )):
                raise OCIManifestError("Unsafe OCI redirect headers")

            if normalized_headers.get(
                "content-encoding", "identity"
            ).lower() != "identity":
                raise OCIManifestError("Encoded OCI redirect unsupported")

            if "upgrade" in (
                item.strip().lower()
                for item in normalized_headers.get(
                    "connection", ""
                ).split(",")
            ):
                raise OCIManifestError("OCI redirect upgrade unsupported")

            content_length = normalized_headers.get("content-length")

            if content_length is not None:
                if (
                    re.fullmatch(r"[0-9]+", content_length) is None
                    or len(content_length) > 20
                    or int(content_length) > 65536
                ):
                    raise OCIManifestError(
                        "Invalid OCI redirect Content-Length"
                    )

            locations = (
                [normalized_headers["location"]]
                if "location" in normalized_headers
                else []
            )

            if len(locations) != 1 or not isinstance(locations[0], str):
                raise OCIManifestError("Ambiguous OCI redirect Location")

            destination = locations[0]

            validate_oci_redirect(
                url,
                destination,
                "blob",
                redirect_count=1,
            )

            parsed = urlsplit(destination)

            # Close the authenticated registry connection before
            # opening the credential-free CDN connection.
            registry.close()

            cdn = factory(
                "production.cloudfront.docker.com",
                timeout=30,
                context=context,
            )
            connections.append(cdn)

            target = parsed.path + "?" + parsed.query

            cdn.request(
                "GET",
                target,
                body=None,
                headers={"Accept-Encoding": "identity"},
            )

            response = cdn.getresponse()

        declared_size = validate_oci_http_response(
            response.status,
            response.headers,
            expected_size,
        )

        if declared_size != expected_size:
            raise OCIManifestError(
                "OCI HTTP length differs from trusted descriptor"
            )

        reader = BoundedOCIHTTPReader(
            response,
            declared_size,
        )

        return publish_verified_oci_blob(
            directory_fd,
            reader,
            digest,
            expected_size,
        )

    finally:
        for connection in reversed(connections):
            connection.close()



def fetch_oci_https_response(
    url: str,
    max_bytes: int,
    bearer_token: str | None = None,
    connection_factory=None,
) -> bytes:
    """Fetch one bounded HTTPS response without following redirects.

    This low-level transport does not establish trusted OCI provenance.
    The caller must independently verify the expected SHA-256 digest.
    """

    import http.client
    import ssl
    from urllib.parse import urlsplit

    if not isinstance(url, str):
        raise OCIManifestError("Invalid OCI HTTPS URL")

    if "#" in url:
        raise OCIManifestError("OCI HTTPS URL fragments are forbidden")

    try:
        parsed = urlsplit(url)

        if (
            parsed.scheme != "https"
            or parsed.username is not None
            or parsed.password is not None
            or parsed.port is not None
            or parsed.fragment
            or not parsed.path.startswith("/")
        ):
            raise OCIManifestError("Unsafe OCI HTTPS URL")

        host = parsed.hostname

    except ValueError as error:
        raise OCIManifestError("Invalid OCI HTTPS URL") from error

    if host not in (
        "registry-1.docker.io",
        "auth.docker.io",
    ):
        raise OCIManifestError("Unapproved OCI HTTPS host")

    if parsed.netloc != host:
        raise OCIManifestError("Noncanonical OCI HTTPS authority")

    if host == "registry-1.docker.io":
        if parsed.query:
            raise OCIManifestError("Unexpected OCI HTTPS query")

        if re.fullmatch(
            r"/v2/pgvector/pgvector/"
            r"(?:manifests|blobs)/sha256:[0-9a-f]{64}",
            parsed.path,
            flags=re.ASCII,
        ) is None:
            raise OCIManifestError(
                "Noncanonical OCI registry resource"
            )

    else:
        from urllib.parse import urlencode

        if parsed.path != "/token":
            raise OCIManifestError(
                "Noncanonical OCI token endpoint"
            )

        expected_query = urlencode((
            ("service", "registry.docker.io"),
            ("scope", "repository:pgvector/pgvector:pull"),
        ))

        if parsed.query != expected_query:
            raise OCIManifestError(
                "Noncanonical OCI token request"
            )

    if type(max_bytes) is not int or max_bytes < 0:
        raise OCIManifestError("Invalid OCI HTTPS byte limit")

    if bearer_token is not None:
        if host != "registry-1.docker.io":
            raise OCIManifestError("Bearer token destination mismatch")

        if (
            not isinstance(bearer_token, str)
            or not bearer_token
            or len(bearer_token) > 16384
            or any(ord(character) < 33 or ord(character) > 126
                   for character in bearer_token)
        ):
            raise OCIManifestError("Invalid OCI bearer token")

    context = ssl.create_default_context()
    factory = connection_factory or http.client.HTTPSConnection

    connection = None

    try:
        connection = factory(
            host,
            timeout=30,
            context=context,
        )

        headers = {"Accept-Encoding": "identity"}

        if bearer_token is not None:
            headers["Authorization"] = "Bearer " + bearer_token

        target = parsed.path

        if parsed.query:
            target += "?" + parsed.query

        connection.request(
            "GET",
            target,
            body=None,
            headers=headers,
        )

        response = connection.getresponse()

        expected_size = validate_oci_http_response(
            response.status,
            response.headers,
            max_bytes,
        )

        chunks = []
        remaining = expected_size

        if expected_size == -1:
            total = 0

            while True:
                chunk = response.read(
                    min(65536, max_bytes - total + 1)
                )

                if not isinstance(chunk, bytes):
                    raise OCIManifestError(
                        "OCI HTTPS reader returned non-bytes"
                    )

                if not chunk:
                    break

                total += len(chunk)

                if total > max_bytes:
                    raise OCIManifestError(
                        "OCI HTTPS response exceeds byte limit"
                    )

                chunks.append(chunk)

            return b"".join(chunks)

        while remaining:
            chunk = response.read(min(remaining, 65536))

            if not isinstance(chunk, bytes) or not chunk:
                raise OCIManifestError(
                    "Truncated OCI HTTPS response"
                )

            if len(chunk) > remaining:
                raise OCIManifestError(
                    "OCI HTTPS response exceeds Content-Length"
                )

            chunks.append(chunk)
            remaining -= len(chunk)

        if response.read(1):
            raise OCIManifestError(
                "OCI HTTPS response exceeds Content-Length"
            )

        return b"".join(chunks)

    finally:
        if connection is not None:
            connection.close()



def parse_image_reference(value: str) -> ImageReference:
    """Parse an explicitly tagged Docker Hub repository reference.

    This initial implementation deliberately supports only Docker Hub
    repository names without an explicit registry hostname.
    """

    if not isinstance(value, str) or not value:
        raise ImageReferenceError("Image reference is required")

    if value != value.strip():
        raise ImageReferenceError("Whitespace is not permitted")

    digest = None
    tag = ""

    if "@" in value:
        if value.count("@") != 1:
            raise ImageReferenceError("Exactly one digest separator is required")

        repository, digest = value.split("@", 1)

        if ":" in repository:
            raise ImageReferenceError("Tag and digest combination is unsupported")

        if re.fullmatch(r"sha256:[0-9a-f]{64}", digest) is None:
            raise ImageReferenceError("Invalid SHA-256 image digest")
    else:
        if value.count(":") != 1:
            raise ImageReferenceError("Exactly one image tag is required")

        repository, tag = value.rsplit(":", 1)

        if not _TAG.fullmatch(tag):
            raise ImageReferenceError("Invalid image tag")

    components = repository.split("/")

    if not 1 <= len(components) <= 2:
        raise ImageReferenceError("Unsupported repository structure")

    if not all(_COMPONENT.fullmatch(part) for part in components):
        raise ImageReferenceError("Invalid repository component")

    if len(components) == 2 and "." in components[0]:
        raise ImageReferenceError("Explicit registry hostnames are unsupported")

    if len(components) == 1:
        repository = "library/" + repository

    return ImageReference(
        registry="registry-1.docker.io",
        repository=repository,
        tag=tag,
        digest=digest,
    )

def publish_unnamed_oci_file(
    file_fd: int,
    directory_fd: int,
    destination: str,
) -> None:
    """Atomically link an unnamed OCI file without replacing a destination."""

    if (
        not isinstance(destination, str)
        or re.fullmatch(r"sha256-[0-9a-f]{64}", destination) is None
    ):
        raise OCIStagingError("Invalid OCI publication destination")

    directory = os.fstat(directory_fd)
    source = os.fstat(file_fd)

    if (
        not stat.S_ISDIR(directory.st_mode)
        or directory.st_uid != os.geteuid()
        or stat.S_IMODE(directory.st_mode) != 0o700
    ):
        raise OCIStagingError("Unsafe staging directory descriptor")

    if (
        not stat.S_ISREG(source.st_mode)
        or source.st_uid != os.geteuid()
        or stat.S_IMODE(source.st_mode) != 0o600
        or source.st_nlink != 0
    ):
        raise OCIStagingError("Unsafe unnamed OCI file descriptor")

    import ctypes

    libc = ctypes.CDLL(None, use_errno=True)
    linkat = libc.linkat
    linkat.argtypes = [
        ctypes.c_int,
        ctypes.c_char_p,
        ctypes.c_int,
        ctypes.c_char_p,
        ctypes.c_int,
    ]
    linkat.restype = ctypes.c_int

    ctypes.set_errno(0)

    result = linkat(
        file_fd,
        b"",
        directory_fd,
        destination.encode("ascii"),
        0x1000,
    )

    if result != 0:
        code = ctypes.get_errno()
        raise OSError(code, os.strerror(code))

def acquire_pgvector_offline_image(
    runtime_reference: str,
    trusted_digest: str,
    staging_directory,
    connection_factory=None,
):
    """Explicitly acquire a digest-pinned offline pgvector image."""
    plan = plan_pgvector_oci_acquisition(
        runtime_reference,
        trusted_digest,
    )

    token = fetch_docker_hub_anonymous_token(
        plan,
        connection_factory=connection_factory,
    )

    manifest = resolve_pgvector_oci_manifest(
        plan,
        bearer_token=token,
        connection_factory=connection_factory,
    )

    return assemble_staged_pgvector_oci_image(
        plan,
        manifest,
        staging_directory,
        bearer_token=token,
        connection_factory=connection_factory,
    )

def main(argv=None):
    """Explicit, digest-pinned offline image acquisition CLI."""
    import argparse

    parser = argparse.ArgumentParser(
        description=(
            "Acquire a digest-verified pgvector Docker archive "
            "for offline DAIA packaging."
        )
    )

    parser.add_argument(
        "--reference",
        required=True,
        help="Runtime image reference",
    )
    parser.add_argument(
        "--trusted-digest",
        required=True,
        help="Independently approved sha256 OCI index digest",
    )
    parser.add_argument(
        "--staging-directory",
        required=True,
        help="Existing secure staging directory",
    )

    args = parser.parse_args(argv)

    try:
        result = acquire_pgvector_offline_image(
            args.reference,
            args.trusted_digest,
            args.staging_directory,
        )
    except (
        ValueError,
        OSError,
    ) as error:
        print(
            "OCI acquisition failed: "
            + type(error).__name__,
            file=sys.stderr,
        )
        return 1

    print("Verified offline OCI archive:", result)
    return 0


if __name__ == "__main__":
    sys.exit(main())
