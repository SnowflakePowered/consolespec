#!/usr/bin/env python3
"""Verify PS3 firmware updaters against their system-software mtrees.

Decrypts each updater (PS3UPDAT.PUP) and checks the extracted flash
filesystem contents against the partitionspec mtrees, laid out one per
flash device per firmware version:

    <mtree-root>/dev_flash/<version>.mtree
    <mtree-root>/dev_flash3/<version>.mtree

Every mtree entry must exist with matching size and md5/sha1/sha256
digests, and the extracted tree must not contain files absent from the
mtree.

Single mode (the device is taken from the mtree's parent directory):
    verify-ps3-mtrees.py <.../dev_flashN/version.mtree> <PS3UPDAT.PUP>

Batch mode (matches each mtree to its firmware directory by version stem):
    verify-ps3-mtrees.py --all <mtree-root> <firmware-dir>

Exits non-zero if any check fails or an updater is missing.
Requires pycryptodome (pip install pycryptodome).
"""
import argparse
import hashlib
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from importlib import import_module  # noqa: E402

dedupe = import_module('dedupe-mtrees')
import ps3pup  # noqa: E402

DEVICE_NAME = re.compile(r'^dev_flash\d*$')
CHECKS = ('md5', 'sha1', 'sha256')


def version_stem(dir_name):
    name = re.sub(r'^Firmware\s+', '', dir_name).strip().lower()
    return re.sub(r'[^a-z0-9]', '', name) or None


def digests(data):
    return {algo: hashlib.new(algo, data).hexdigest() for algo in CHECKS}


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


def verify_tree(mtree, files):
    failures = []
    want = parse_mtree(mtree)
    for name, kv in sorted(want.items()):
        data = files.get(name)
        if data is None:
            failures.append(f'missing from extraction: {name}')
            continue
        if 'size' in kv and len(data) != int(kv['size']):
            failures.append(f'{name}: size {len(data)} != {kv["size"]}')
        actual = digests(data)
        for algo in CHECKS:
            # `md5` and `md5digest` (etc.) are synonyms in mtree
            expect = kv.get(algo) or kv.get(f'{algo}digest')
            if expect and actual[algo] != expect.lower():
                failures.append(f'{name}: {algo} {actual[algo]} != {expect.lower()}')
    for name in sorted(set(files) - set(want)):
        failures.append(f'not in mtree: {name}')
    return failures


def report(label, failures):
    if failures:
        print(f'FAIL {label}')
        for f in failures[:20]:
            print(f'     {f}')
        if len(failures) > 20:
            print(f'     ... and {len(failures) - 20} more')
    else:
        print(f'ok   {label}')
    return not failures


def main():
    ap = argparse.ArgumentParser(add_help=False)
    ap.add_argument('--all', action='store_true')
    ap.add_argument('a', nargs='?')
    ap.add_argument('b', nargs='?')
    args = ap.parse_args()
    if not args.a or not args.b:
        print(__doc__, file=sys.stderr)
        return 2

    if not args.all:
        mtree, pup = args.a, args.b
        device = os.path.basename(os.path.dirname(os.path.abspath(mtree)))
        if not DEVICE_NAME.match(device):
            print(f'error: cannot tell which flash device {mtree} describes; '
                  'expected it inside a dev_flashN directory', file=sys.stderr)
            return 2
        try:
            trees = ps3pup.flash_trees(pup)
            failures = ([f'{device} not present in updater'] if device not in trees
                        else verify_tree(mtree, trees[device]))
        except Exception as e:  # noqa: BLE001
            failures = [str(e)]
        return 0 if report(f'{pup} ({device})', failures) else 1

    mtree_root, fw_dir = args.a, args.b
    # one mtree may cover several firmwares once identical states are
    # deduplicated, so the index is built from each file's attribution header
    mtrees = dedupe.index(mtree_root, DEVICE_NAME.match)
    pups = {}
    for entry in sorted(os.listdir(fw_dir)):
        pup = os.path.join(fw_dir, entry, 'PS3UPDAT.PUP')
        stem = version_stem(entry)
        if stem and os.path.isfile(pup):
            pups[stem] = pup

    ok = missing = failed = 0
    for stem in sorted(mtrees):
        pup = pups.get(stem)
        if pup is None:
            print(f'MISS {stem}: no matching PS3UPDAT.PUP under {fw_dir}')
            missing += 1
            continue
        try:
            trees = ps3pup.flash_trees(pup)
        except Exception as e:  # noqa: BLE001
            print(f'FAIL {stem}: {e}')
            failed += 1
            continue
        for device in sorted(set(mtrees[stem]) | set(trees)):
            if device not in mtrees[stem]:
                failures = [f'extracted {device} has no mtree']
            elif device not in trees:
                failures = [f'{device} not present in updater']
            else:
                failures = verify_tree(mtrees[stem][device], trees[device])
            if report(f'{stem} {device}', failures):
                ok += 1
            else:
                failed += 1
    print(f'{ok} ok, {failed} failed, {missing} missing')
    return 0 if not failed and not missing else 1


if __name__ == '__main__':
    sys.exit(main())
