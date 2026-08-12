"""Parse and stream-decrypt Nintendo Wii U NUS title contents.

Wii U downloadable titles carry an encrypted FST as their first content.
That table names every installed file, its size, offset and backing content.
The helpers here decrypt only the ranges needed for each file and calculate
its hashes without writing a second, decrypted copy of the package.
"""
import hashlib
import os
import struct

from Crypto.Cipher import AES


SIG_PAYLOAD = {
    0x010000: 0x240, 0x010001: 0x140, 0x010002: 0x80,
    0x010003: 0x240, 0x010004: 0x140, 0x010005: 0x80,
}
CONTENT_INFO_RECORDS = 64 * 36
COMMON_KEY = bytes.fromhex('d7b00402659ba2abd2cb0db27fa2b656')
HASHED = 0x0002
FST_MAGIC = b'FST\0'


class Tmd:
    def __init__(self, blob):
        self.blob = blob
        try:
            base = SIG_PAYLOAD[struct.unpack_from('>I', blob, 0)[0]]
        except (KeyError, struct.error) as e:
            raise ValueError('invalid TMD signature') from e
        if base + 0xC4 + CONTENT_INFO_RECORDS > len(blob):
            raise ValueError('truncated TMD')
        self.format_version = blob[base + 0x40]
        self.title_id, = struct.unpack_from('>Q', blob, base + 0x4C)
        self.title_version, count = struct.unpack_from('>HH', blob, base + 0x9C)
        off = base + 0xC4 + CONTENT_INFO_RECORDS
        if off + count * 48 > len(blob):
            raise ValueError('truncated TMD content records')
        self.contents = []
        for i in range(count):
            cid, index, ctype, size = struct.unpack_from('>IHHQ', blob, off + i * 48)
            digest = blob[off + i * 48 + 16:off + i * 48 + 48]
            self.contents.append({
                'id': cid, 'index': index, 'type': ctype,
                # Wii U records reserve 32 bytes here, but retail NUS titles
                # use a SHA-1 followed by twelve zero bytes.
                'size': size, 'hash': digest,
            })


def load_tmd(path):
    with open(path, 'rb') as f:
        return Tmd(f.read())


def title_key(ticket_path, title_id):
    with open(ticket_path, 'rb') as f:
        ticket = f.read()
    if len(ticket) < 0x1CF:
        raise ValueError(f'{ticket_path}: truncated ticket')
    encrypted = ticket[0x1BF:0x1CF]
    iv = struct.pack('>Q', title_id) + b'\0' * 8
    return AES.new(COMMON_KEY, AES.MODE_CBC, iv).decrypt(encrypted)


def content_path(cache_dir, tid, version, content):
    return os.path.join(cache_dir, 'titles', tid, str(version),
                        f'{content["id"]:08x}.app')


def _decrypt_plain_whole(path, key, index, size):
    with open(path, 'rb') as f:
        blob = f.read((size + 15) & ~15)
    if len(blob) < size or len(blob) % 16:
        raise ValueError(f'{path}: truncated encrypted content')
    iv = struct.pack('>H', index) + b'\0' * 14
    return AES.new(key, AES.MODE_CBC, iv).decrypt(blob)[:size]


