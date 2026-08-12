#!/usr/bin/env python3
"""Verify Nintendo 3DS firmware mtrees.

Two levels of checking:

Metadata (default) re-derives each mtree from the signed TMDs it was built
from and compares entry for entry, so drift between the TMDs and the
manifests is caught:

    verify-3ds-mtrees.py --system {ctr,ktr} <mtree-root> <csv-dir> <tmd-dir>

Content (--content N) additionally proves the manifests describe real data:
for N randomly chosen entries it downloads the content from NUS, decrypts it
with the ticket's title key, and checks the SHA-256 against the mtree. This
is the end-to-end check that the TMD hashes match what Nintendo actually
serves. It needs network access and pycryptodome.

Exits non-zero if any check fails.
"""
import argparse
import hashlib
import os
import struct
import sys
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ctrtmd  # noqa: E402
from importlib import import_module  # noqa: E402

gen = import_module('gen-3ds-mtrees')

NUS = 'http://ccs.cdn.c.shop.nintendowifi.net/ccs/download'
# 3DS retail common keys, indexed by the ticket's common key index
COMMON_KEYS = [
    '64c5fd55dd3ad988325baaec5243db98', '4aaa3d0e27d4d728d0b1b433f0f9cbc8',
    'fbb0ef8cdbb0d8e453cd99344371697f', '25959b7ad0409f72684198ba2ecd7dc6',
    '7ada22caffc476cc8297a0c7ceeeeebe', 'a5051ca1b37dcf3afbcf8cc1edd9ce02',
]
SIG_PAYLOAD = {0x010003: 0x240, 0x010004: 0x140, 0x010005: 0x80}


def parse_mtree(path):
    files = {}
    for line in open(path):
        line = line.strip()
        if not line or line.startswith('#'):
            continue
        name, *keywords = line.split()
        kv = dict(k.split('=', 1) for k in keywords if '=' in k)
        if kv.get('type') == 'file':
            files[name[2:] if name.startswith('./') else name] = kv
    return files


def fetch(url):
    with urllib.request.urlopen(url, timeout=120) as r:
        return r.read()


def verify_content(title_id, content_id, index):
    """Download and decrypt one content, returning its SHA-256."""
    from Crypto.Cipher import AES
    cetk = fetch(f'{NUS}/{title_id}/cetk')
    sig_type, = struct.unpack_from('>I', cetk, 0)
    base = SIG_PAYLOAD[sig_type]
    encrypted = cetk[base + 0x7F:base + 0x8F]
    tid, = struct.unpack_from('>Q', cetk, base + 0x9C)
    key_index = cetk[base + 0xB1]
    common = bytes.fromhex(COMMON_KEYS[key_index])
    title_key = AES.new(common, AES.MODE_CBC,
                        struct.pack('>Q', tid) + b'\0' * 8).decrypt(encrypted)
    blob = fetch(f'{NUS}/{title_id}/{content_id}')
    iv = struct.pack('>H', index) + b'\0' * 14
    plain = AES.new(title_key, AES.MODE_CBC, iv).decrypt(blob)
    return hashlib.sha256(plain).hexdigest()


