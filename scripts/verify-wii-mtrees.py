#!/usr/bin/env python3
"""Verify Nintendo Wii system menu mtrees.

Two levels of checking:

Metadata (default) re-derives each mtree from the signed TMDs and tickets it
was built from and compares entry for entry, so drift between the cache and
the manifests is caught:

    verify-wii-mtrees.py [--system {wii,vwii}] <mtree-root>
                         [<nusdownloader.cpp>] <tmd-dir>

Content (--content N) additionally proves the manifests describe real data:
for N randomly chosen entries it downloads the content from NUS, decrypts it
with the ticket's title key, and checks the SHA-1 against the mtree. This is
the end-to-end check that the TMD hashes match what Nintendo actually
serves. It needs network access and pycryptodome.

--system vwii verifies the Wii U's SLCCMPT mtrees instead, from the vWii
table in wiiupd.py; the source file is not needed and must be omitted.

Exits non-zero if any check fails.
"""
import argparse
import hashlib
import os
import struct
import sys
import urllib.request
from importlib import import_module

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
dedupe = import_module('dedupe-mtrees')
import wiitmd  # noqa: E402
import wiiupd  # noqa: E402

gen = import_module('gen-wii-mtrees')

NUS = 'http://nus.cdn.shop.wii.com/ccs/download'
# Wii common keys, indexed by the ticket's common key index. vWii titles are
# served from their own NUS namespace but their tickets carry the ordinary Wii
# title id, which is also the IV the title key is unwrapped with; only the key
# they are wrapped under differs.
COMMON_KEYS = [
    'ebe42a225e8593e448d9c5457381aaf7',   # retail
    '63b82bb4f4614e2e13f2fefbba4c9b7e',   # Korean
    '30bfc76e7c19afbb23163330ced7c28d',   # vWii
]


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
    req = urllib.request.Request(url, headers={'User-Agent': 'wii libnup/1.0'})
    with urllib.request.urlopen(req, timeout=120) as r:
        return r.read()


def title_key(cetk):
    """Decrypt a ticket's title key with the right Wii common key."""
    from Crypto.Cipher import AES
    base = wiitmd.SIG_PAYLOAD[struct.unpack_from('>I', cetk, 0)[0]]
    encrypted = cetk[base + 0x7F:base + 0x8F]
    tid, = struct.unpack_from('>Q', cetk, base + 0x9C)
    common = bytes.fromhex(COMMON_KEYS[cetk[base + 0xB1]])
    return AES.new(common, AES.MODE_CBC,
                   struct.pack('>Q', tid) + b'\0' * 8).decrypt(encrypted)


def verify_content(tid, cetk, cid, index, size):
    """Download and decrypt one content, returning its SHA-1.

    NUS serves the content padded out to the AES block size; the console
    stores and hashes only the `size` bytes the TMD declares.
    """
    from Crypto.Cipher import AES
    blob = fetch(f'{NUS}/{tid:016x}/{cid:08x}')
    iv = struct.pack('>H', index) + b'\0' * 14
    plain = AES.new(title_key(cetk), AES.MODE_CBC, iv).decrypt(blob)
    return hashlib.sha1(plain[:size]).hexdigest()


def derive(updates, tmd_dir):
    """Re-derive {label: {partition: {path: (size, sha1)}}} from the cache."""
    cache, out = {}, {}
    for label, titles in updates.items():
        per_partition = {'title': {}, 'ticket': {}}
        shared_hashes = set()
        for tid, version in titles.items():
            path = gen.tmd_path(tmd_dir, tid, version)
            if path not in cache:
                try:
                    cache[path] = wiitmd.load(path)
                except (OSError, ValueError):
                    cache[path] = None
            if cache[path]:
                per_partition['title'].update(cache[path].app_files())
                per_partition['title'].update(cache[path].tmd_file())
                shared_hashes.update(h for _, h in cache[path].shared_contents())
            path = gen.tik_path(tmd_dir, tid, version)
            if path not in cache:
                try:
                    cache[path] = wiitmd.load_ticket(path)
                except (OSError, ValueError):
                    cache[path] = None
            if cache[path]:
                per_partition['ticket'].update(cache[path].tik_file())
        per_partition['shared1'] = gen.shared1_entries(sorted(shared_hashes))
        out[label] = {part: gen.rooted(part, e)
                      for part, e in per_partition.items()}
    return out, cache


