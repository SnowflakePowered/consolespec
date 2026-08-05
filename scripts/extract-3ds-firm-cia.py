#!/usr/bin/env python3
"""Extract 3DS FIRM binaries from firmware update zips (CIA collections).

NUS prunes older content, so FIRM binaries for early firmware versions can no
longer be fetched from Nintendo. Archived firmware update packs carry the same
titles as CIAs, which embed their own ticket, TMD and content — everything
needed to recover them offline.

Each 0004013?00000?0? CIA is parsed, its content decrypted with the title key
from the embedded ticket, the NCCH ExeFS decrypted with the 0x2C keyslot, and
the `.firm` entry extracted. Every content is checked against the SHA-256 in
its own TMD before use.

Usage:
    extract-3ds-firm-cia.py --keys aes_keys.txt <zip-dir> <output-dir>

Requires pycryptodome.
"""
import argparse
import hashlib
import os
import struct
import sys
import zipfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ctrtmd  # noqa: E402
from importlib import import_module  # noqa: E402

firm = import_module('extract-3ds-firm')


def align(n, boundary=64):
    return (n + boundary - 1) & ~(boundary - 1)


def parse_cia(blob):
    """Return (ticket, tmd, content_bytes) from a CIA."""
    header_size, _type, _ver, cert_size, ticket_size, tmd_size, meta_size = \
        struct.unpack_from('<IHHIIII', blob, 0)
    content_size, = struct.unpack_from('<Q', blob, 0x18)
    off = align(header_size)
    off += align(cert_size)
    ticket = blob[off:off + ticket_size]
    off += align(ticket_size)
    tmd = blob[off:off + tmd_size]
    off += align(tmd_size)
    content = blob[off:off + content_size]
    return ticket, tmd, content


def title_key_from_ticket(ticket):
    from Crypto.Cipher import AES
    sig_type, = struct.unpack_from('>I', ticket, 0)
    base = firm.SIG_PAYLOAD[sig_type]
    encrypted = ticket[base + 0x7F:base + 0x8F]
    tid, = struct.unpack_from('>Q', ticket, base + 0x9C)
    common = bytes.fromhex(firm.COMMON_KEYS[ticket[base + 0xB1]])
    return AES.new(common, AES.MODE_CBC,
                   struct.pack('>Q', tid) + b'\0' * 8).decrypt(encrypted)


def extract(keys, blob):
    """Return (title_id, title_version, firm_bytes) for one FIRM CIA."""
    from Crypto.Cipher import AES
    from Crypto.Util import Counter
    ticket, tmd_blob, content = parse_cia(blob)
    tmd = ctrtmd.Tmd(tmd_blob)
    record = tmd.contents[0]
    key = title_key_from_ticket(ticket)
    iv = struct.pack('>H', record['index']) + b'\0' * 14
    app = AES.new(key, AES.MODE_CBC, iv).decrypt(content[:align(record['size'], 16)])
    app = app[:record['size']]
    if hashlib.sha256(app).hexdigest() != record['sha256']:
        raise ValueError('content does not match the TMD hash')
    if app[0x100:0x104] != b'NCCH':
        raise ValueError('content is not an NCCH')
    exefs_off, _ = struct.unpack_from('<II', app, 0x1A0)
    base = exefs_off * 0x200
    flags = app[0x188:0x190]
    if flags[7] & 0x04:
        exefs = app[base:]
    else:
        nk = firm.ncch_key(keys, int.from_bytes(app[0:16], 'big'))
        counter = app[0x108:0x110][::-1] + bytes([2]) + b'\0' * 7
        ctr = Counter.new(128, initial_value=int.from_bytes(counter, 'big'))
        exefs = AES.new(nk, AES.MODE_CTR, counter=ctr).decrypt(app[base:])
    for i in range(10):
        if exefs[i * 16:i * 16 + 8].rstrip(b'\0') == b'.firm':
            off, size = struct.unpack_from('<II', exefs, i * 16 + 8)
            out = exefs[0x200 + off:0x200 + off + size]
            if out[:4] != b'FIRM':
                raise ValueError(f'extracted .firm has magic {out[:4]!r}')
            return tmd.title_id, tmd.title_version, out
    raise ValueError('no .firm entry in ExeFS')


def main():
    ap = argparse.ArgumentParser(add_help=False)
    ap.add_argument('--keys')
    ap.add_argument('zip_dir', nargs='?')
    ap.add_argument('output_dir', nargs='?')
    args = ap.parse_args()
    if not args.keys or not args.zip_dir or not args.output_dir:
        print(__doc__, file=sys.stderr)
        return 2
    keys = firm.read_keys(args.keys)
    os.makedirs(args.output_dir, exist_ok=True)

    new, existing, failed = 0, 0, 0
    for name in sorted(os.listdir(args.zip_dir)):
        if not name.lower().endswith('.zip'):
            continue
        with zipfile.ZipFile(os.path.join(args.zip_dir, name)) as z:
            for member in sorted(z.namelist()):
                base = os.path.basename(member).upper()
                if not base.endswith('.CIA') or not base.startswith('00040138'):
                    continue
                try:
                    tid, version, data = extract(keys, z.read(member))
                except Exception as e:  # noqa: BLE001
                    print(f'FAIL {name}:{base}: {e}', file=sys.stderr)
                    failed += 1
                    continue
                label = firm.FIRM_NAMES.get(tid, f'{tid:016X}')
                out = os.path.join(args.output_dir, f'{label}_v{version}.firm')
                if os.path.exists(out):
                    existing += 1
                    continue
                with open(out, 'wb') as f:
                    f.write(data)
                print(f'ok   {label} v{version}: {len(data)} bytes  (from {name})')
                new += 1
    print(f'{new} new FIRM binaries, {existing} already present'
          + (f', {failed} failed' if failed else ''))
    return 0 if not failed else 1


if __name__ == '__main__':
    sys.exit(main())