def main():
    ap = argparse.ArgumentParser(add_help=False)
    ap.add_argument('--system', choices=('ctr', 'ktr'), required=False)
    ap.add_argument('--content', type=int, default=0)
    ap.add_argument('mtree_root', nargs='?')
    ap.add_argument('csv_dir', nargs='?')
    ap.add_argument('tmd_dir', nargs='?')
    args = ap.parse_args()
    if not args.system or not args.mtree_root or not args.csv_dir or not args.tmd_dir:
        print(__doc__, file=sys.stderr)
        return 2

    rows = gen.read_titlelist(args.csv_dir, args.system)
    sets = gen.title_sets(rows)

    cache = {}
    ok = failed = 0
    entries_by_label = {}
    for (firmware, region), state in sets.items():
        per_partition = {}
        for title, version in state.items():
            path = os.path.join(args.tmd_dir, f'{title}.{version}.tmd')
            if path not in cache:
                try:
                    cache[path] = ctrtmd.load(path)
                except (OSError, ValueError):
                    cache[path] = None
            tmd = cache[path]
            if tmd:
                per_partition.setdefault(tmd.partition, {}).update(tmd.app_files())
        entries_by_label[f'{firmware}_{region}'] = per_partition

    # mtrees live at <root>/<device>/<partition>.mtree when a partition's
    # content never varies, or <root>/<device>/<partition>/<firmware>.mtree
    # when it does. Either way each one is rooted at its partition, and a
    # single file may stand for several firmwares (named in its header).
    def mtree_files():
        for device in sorted(d for d in os.listdir(args.mtree_root)
                             if os.path.isdir(os.path.join(args.mtree_root, d))):
            device_dir = os.path.join(args.mtree_root, device)
            for entry in sorted(os.listdir(device_dir)):
                path = os.path.join(device_dir, entry)
                if entry.endswith('.mtree'):
                    yield device, entry[:-len('.mtree')], path, None
                elif os.path.isdir(path):
                    for name in sorted(os.listdir(path)):
                        if name.endswith('.mtree'):
                            yield device, entry, os.path.join(path, name), name[:-len('.mtree')]

    skeleton_files = set(gen.CTRNAND_FILES) | set(gen.CTRNAND_FILES_UNSIZED)
    skeleton = {p.partition('/')[2]: p for p in skeleton_files if '/' in p}
    for device, partition, path, label in mtree_files():
        want = parse_mtree(path)
        failures = []
        if label is None:
            # static partition: every firmware must agree, so check them all
            labels = sorted(entries_by_label)
        else:
            labels = [label]
        for one in labels:
            entries = entries_by_label.get(one, {}).get(device, {})
            have = {p.partition('/')[2]: v for p, v in entries.items()
                    if p.partition('/')[0] == partition}
            for rel, kv in sorted(want.items()):
                full = skeleton.get(rel)
                if full:
                    # console-unique files: check the recorded size where the
                    # generator gives one, and never expect a digest
                    expect = gen.CTRNAND_FILES.get(full)
                    if expect is not None and int(kv.get('size', -1)) != expect:
                        failures.append(f'{rel}: size {kv.get("size")} != {expect}')
                    if kv.get('sha256'):
                        failures.append(f'{rel}: has a digest but is console-unique')
                    continue
                if rel not in have:
                    if label is not None:
                        failures.append(f'not derivable from TMDs: {rel}')
                    continue
                size, sha256 = have[rel]
                if int(kv.get('size', -1)) != size:
                    failures.append(f'{rel}: size {kv.get("size")} != {size}')
                if kv.get('sha256') != sha256:
                    failures.append(f'{rel}: sha256 {kv.get("sha256")} != {sha256}')
            if label is not None:
                for rel in sorted(set(have) - set(want)):
                    failures.append(f'missing from mtree: {rel}')
        if failures:
            print(f'FAIL {device}/{partition}' + (f'/{label}' if label else ''))
            for f in failures[:10]:
                print(f'     {f}')
            failed += 1
        else:
            ok += 1
    print(f'metadata: {ok} ok, {failed} failed')

    if args.content:
        import random
        pool = []
        for tmd in cache.values():
            if not tmd:
                continue
            for c in tmd.contents:
                pool.append((f'{tmd.title_id:016X}', c['id'], c['index'],
                             c['size'], c['sha256']))
        pool.sort()
        random.seed(0)
        sample = random.sample(pool, min(args.content, len(pool)))
        good = bad = 0
        for title_id, cid, index, size, sha256 in sample:
            try:
                got = verify_content(title_id, f'{cid:08x}', index)
            except Exception as e:  # noqa: BLE001
                print(f'FAIL content {title_id}/{cid:08x}: {e}')
                bad += 1
                continue
            if got == sha256:
                print(f'ok   content {title_id}/{cid:08x} ({size} bytes)')
                good += 1
            else:
                print(f'FAIL content {title_id}/{cid:08x}: {got} != {sha256}')
                bad += 1
        print(f'content: {good} ok, {bad} failed')
        failed += bad
    return 0 if not failed else 1


if __name__ == '__main__':
    sys.exit(main())
