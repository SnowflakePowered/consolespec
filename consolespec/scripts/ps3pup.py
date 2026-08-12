"""PS3 PUP firmware extraction: PUP -> update_files.tar -> dev_flash trees.

Implements the same path RPCS3 uses to install firmware: each dev_flash_*
entry of update_files.tar is an SCE type-3 (PKG) container decrypted with
the public SCEPKG_ERK/SCEPKG_RIV keys; its third section is a tar holding
the files installed to the console's flash filesystems.

Requires pycryptodome.
"""
import io
import struct
import tarfile
import zlib

from Crypto.Cipher import AES
from Crypto.Util import Counter

# Public keys, as published in RPCS3's Crypto/key_vault.h
SCEPKG_ERK = bytes.fromhex(
    'a97818bd193a67a16fe83a855e1be9fb5640938d4dbcb2cb52c5a2f8b02b1031')
SCEPKG_RIV = bytes.fromhex('4acef01224fbeedf8245f8ff10211e6e')

PUP_MAGIC = b'SCEUF\0\0\0'
SEGMENT_VERSION = 0x100
SEGMENT_UPDATE_FILES = 0x300


def aes_ctr(key, iv, data):
    counter = Counter.new(128, initial_value=int.from_bytes(iv, 'big'))
    return AES.new(key, AES.MODE_CTR, counter=counter).decrypt(data)


def pup_segments(path):
    """Return {segment_id: bytes} for a PS3 PUP."""
    with open(path, 'rb') as f:
        if f.read(8) != PUP_MAGIC:
            raise ValueError('not a PS3 PUP (bad magic)')
        _fmt, _imgver, nfiles, _hdrlen, _datalen = struct.unpack('>QQQQQ', f.read(40))
        f.seek(0x30)
        table = [struct.unpack('>QQQQ', f.read(32)) for _ in range(nfiles)]
        segments = {}
        for seg_id, off, size, _ in table:
            f.seek(off)
            segments[seg_id] = f.read(size)
    return segments


def sce_decrypt_sections(blob):
    """Decrypt an SCE container, returning its sections' plaintext."""
    magic, _ver, flags, _type, meta_off, hsize, _esize = struct.unpack_from('>IIHHIQQ', blob, 0)
    if magic != 0x53434500:  # 'SCE\0'
        raise ValueError('not an SCE container')

    info_off = meta_off + 0x20
    meta_info = blob[info_off:info_off + 0x40]
    if not flags & 0x8000:  # 0x8000 marks a debug package, which is not encrypted
        meta_info = AES.new(SCEPKG_ERK, AES.MODE_CBC, SCEPKG_RIV).decrypt(meta_info)
    key, key_pad, iv, iv_pad = (meta_info[0:16], meta_info[16:32],
                                meta_info[32:48], meta_info[48:64])
    if key_pad[0] or iv_pad[0]:
        raise ValueError('failed to decrypt SCE metadata info')

    headers = aes_ctr(key, iv, blob[info_off + 0x40:hsize])
    _siglen, _u1, section_count, key_count, _optsize, _u2, _u3 = \
        struct.unpack_from('>QIIIIII', headers, 0)
    sections = [struct.unpack_from('>QQIIIIIIII', headers, 0x20 + i * 0x30)
                for i in range(section_count)]
    keys_off = 0x20 + section_count * 0x30
    data_keys = headers[keys_off:keys_off + key_count * 0x10]

    out = []
    for (data_off, data_size, _type, _pidx, _hashed, _sha1,
         encrypted, key_idx, iv_idx, compressed) in sections:
        chunk = blob[data_off:data_off + data_size]
        if encrypted == 3 and key_idx < key_count and iv_idx <= key_count:
            chunk = aes_ctr(data_keys[key_idx * 0x10:key_idx * 0x10 + 0x10],
                            data_keys[iv_idx * 0x10:iv_idx * 0x10 + 0x10], chunk)
        if compressed == 2:
            chunk = zlib.decompress(chunk)
        out.append(chunk)
    return out


def flash_trees(pup_path):
    """Return {flash_device: {relative_path: bytes}} of a PUP's flash contents.

    Flash devices are the top-level directories of the update tarballs,
    e.g. 'dev_flash' and 'dev_flash3'.
    """
    segments = pup_segments(pup_path)
    if SEGMENT_UPDATE_FILES not in segments:
        raise ValueError('PUP has no update_files.tar segment')
    update = tarfile.open(fileobj=io.BytesIO(segments[SEGMENT_UPDATE_FILES]))

    trees = {}
    for name in update.getnames():
        if 'dev_flash' not in name:
            continue
        member = update.extractfile(name)
        if member is None:
            continue
        sections = sce_decrypt_sections(member.read())
        if len(sections) < 3:
            raise ValueError(f'{name}: unexpected SCE section count {len(sections)}')
        inner = tarfile.open(fileobj=io.BytesIO(sections[2]))
        for entry in inner.getmembers():
            if not entry.isfile():
                continue
            path = entry.name.lstrip('./')
            device, _, rel = path.partition('/')
            if rel:
                trees.setdefault(device, {})[rel] = inner.extractfile(entry).read()
    return trees


def pup_version(pup_path):
    """Return the firmware version string a PUP declares, e.g. '4.91'."""
    raw = pup_segments(pup_path).get(SEGMENT_VERSION, b'')
    return raw.decode('ascii', 'replace').split('\n')[0].strip()
