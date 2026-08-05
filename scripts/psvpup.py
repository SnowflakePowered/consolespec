"""PS Vita PUP firmware extraction: PUP -> SPKG -> flash partition images.

A Vita PUP's 0x3xx segments are SPKG containers (SCE "Certified File",
little-endian, 0x30-byte header). Their metadata is decrypted with a
published ERK/RIV keyset, and the payload section with AES-128-CTR using
per-section keys from the metadata. The payloads are raw FAT filesystem
images of the console's flash partitions, split across 8 MiB segments.

Keys are the public SPKG keysets published on psdevwiki (vita/Keys).

Requires pycryptodome.
"""
import struct

from Crypto.Cipher import AES
from Crypto.Util import Counter

# SPKG "Certified File" keysets: retail units use `external`, devkit and
# prototype firmware use `internal` / `proto`.
KEYSETS = {
    'external': ('2e6f4751d15b06c51f572a9306e52dd7007ea56a31d459ec6d3681ab08625501',
                 'b3d541a568751df8f4833bab4efe0537'),
    'internal': ('23f1d525244266e6da7a52da9446318301ee8cc58d54901ae94d93010f7dee6b',
                 '3721f7c05de5f55ecc39bddb4a6c585d'),
    'proto':    ('fa88e5b5cbb49603df689f139045e7c3c9c7e33b5923df54e4c5fe5298b4fd32',
                 '5eaa69ab35e737ec22c721a916e00263'),
}

PUP_MAGIC = b'SCEUF\0\0\1'
SEGMENT_VERSION = 0x100


def pup_segments(path):
    """Return (open file, [(segment_id, offset, size), ...]) for a Vita PUP."""
    f = open(path, 'rb')
    header = f.read(0x1000)
    if header[:8] != PUP_MAGIC:
        f.close()
        raise ValueError('not a PS Vita PUP (bad magic)')
    count, = struct.unpack_from('<Q', header, 0x18)
    segments = [struct.unpack_from('<QQQQ', header, 0x80 + i * 32)[:3] for i in range(count)]
    return f, segments


def pup_version(path):
    """Return the firmware version a PUP declares, e.g. '3.74'."""
    f, segments = pup_segments(path)
    with f:
        for seg_id, off, size in segments:
            if seg_id == SEGMENT_VERSION:
                f.seek(off)
                return f.read(size).decode('ascii', 'replace').split('\n')[0].strip()
    return ''


def spkg_payload(blob, keyset=None):
    """Decrypt an SPKG container, returning (payload, keyset_name).

    Returns (None, None) when no keyset validates the metadata.
    """
    if len(blob) < 0x70:
        return None, None
    header_len, = struct.unpack_from('<Q', blob, 16)
    names = [keyset] if keyset else list(KEYSETS)
    for name in names:
        erk, riv = KEYSETS[name]
        info = AES.new(bytes.fromhex(erk), AES.MODE_CBC, bytes.fromhex(riv)).decrypt(blob[0x30:0x70])
        if info[16:32] != b'\0' * 16 or info[48:64] != b'\0' * 16:
            continue
        headers = AES.new(info[:16], AES.MODE_CBC, info[32:48]).decrypt(blob[0x70:header_len])
        _sig, _u1, section_count, key_count, _opt, _u2, _u3 = \
            struct.unpack_from('<QIIIIII', headers, 0)
        keys_off = 0x20 + section_count * 0x30
        data_keys = headers[keys_off:keys_off + key_count * 0x10]
        # the payload is the last section; earlier ones are small metadata blobs
        off, size, _type, _idx, _hashed, _sha1, encrypted, key_idx, iv_idx, _comp = \
            struct.unpack_from('<QQIIIIIIII', headers, 0x20 + (section_count - 1) * 0x30)
        data = blob[off:off + size]
        if encrypted == 3:
            iv = data_keys[iv_idx * 16:iv_idx * 16 + 16]
            counter = Counter.new(128, initial_value=int.from_bytes(iv, 'big'))
            data = AES.new(data_keys[key_idx * 16:key_idx * 16 + 16],
                           AES.MODE_CTR, counter=counter).decrypt(data)
        return data, name
    return None, None


def is_fat_boot_sector(data):
    return (len(data) > 0x200 and data[0x0B:0x0D] == b'\x00\x02'
            and data[3:11] == b'SCEI    ')


def partition_images(pup_path):
    """Return {partition: image_bytes} for a PUP's flash partition images.

    Payload segments that begin with a FAT boot sector start a new image;
    the segments that follow continue it until the next boot sector.
    """
    f, segments = pup_segments(pup_path)
    images, current = [], None
    with f:
        for seg_id, off, size in segments:
            if not 0x300 <= seg_id < 0x400:
                continue
            f.seek(off)
            payload, _keyset = spkg_payload(f.read(size))
            if payload is None:
                continue
            if is_fat_boot_sector(payload):
                current = bytearray(payload)
                images.append(current)
            elif current is not None:
                current += payload
    return {name: bytes(img) for name, img in zip(_names(images), images)}


def _names(images):
    """Name each image after the partition its root directory identifies."""
    from fatfs import Fat  # local import so callers can use the crypto alone
    used = {}
    for img in images:
        try:
            paths = [name for name, _size, _get in Fat(img).walk()]
        except Exception:  # noqa: BLE001 - an unreadable image still gets a name
            paths = []
        roots = {p.split('/')[0] for p in paths}
        if 'kd' in roots:
            name = 'os0'
        elif roots & {'app', 'vsh'}:
            name = 'vs0'
        elif any(p.startswith('data/dic') for p in paths):
            name = 'sa0'  # dictionary data
        elif roots & {'psp2config', 'registry'}:
            name = 'ur0'
        else:
            name = 'unknown'
        used[name] = used.get(name, 0) + 1
        yield name if used[name] == 1 else f'{name}_{used[name]}'
