"""Nintendo 3DS title metadata (TMD) parsing.

A 3DS TMD is a signed blob describing one version of one title: which
contents it is made of, how large each is, and the SHA-256 of each. That
hash covers the content as it is stored on the console — the NCCH — so a
TMD is an authoritative manifest of a title's installed files without
needing to download or decrypt the contents themselves.
"""
import struct

# offset of a signed blob's payload, by signature type
SIG_PAYLOAD = {
    0x010000: 0x240, 0x010001: 0x140, 0x010002: 0x80,   # RSA-4096/2048/ECDSA + SHA-1
    0x010003: 0x240, 0x010004: 0x140, 0x010005: 0x80,   # ... + SHA-256
}

CONTENT_INFO_RECORDS = 64 * 36


class Tmd:
    def __init__(self, blob):
        self.blob = blob
        sig_type, = struct.unpack_from('>I', blob, 0)
        if sig_type not in SIG_PAYLOAD:
            raise ValueError(f'unknown signature type 0x{sig_type:x}')
        base = SIG_PAYLOAD[sig_type]
        self.issuer = blob[base:base + 64].split(b'\0')[0].decode('ascii', 'replace')
        self.version = blob[base + 0x40]
        self.title_id, = struct.unpack_from('>Q', blob, base + 0x4C)
        self.title_version, count, self.boot_index = struct.unpack_from('>HHH', blob, base + 0x9C)
        # v1 TMDs put a hash of the content info records between the header and
        # the info records themselves; the chunk records follow those.
        chunk = base + 0xC4 + CONTENT_INFO_RECORDS
        if chunk + count * 48 > len(blob):
            raise ValueError('truncated TMD')
        self.contents = []
        for i in range(count):
            off = chunk + i * 48
            cid, index, ctype, size = struct.unpack_from('>IHHQ', blob, off)
            self.contents.append({
                'id': cid, 'index': index, 'type': ctype, 'size': size,
                'sha256': blob[off + 16:off + 48].hex(),
            })

    @property
    def title_high(self):
        return (self.title_id >> 32) & 0xFFFFFFFF

    @property
    def title_low(self):
        return self.title_id & 0xFFFFFFFF

    @property
    def partition(self):
        """NAND partition this title is installed to.

        TWL titles — the DSi-mode system software a 3DS carries — live in the
        TWL_NAND partition, not CTRNAND.
        """
        return 'twln' if (self.title_high >> 16) == 0x0004 and \
            (self.title_high & 0xFFFF) in (0x8005, 0x800F) else 'ctrnand'

    def app_files(self):
        """Return {nand_path: (size, sha256)} for this title's .app files.

        Titles live under title/<id high>/<id low>/content/, with one
        <content id>.app per content.
        """
        base = f'title/{self.title_high:08x}/{self.title_low:08x}/content'
        return {f'{base}/{c["id"]:08x}.app': (c['size'], c['sha256'])
                for c in self.contents}


def load(path):
    with open(path, 'rb') as f:
        return Tmd(f.read())
