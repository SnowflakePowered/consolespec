"""Minimal read-only FAT12/16/32 reader for firmware partition images."""
import struct

class Fat:
    def __init__(self, data):
        self.d = data
        bps, spc, res, nfat = struct.unpack_from('<HBHB', data, 0x0B)
        root_ents, tot16 = struct.unpack_from('<HH', data, 0x11)
        spf16, = struct.unpack_from('<H', data, 0x16)
        tot32, = struct.unpack_from('<I', data, 0x20)
        self.bps, self.spc, self.nfat, self.root_ents = bps, spc, nfat, root_ents
        self.total = tot16 or tot32
        if spf16:
            self.spf, self.root_clus, self.fat32 = spf16, None, False
        else:
            self.spf, = struct.unpack_from('<I', data, 0x24)
            self.root_clus, = struct.unpack_from('<I', data, 0x2C)
            self.fat32 = True
        self.fat_start = res * bps
        self.root_start = self.fat_start + nfat * self.spf * bps
        root_bytes = root_ents * 32
        self.data_start = self.root_start + root_bytes
        clusters = (self.total - (res + nfat * self.spf) - root_bytes // bps) // spc
        self.fat12 = (not self.fat32) and clusters < 4085

    def _fat_entry(self, n):
        if self.fat32:
            off = self.fat_start + n * 4
            return struct.unpack_from('<I', self.d, off)[0] & 0x0FFFFFFF
        if self.fat12:
            off = self.fat_start + n + n // 2
            v = struct.unpack_from('<H', self.d, off)[0]
            return (v >> 4) if (n & 1) else (v & 0xFFF)
        return struct.unpack_from('<H', self.d, self.fat_start + n * 2)[0]

    def _eof(self, n):
        limit = 0x0FFFFFF8 if self.fat32 else (0xFF8 if self.fat12 else 0xFFF8)
        return n >= limit or n < 2

    def _chain(self, start, size=None):
        out, n, guard = bytearray(), start, 0
        csize = self.spc * self.bps
        while not self._eof(n) and guard < 1 << 22:
            off = self.data_start + (n - 2) * csize
            out += self.d[off:off + csize]
            if size is not None and len(out) >= size:
                break
            n = self._fat_entry(n)
            guard += 1
        return bytes(out[:size]) if size is not None else bytes(out)

    def _entries(self, raw):
        out, lfn = [], []
        for i in range(0, len(raw), 32):
            e = raw[i:i + 32]
            if not e or e[0] == 0x00:
                break
            if e[0] == 0xE5:
                lfn = []
                continue
            attr = e[11]
            if attr == 0x0F:
                seq = e[0] & 0x3F
                chars = (e[1:11] + e[14:26] + e[28:32])
                lfn.append((seq, chars.decode('utf-16-le', 'ignore')))
                continue
            if attr & 0x08:  # volume label
                lfn = []
                continue
            if lfn:
                name = ''.join(t for _, t in sorted(lfn)).split('\0')[0]
                lfn = []
            else:
                base = e[0:8].decode('ascii', 'replace').rstrip()
                ext = e[8:11].decode('ascii', 'replace').rstrip()
                name = f'{base}.{ext}' if ext else base
            hi, = struct.unpack_from('<H', e, 20)
            lo, = struct.unpack_from('<H', e, 26)
            size, = struct.unpack_from('<I', e, 28)
            out.append((name, attr, (hi << 16) | lo, size))
        return out

    def walk(self, cluster=None, path=''):
        """Yield (path, size, data_getter) for every file, recursively."""
        if cluster is None and not self.fat32:
            raw = self.d[self.root_start:self.root_start + self.root_ents * 32]
        else:
            raw = self._chain(cluster if cluster else self.root_clus)
        for name, attr, clus, size in self._entries(raw):
            if name in ('.', '..'):
                continue
            full = f'{path}/{name}' if path else name
            if attr & 0x10:
                yield (full, None, None)
                if clus:
                    yield from self.walk(clus, full)
            else:
                yield (full, size, (lambda c=clus, s=size: self._chain(c, s) if c else b''))
