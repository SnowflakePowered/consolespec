#!/usr/bin/env python3
"""Extract 3DS FIRM binaries from NUS and emit machinespec bios entries.

The firm0 and firm1 NAND partitions each hold one raw FIRM binary rather than
a filesystem, so they are described as bios entries rather than mtrees — the
same treatment the DSi's stage2 bootloader gets.

Each FIRM title (NATIVE_FIRM, SAFE_MODE_FIRM, TWL_FIRM, AGB_FIRM) is fetched
from NUS, decrypted with the ticket's title key, its NCCH ExeFS decrypted with
the 0x2C keyslot, and the `.firm` entry extracted. Every step is checked: the
content against the SHA-256 Nintendo signed into the TMD, and the extracted
binary against the FIRM magic.

NUS prunes older content, so only versions it still serves can be extracted;
the rest are reported and skipped.

Usage:
    extract-3ds-firm.py --keys aes_keys.txt <tmd-dir> <output-dir>

The key file is the usual `name=hex` list; slot0x2CKeyX and generatorConstant
are the two entries used here. Requires pycryptodome and network access.
"""
import argparse
import hashlib
import os
import re
import struct
import sys
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ctrtmd  # noqa: E402

NUS = 'http://ccs.cdn.c.shop.nintendowifi.net/ccs/download'
COMMON_KEYS = [
    '64c5fd55dd3ad988325baaec5243db98', '4aaa3d0e27d4d728d0b1b433f0f9cbc8',
    'fbb0ef8cdbb0d8e453cd99344371697f', '25959b7ad0409f72684198ba2ecd7dc6',
    '7ada22caffc476cc8297a0c7ceeeeebe', 'a5051ca1b37dcf3afbcf8cc1edd9ce02',
]
SIG_PAYLOAD = {0x010003: 0x240, 0x010004: 0x140, 0x010005: 0x80}
FIRM_NAMES = {
    0x0004013800000002: 'NATIVE_FIRM', 0x0004013800000003: 'SAFE_MODE_FIRM',
    0x0004013800000102: 'TWL_FIRM', 0x0004013800000202: 'AGB_FIRM',
    0x0004013820000002: 'NATIVE_FIRM_N3DS', 0x0004013820000003: 'SAFE_MODE_FIRM_N3DS',
    0x0004013820000102: 'TWL_FIRM_N3DS', 0x0004013820000202: 'AGB_FIRM_N3DS',
}


def read_keys(path):
    keys = {}
    for line in open(path):
        if '=' in line and not line.startswith('#'):
            name, value = line.strip().split('=', 1)
            if re.fullmatch(r'[0-9A-Fa-f]{32}', value):
                keys[name] = int(value, 16)
    for required in ('slot0x2CKeyX', 'generatorConstant'):
        if required not in keys:
            raise ValueError(f'key file is missing {required}')
    return keys


def rol(value, n, width=128):
    n %= width
    return ((value << n) | (value >> (width - n))) & ((1 << width) - 1)


def ncch_key(keys, key_y):
    """3DS key scrambler: rol((rol(KeyX, 2) ^ KeyY) + C, 87)."""
    return rol((rol(keys['slot0x2CKeyX'], 2) ^ key_y) + keys['generatorConstant'],
               87).to_bytes(16, 'big')


def fetch(url, timeout=300):
    with urllib.request.urlopen(url, timeout=timeout) as r:
        return r.read()


def title_key(title_id):
    from Crypto.Cipher import AES
    cetk = fetch(f'{NUS}/{title_id}/cetk', timeout=90)
    sig_type, = struct.unpack_from('>I', cetk, 0)
    base = SIG_PAYLOAD[sig_type]
    tid, = struct.unpack_from('>Q', cetk, base + 0x9C)
    common = bytes.fromhex(COMMON_KEYS[cetk[base + 0xB1]])
    return AES.new(common, AES.MODE_CBC,
                   struct.pack('>Q', tid) + b'\0' * 8).decrypt(cetk[base + 0x7F:base + 0x8F])