def _verify_content(cache_dir, tid, version, content, key):
    """Verify a complete content against the hash Nintendo signed in its TMD."""
    path = content_path(cache_dir, tid, version, content)
    expected = content['hash'][:20]
    if content['type'] & HASHED:
        h3_path = path[:-4] + '.h3'
        with open(h3_path, 'rb') as f:
            h3 = f.read()
        if hashlib.sha1(h3).digest() != expected:
            raise ValueError(f'{h3_path}: H3 root hash mismatch')
        h0_i = h1_i = h2_i = h3_i = 0
        with open(path, 'rb') as f:
            for block in range(content['size'] // 0x10000):
                encrypted = f.read(0x10000)
                if len(encrypted) != 0x10000:
                    raise ValueError(f'{path}: truncated hashed block {block}')
                hashes = AES.new(key, AES.MODE_CBC, b'\0' * 16).decrypt(encrypted[:0x400])
                h0s, h1s, h2s = hashes[:0x140], hashes[0x140:0x280], hashes[0x280:0x3C0]
                h0 = h0s[h0_i * 20:(h0_i + 1) * 20]
                h1 = h1s[h1_i * 20:(h1_i + 1) * 20]
                h2 = h2s[h2_i * 20:(h2_i + 1) * 20]
                h3_hash = h3[h3_i * 20:(h3_i + 1) * 20]
                if hashlib.sha1(h0s).digest() != h1:
                    raise ValueError(f'{path}: H1 mismatch in block {block}')
                if hashlib.sha1(h1s).digest() != h2:
                    raise ValueError(f'{path}: H2 mismatch in block {block}')
                if hashlib.sha1(h2s).digest() != h3_hash:
                    raise ValueError(f'{path}: H3 mismatch in block {block}')
                plain = AES.new(key, AES.MODE_CBC, h0[:16]).decrypt(encrypted[0x400:])
                if hashlib.sha1(plain).digest() != h0:
                    raise ValueError(f'{path}: data hash mismatch in block {block}')
                h0_i += 1
                if h0_i == 16:
                    h0_i, h1_i = 0, h1_i + 1
                if h1_i == 16:
                    h1_i, h2_i = 0, h2_i + 1
                if h2_i == 16:
                    h2_i, h3_i = 0, h3_i + 1
        return

    digest = hashlib.sha1()
    left = content['size']
    iv = struct.pack('>H', content['index']) + b'\0' * 14
    cipher = AES.new(key, AES.MODE_CBC, iv)
    with open(path, 'rb') as f:
        while left:
            encrypted = f.read(min(8 * 1024 * 1024, (left + 15) & ~15))
            if not encrypted or len(encrypted) % 16:
                raise ValueError(f'{path}: truncated encrypted content')
            plain = cipher.decrypt(encrypted)
            take = min(left, len(plain))
            digest.update(plain[:take])
            left -= take
    if digest.digest() != expected:
        raise ValueError(f'{path}: signed content hash mismatch')


class Fst:
    def __init__(self, blob):
        if blob[:4] != FST_MAGIC or len(blob) < 0x20:
            raise ValueError('content has no Wii U FST')
        self.factor, cluster_count = struct.unpack_from('>II', blob, 4)
        self.factor = self.factor or 1
        root = 0x20 + cluster_count * 0x20
        if root + 16 > len(blob):
            raise ValueError('truncated FST root')
        total, = struct.unpack_from('>I', blob, root + 8)
        if not 1 <= total <= 1_000_000:
            raise ValueError(f'invalid FST entry count {total}')
        names = root + total * 16
        if names > len(blob):
            raise ValueError('truncated FST entries')
        self.entries = []
        for i in range(total):
            off = root + i * 16
            type_name, offset, length, flags, content = struct.unpack_from('>IIIHH', blob, off)
            name_off = type_name & 0xFFFFFF
            start = names + name_off
            end = blob.find(b'\0', start)
            if not names <= start < len(blob) or end < 0:
                raise ValueError(f'invalid FST name on entry {i}')
            self.entries.append({
                'type': type_name >> 24, 'name': blob[start:end].decode('utf-8'),
                'offset': offset, 'length': length, 'flags': flags,
                'content': content,
            })

    def walk(self):
        """Yield (relative path, entry), including empty directories."""
        stack = []
        for i, entry in enumerate(self.entries[1:], 1):
            while stack and i >= stack[-1][0]:
                stack.pop()
            path = '/'.join([x[1] for x in stack] + [entry['name']])
            if entry['type'] & 1:
                stack.append((entry['length'], entry['name']))
            yield path, entry


def _plain_range(path, key, content_id, offset, size):
    """Yield a file range from an ordinary content in 0x8000 units."""
    first = offset // 0x8000
    sub = offset % 0x8000
    left = size
    cipher = AES.new(key, AES.MODE_ECB)
    with open(path, 'rb') as f:
        block = first
        while left:
            f.seek(block * 0x8000)
            encrypted = f.read(0x8000)
            if not encrypted or len(encrypted) % 16:
                raise ValueError(f'{path}: truncated content block {block}')
            iv = bytes((0, content_id & 0xFF)) + b'\0' * 14
            plain = AES.new(key, AES.MODE_CBC, iv).decrypt(encrypted)
            take = min(left, len(plain) - sub)
            if take <= 0:
                raise ValueError(f'{path}: range past end of content')
            yield plain[sub:sub + take]
            left -= take
            sub = 0
            block += 1


def _hashed_range(path, key, content_id, offset, size):
    """Yield a logical file range from 0x10000 hash-tree content blocks."""
    first = offset // 0xFC00
    sub = offset % 0xFC00
    left = size
    with open(path, 'rb') as f:
        block = first
        while left:
            f.seek(block * 0x10000)
            encrypted = f.read(0x10000)
            if len(encrypted) != 0x10000:
                raise ValueError(f'{path}: truncated hashed block {block}')
            iv = bytes((0, content_id & 0xFF)) + b'\0' * 14
            hashes = AES.new(key, AES.MODE_CBC, iv).decrypt(encrypted[:0x400])
            h0_off = (block & 0xF) * 20
            expect = bytearray(hashes[h0_off:h0_off + 20])
            data_iv = bytearray(hashes[h0_off:h0_off + 16])
            if not (block & 0xF):
                data_iv[1] ^= content_id & 0xFF
            plain = AES.new(key, AES.MODE_CBC, bytes(data_iv)).decrypt(encrypted[0x400:])
            actual = bytearray(hashlib.sha1(plain).digest())
            if not (block & 0xF):
                actual[1] ^= content_id & 0xFF
            if actual != expect:
                raise ValueError(f'{path}: H0 mismatch in block {block}')
            take = min(left, 0xFC00 - sub)
            yield plain[sub:sub + take]
            left -= take
            sub = 0
            block += 1


def installed_entries(cache_dir, tid, version):
    """Return (directories, {path: (size, sha256)}) for one MLC title."""
    meta = os.path.join(cache_dir, 'metadata', f'{tid}.{version}.tmd')
    ticket = os.path.join(cache_dir, 'tickets', f'{tid}.tik')
    tmd = load_tmd(meta)
    if f'{tmd.title_id:016x}' != tid or tmd.title_version != version:
        raise ValueError(f'{meta}: title id/version mismatch')
    key = title_key(ticket, tmd.title_id)
    for content in tmd.contents:
        _verify_content(cache_dir, tid, version, content, key)
    fst_content = tmd.contents[0]
    fst_blob = _decrypt_plain_whole(
        content_path(cache_dir, tid, version, fst_content), key,
        fst_content['index'], fst_content['size'])
    fst = Fst(fst_blob)
    directories, files = set(), {}
    for path, entry in fst.walk():
        if entry['type'] & 1:
            directories.add(path)
            continue
        if entry['type'] & 0x80:
            continue
        try:
            content = tmd.contents[entry['content']]
        except IndexError as e:
            raise ValueError(f'{meta}: invalid content index {entry["content"]}') from e
        offset = entry['offset']
        if not (entry['flags'] & 0x04):
            offset *= fst.factor
        source = content_path(cache_dir, tid, version, content)
        reader = (_hashed_range if content['type'] & HASHED else _plain_range)
        digest = hashlib.sha256()
        seen = 0
        for chunk in reader(source, key, entry['content'], offset, entry['length']):
            digest.update(chunk)
            seen += len(chunk)
        if seen != entry['length']:
            raise ValueError(f'{source}: extracted {seen}, expected {entry["length"]}')
        files[path] = (entry['length'], digest.hexdigest())
    return directories, files
