"""Nintendo DSi TAD (title archive) parsing and content decryption.

A TAD is the DSi's installable title archive — the same container layout as
the Wii's WAD: a header naming the size of each section, followed by the
certificate chain, ticket, TMD, and the title's contents, each padded to a
64-byte boundary.

Contents are stored encrypted exactly as Nintendo distributes them. The
ticket carries the title key encrypted with the DSi common key; the content
is then AES-128-CBC encrypted under that title key with the content index as
the IV. Every decrypted content is checked against the SHA-1 recorded in the
TMD, which Nintendo signs, so a successful decrypt is self-verifying.

The DSi common key is public and is what every DSi tool uses.
"""
import hashlib
import struct

from Crypto.Cipher import AES

COMMON_KEY = bytes.fromhex('af1bf516a807d21aea45984f04742861')

# offset of a signed blob's payload, by signature type
SIG_PAYLOAD = {0x10000: 0x240, 0x10001: 0x140, 0x10002: 0x80}


def _align(n, boundary=64):
    return (n + boundary - 1) & ~(boundary - 1)


class Tmd:
    """A title metadata blob."""

    def __init__(self, blob):
        self.blob = blob
        self.title_id, = struct.unpack_from('>Q', blob, 0x18C)
        self.title_version, count, self.boot_index = struct.unpack_from('>HHH', blob, 0x1DC)
        self.contents = []
        for i in range(count):
            off = 0x1E4 + i * 36
            cid, index, ctype, size = struct.unpack_from('>IHHQ', blob, off)
            self.contents.append({
                'id': cid, 'index': index, 'type': ctype, 'size': size,
                'sha1': blob[off + 16:off + 36],
            })


class Tad:
    """A parsed TAD archive."""

    def __init__(self, data):
        self.data = data
        header_size, self.type = struct.unpack_from('>I4s', data, 0)
        sizes = struct.unpack_from('>IIIIII', data, 8)
        certs, crl, ticket, tmd, content, footer = sizes
        off = _align(header_size)
        section = {}
        for name, size in (('certs', certs), ('crl', crl), ('ticket', ticket),
                           ('tmd', tmd), ('content', content), ('footer', footer)):
            section[name] = (off, size)
            off += _align(size)
        self.section = section
        self.tmd = Tmd(self._slice('tmd'))
        self.ticket = self._slice('ticket')

    def _slice(self, name):
        off, size = self.section[name]
        return self.data[off:off + size]

    @property
    def title_key(self):
        sig_type, = struct.unpack_from('>I', self.ticket, 0)
        base = SIG_PAYLOAD[sig_type]
        encrypted = self.ticket[base + 0x7F:base + 0x8F]
        title_id, = struct.unpack_from('>Q', self.ticket, base + 0x9C)
        iv = struct.pack('>Q', title_id) + b'\0' * 8
        return AES.new(COMMON_KEY, AES.MODE_CBC, iv).decrypt(encrypted)

    def decrypted_contents(self):
        """Yield (content_record, plaintext), verifying each against the TMD.

        Raises ValueError when a content does not match the SHA-1 that
        Nintendo signed into the TMD.
        """
        key = self.title_key
        base, _size = self.section['content']
        off = base
        for record in self.tmd.contents:
            blob = self.data[off:off + _align(record['size'], 16)]
            iv = struct.pack('>H', record['index']) + b'\0' * 14
            plain = AES.new(key, AES.MODE_CBC, iv).decrypt(blob)[:record['size']]
            digest = hashlib.sha1(plain).digest()
            if digest != record['sha1']:
                raise ValueError(
                    f'content 0x{record["id"]:08x} of title '
                    f'0x{self.tmd.title_id:016X}: sha1 {digest.hex()} '
                    f'does not match TMD {record["sha1"].hex()}')
            yield record, plain
            off += _align(record['size'])


def save_sizes(title_id, app):
    """Return (public_size, private_size) from a title's DSi ROM header.

    The sizes are only meaningful when the app really is an SRL whose header
    carries this title's id — wlanfirm and sysmenuVersion are raw blobs whose
    bytes at these offsets are not header fields.
    """
    if len(app) < 0x240:
        return 0, 0
    low = int.from_bytes(app[0x230:0x234], 'little')
    high, public, private = struct.unpack_from('<III', app, 0x234)
    if (high << 32 | low) != title_id:
        return 0, 0
    return public, private


def nand_files(tad):
    """Return {nand_path: bytes} for the files installing a TAD writes.

    A DSi title occupies three places on NAND: its ticket under ticket/, and
    its TMD, contents, and freshly created (zero-filled) save files under
    title/<title id high>/<title id low>/.
    """
    high = (tad.tmd.title_id >> 32) & 0xFFFFFFFF
    low = tad.tmd.title_id & 0xFFFFFFFF
    base = f'title/{high:08x}/{low:08x}'
    files = {
        f'ticket/{high:08x}/{low:08x}.tik': tad.ticket,
        f'{base}/content/title.tmd': tad.tmd.blob,
    }
    for record, plain in tad.decrypted_contents():
        files[f'{base}/content/{record["id"]:08x}.app'] = plain
        if record['index'] == tad.tmd.boot_index:
            public, private = save_sizes(tad.tmd.title_id, plain)
            if public:
                files[f'{base}/data/public.sav'] = b'\0' * public
            if private:
                files[f'{base}/data/private.sav'] = b'\0' * private
    return files


def load(path):
    with open(path, 'rb') as f:
        return Tad(f.read())