def extract_firm(keys, title_id, content, key):
    """Return the raw FIRM binary for one content, or raise."""
    from Crypto.Cipher import AES
    from Crypto.Util import Counter
    blob = fetch(f'{NUS}/{title_id}/{content["id"]:08x}')
    iv = struct.pack('>H', content['index']) + b'\0' * 14
    app = AES.new(key, AES.MODE_CBC, iv).decrypt(blob)[:content['size']]
    if hashlib.sha256(app).hexdigest() != content['sha256']:
        raise ValueError('content does not match the TMD hash')
    if app[0x100:0x104] != b'NCCH':
        raise ValueError('content is not an NCCH')

    exefs_off, _exefs_pages = struct.unpack_from('<II', app, 0x1A0)
    base = exefs_off * 0x200
    counter = app[0x108:0x110][::-1] + bytes([2]) + b'\0' * 7
    flags = app[0x188:0x190]
    if flags[7] & 0x04:  # NoCrypto
        exefs = app[base:]
    else:
        nk = ncch_key(keys, int.from_bytes(app[0:16], 'big'))
        ctr = Counter.new(128, initial_value=int.from_bytes(counter, 'big'))
        exefs = AES.new(nk, AES.MODE_CTR, counter=ctr).decrypt(app[base:])
    for i in range(10):
        name = exefs[i * 16:i * 16 + 8].rstrip(b'\0')
        if name == b'.firm':
            off, size = struct.unpack_from('<II', exefs, i * 16 + 8)
            firm = exefs[0x200 + off:0x200 + off + size]
            if firm[:4] != b'FIRM':
                raise ValueError(f'extracted .firm has magic {firm[:4]!r}')
            return firm
    raise ValueError('no .firm entry in ExeFS')


def main():
    ap = argparse.ArgumentParser(add_help=False)
    ap.add_argument('--keys', required=False)
    ap.add_argument('tmd_dir', nargs='?')
    ap.add_argument('output_dir', nargs='?')
    args = ap.parse_args()
    if not args.keys or not args.tmd_dir or not args.output_dir:
        print(__doc__, file=sys.stderr)
        return 2
    keys = read_keys(args.keys)
    os.makedirs(args.output_dir, exist_ok=True)

    tmds = []
    for name in sorted(os.listdir(args.tmd_dir)):
        if not name.endswith('.tmd') or name[:8] != '00040138':
            continue
        try:
            tmds.append(ctrtmd.load(os.path.join(args.tmd_dir, name)))
        except (OSError, ValueError):
            pass

    got, pruned, failed = [], 0, 0
    cached_key = {}
    for tmd in sorted(tmds, key=lambda t: (t.title_id, t.title_version)):
        label = FIRM_NAMES.get(tmd.title_id, f'{tmd.title_id:016X}')
        tid = f'{tmd.title_id:016X}'
        for content in tmd.contents:
            out = os.path.join(args.output_dir, f'{label}_v{tmd.title_version}.firm')
            if os.path.exists(out):
                firm = open(out, 'rb').read()
            else:
                try:
                    if tid not in cached_key:
                        cached_key[tid] = title_key(tid)
                    firm = extract_firm(keys, tid, content, cached_key[tid])
                except urllib.error.HTTPError as e:
                    if e.code == 404:
                        pruned += 1
                        continue
                    print(f'FAIL {label} v{tmd.title_version}: {e}', file=sys.stderr)
                    failed += 1
                    continue
                except Exception as e:  # noqa: BLE001
                    print(f'FAIL {label} v{tmd.title_version}: {e}', file=sys.stderr)
                    failed += 1
                    continue
                with open(out, 'wb') as f:
                    f.write(firm)
            got.append((os.path.basename(out), firm))
            print(f'ok   {label} v{tmd.title_version}: {len(firm)} bytes')

    entries = []
    for name, firm in got:
        entries.append(
            '[[bios]]\n'
            f'name = "{name}"\n'
            f'md5 = ["{hashlib.md5(firm).hexdigest()}"]\n'
            f'sha1 = ["{hashlib.sha1(firm).hexdigest()}"]\n'
            f'sha256 = ["{hashlib.sha256(firm).hexdigest()}"]\n')
    toml = os.path.join(args.output_dir, 'bios-entries.toml')
    with open(toml, 'w') as f:
        f.write('\n'.join(entries))
    print(f'{len(got)} FIRM binaries extracted, {pruned} no longer served by NUS'
          + (f', {failed} failed' if failed else ''))
    print(f'bios entries written to {toml}')
    return 0 if got and not failed else 1


if __name__ == '__main__':
    sys.exit(main())
