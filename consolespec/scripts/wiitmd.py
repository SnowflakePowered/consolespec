"""Nintendo Wii title metadata (TMD) and ticket parsing.

A Wii TMD is a signed blob describing one version of one title: which
contents it is made of, how large each is, and the SHA-1 of each. The hash
covers the decrypted content, which is exactly what the console writes to
NAND, so a TMD is an authoritative manifest of a title's installed files
without needing to download or decrypt the contents themselves.

The layout matches the 3DS's (see ctrtmd.py) up to the content records: the
Wii has no content info records, so its chunks start right after the header,
and each record carries a 20-byte SHA-1 rather than a 32-byte SHA-256.

NUS serves both blobs with the certificate chain appended. The console
stores only the signed portion, so `signed` and `signed_size` describe the
`title.tmd` and `.tik` files that end up on NAND.
"""
import hashlib
import struct

# offset of a signed blob's payload, by signature type
SIG_PAYLOAD = {
    0x010000: 0x240, 0x010001: 0x140, 0x010002: 0x80,   # RSA-4096/2048/ECDSA + SHA-1
}

CONTENT_SHARED = 0x8000


class Tmd:
    def __init__(self, blob):
        self.blob = blob
        sig_type, = struct.unpack_from('>I', blob, 0)
        if sig_type not in SIG_PAYLOAD:
            raise ValueError(f'unknown signature type 0x{sig_type:x}')
        base = SIG_PAYLOAD[sig_type]
        if base + 0xA4 > len(blob):
            raise ValueError('truncated TMD')
        self.issuer = blob[base:base + 64].split(b'\0')[0].decode('ascii', 'replace')
        self.version = blob[base + 0x40]
        self.ios_version, self.title_id = struct.unpack_from('>QQ', blob, base + 0x44)
        self.title_version, count, self.boot_index = struct.unpack_from('>HHH', blob, base + 0x9C)
        chunk = base + 0xA4
        if chunk + count * 36 > len(blob):
            raise ValueError('truncated TMD')
        self.signed_size = chunk + count * 36
        self.contents = []
        for i in range(count):
            off = chunk + i * 36
            cid, index, ctype, size = struct.unpack_from('>IHHQ', blob, off)
            self.contents.append({
                'id': cid, 'index': index, 'type': ctype, 'size': size,
                'sha1': blob[off + 16:off + 36].hex(),
                'shared': bool(ctype & CONTENT_SHARED),
            })

    @property
    def signed(self):
        """The TMD as stored on NAND: signature through last content record."""
        return self.blob[:self.signed_size]

    @property
    def title_high(self):
        return (self.title_id >> 32) & 0xFFFFFFFF

    @property
    def title_low(self):
        return self.title_id & 0xFFFFFFFF

    @property
    def content_dir(self):
        return f'title/{self.title_high:08x}/{self.title_low:08x}/content'

    def app_files(self):
        """Return {nand_path: (size, sha1)} for this title's private contents.

        Shared contents are excluded: they live in the shared1 partition
        under a sequentially allocated name, not under title/.
        """
        return {f'{self.content_dir}/{c["id"]:08x}.app': (c['size'], c['sha1'])
                for c in self.contents if not c['shared']}

    def tmd_file(self):
        """Return {nand_path: (size, sha1)} for this title's title.tmd."""
        return {f'{self.content_dir}/title.tmd':
                (self.signed_size, hashlib.sha1(self.signed).hexdigest())}

    def shared_contents(self):
        """Return [(size, sha1)] for contents that land in shared1."""
        return [(c['size'], c['sha1']) for c in self.contents if c['shared']]


class Ticket:
    """A Wii ticket, as NUS serves it in the `cetk` blob.

    Unlike the TMD there is nothing variable-length here, so the signed
    portion is always 0x2A4 bytes.
    """

    def __init__(self, blob):
        self.blob = blob
        sig_type, = struct.unpack_from('>I', blob, 0)
        if sig_type not in SIG_PAYLOAD:
            raise ValueError(f'unknown signature type 0x{sig_type:x}')
        base = SIG_PAYLOAD[sig_type]
        self.signed_size = base + 0x164
        if self.signed_size > len(blob):
            raise ValueError('truncated ticket')
        self.issuer = blob[base:base + 64].split(b'\0')[0].decode('ascii', 'replace')
        self.title_id, = struct.unpack_from('>Q', blob, base + 0x9C)

    @property
    def signed(self):
        return self.blob[:self.signed_size]

    @property
    def title_high(self):
        return (self.title_id >> 32) & 0xFFFFFFFF

    @property
    def title_low(self):
        return self.title_id & 0xFFFFFFFF

    def tik_file(self):
        """Return {nand_path: (size, sha1)} for this ticket as stored on NAND."""
        return {f'ticket/{self.title_high:08x}/{self.title_low:08x}.tik':
                (self.signed_size, hashlib.sha1(self.signed).hexdigest())}


def load(path):
    with open(path, 'rb') as f:
        return Tmd(f.read())


def load_ticket(path):
    with open(path, 'rb') as f:
        return Ticket(f.read())