def check_skeletons(mtree_root, system='wii'):
    """Check each console-state skeleton against the generator's tables.

    They are not derived from anything, so the checks are that each says
    exactly what its table says, and that no entry ever carries a digest.
    """
    ok = failed = 0
    for partition, spec in sorted(gen.skeletons_for(system).items()):
        path = os.path.join(mtree_root, f'{partition}.mtree')
        if not os.path.exists(path):
            print(f'FAIL {partition}: {partition}.mtree missing')
            failed += 1
            continue
        want = parse_mtree(path)
        files = gen.rooted(partition, spec['files'])
        unsized = set(gen.rooted(partition, {p: None for p in spec['unsized']}))
        failures = []
        for entry, kv in sorted(want.items()):
            if kv.get('sha1') or kv.get('sha256') or kv.get('md5'):
                failures.append(f'{entry}: has a digest but {partition} is console state')
            if entry in files:
                expect = files[entry]
                if int(kv.get('size', -1)) != expect:
                    failures.append(f'{entry}: size {kv.get("size")} != {expect}')
            elif entry in unsized:
                if 'size' in kv:
                    failures.append(f'{entry}: has a size but none is documented')
            else:
                failures.append(f'not in the {partition} tables: {entry}')
        known = set(files) | unsized
        for entry in sorted(known - set(want)):
            failures.append(f'missing from mtree: {entry}')
        if failures:
            print(f'FAIL {partition}')
            for f in failures[:10]:
                print(f'     {f}')
            failed += 1
        else:
            ok += 1
    return ok, failed


def main():
    ap = argparse.ArgumentParser(add_help=False)
    ap.add_argument('--system', choices=('wii', 'vwii'), default='wii')
    ap.add_argument('--content', type=int, default=0)
    ap.add_argument('args', nargs='*')
    args = ap.parse_args()
    # vWii needs no source file, so it takes two positional arguments, not three
    wanted = 2 if args.system == 'vwii' else 3
    if len(args.args) != wanted:
        print(__doc__, file=sys.stderr)
        return 2
    mtree_root, source, tmd_dir = (
        (args.args[0], None, args.args[1]) if wanted == 2 else args.args)

    updates = (wiiupd.vwii_updates() if args.system == 'vwii'
               else wiiupd.parse(source))
    derived, cache = derive(updates, tmd_dir)

    ok, failed = check_skeletons(mtree_root, args.system)
    partitions = sorted(d for d in os.listdir(mtree_root)
                        if os.path.isdir(os.path.join(mtree_root, d)))
    for partition in partitions:
        mtree_dir = os.path.join(mtree_root, partition)
        for name in sorted(os.listdir(mtree_dir)):
            if not name.endswith('.mtree'):
                continue
            # after deduplication one mtree stands for every firmware listed in
            # its header, and each of those must agree with it
            path = os.path.join(mtree_dir, name)
            want = parse_mtree(path)
            for label in dedupe.labels_of(path):
                have = derived.get(label, {}).get(partition)
                failures = []
                if not have:
                    failures.append(f'no reconstructed {partition} set for this label')
                else:
                    for entry, kv in sorted(want.items()):
                        if entry not in have:
                            failures.append(f'not derivable from metadata: {entry}')
                            continue
                        size, sha1 = have[entry]
                        # shared1 entries carry neither: the name says nothing
                        # about which content was allocated to it
                        if size is None:
                            if 'size' in kv:
                                failures.append(f'{entry}: has a size but none is derivable')
                        elif int(kv.get('size', -1)) != size:
                            failures.append(f'{entry}: size {kv.get("size")} != {size}')
                        if kv.get('sha1') != sha1:
                            failures.append(f'{entry}: sha1 {kv.get("sha1")} != {sha1}')
                    for entry in sorted(set(have) - set(want)):
                        failures.append(f'missing from mtree: {entry}')
                if failures:
                    print(f'FAIL {partition}/{label}')
                    for f in failures[:10]:
                        print(f'     {f}')
                    failed += 1
                else:
                    ok += 1
    print(f'metadata: {ok} ok, {failed} failed')

    if args.content:
        import random
        # Contents are fetched under the id the title is *served* as, which for
        # vWii is not the id inside its TMD. The cache filename carries it, so
        # key everything by that rather than by TMD.title_id — which for vWii
        # would collide with the Wii title of the same number.
        def nus_id(path):
            return int(os.path.basename(path).split('.')[0], 16)

        tickets = {nus_id(p): t.blob for p, t in cache.items()
                   if isinstance(t, wiitmd.Ticket)}
        pool = []
        for path, tmd in cache.items():
            if not isinstance(tmd, wiitmd.Tmd) or nus_id(path) not in tickets:
                continue
            for c in tmd.contents:
                pool.append((nus_id(path), c['id'], c['index'], c['size'],
                             c['sha1'], c['shared']))
        pool.sort()
        random.seed(0)
        sample = random.sample(pool, min(args.content, len(pool)))
        good = bad = 0
        for tid, cid, index, size, sha1, is_shared in sample:
            where = 'shared1' if is_shared else 'title'
            try:
                got = verify_content(tid, tickets[tid], cid, index, size)
            except Exception as e:  # noqa: BLE001
                print(f'FAIL content {tid:016x}/{cid:08x}: {e}')
                bad += 1
                continue
            if got == sha1:
                print(f'ok   content {tid:016x}/{cid:08x} ({size} bytes, {where})')
                good += 1
            else:
                print(f'FAIL content {tid:016x}/{cid:08x}: {got} != {sha1}')
                bad += 1
        print(f'content: {good} ok, {bad} failed')
        failed += bad
    return 0 if not failed else 1


if __name__ == '__main__':
    sys.exit(main())
