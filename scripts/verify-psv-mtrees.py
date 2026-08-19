#!/usr/bin/env python3
"""Verify PS Vita firmware updaters against their system-software mtrees.

Decrypts each updater (PSVUPDAT.PUP) and checks the files in its flash
partition images against the partitionspec mtrees, laid out one per
partition per firmware version:

    <mtree-root>/os0/<version>.mtree
    <mtree-root>/vs0/<version>.mtree

Every mtree entry must exist with matching size and md5/sha1/sha256
digests, and the partition image must not contain files absent from the
mtree.

Single mode (the partition is taken from the mtree's parent directory):
    verify-psv-mtrees.py <.../os0/version.mtree> <PSVUPDAT.PUP>

Batch mode (matches NNN.mtree to NNN.PUP by name):
    verify-psv-mtrees.py --all <mtree-root> <firmware-dir>

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
import fatfs  # noqa: E402
import psvpup  # noqa: E402

PARTITION = re.compile(r'^[a-z]{2}\d(_\d+)?$|^unknown(_\d+)?$')
CHECKS = ('md5', 'sha1', 'sha256')


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


def verify_image(mtree, image):
    failures = []
    want = parse_mtree(mtree)
    have = {p: get for p, size, get in fatfs.Fat(image).walk() if size is not None}
    for name, kv in sorted(want.items()):
        get = have.get(name)
        if get is None:
            failures.append(f'missing from image: {name}')
            continue
        data = get()
        if 'size' in kv and len(data) != int(kv['size']):
            failures.append(f'{name}: size {len(data)} != {kv["size"]}')
        actual = digests(data)
        for algo in CHECKS:
            # `md5` and `md5digest` (etc.) are synonyms in mtree
            expect = kv.get(algo) or kv.get(f'{algo}digest')
            if expect and actual[algo] != expect.lower():
                failures.append(f'{name}: {algo} {actual[algo]} != {expect.lower()}')
    for name in sorted(set(have) - set(want)):
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
        partition = os.path.basename(os.path.dirname(os.path.abspath(mtree)))
        if not PARTITION.match(partition):
            print(f'error: cannot tell which partition {mtree} describes; '
                  'expected it inside a partition directory', file=sys.stderr)
            return 2
        try:
            images = psvpup.partition_images(pup)
            failures = ([f'{partition} not present in updater'] if partition not in images
                        else verify_image(mtree, images[partition]))
        except Exception as e:  # noqa: BLE001
            failures = [str(e)]
        return 0 if report(f'{pup} ({partition})', failures) else 1

    mtree_root, fw_dir = args.a, args.b
    # one mtree may cover several firmwares once identical states are
    # deduplicated, so the index is built from each file's attribution header
    mtrees = dedupe.index(mtree_root, PARTITION.match)
    pups = {}
    for name in sorted(os.listdir(fw_dir)):
        stem, ext = os.path.splitext(name)
        if ext.lower() == '.pup':
            pups[stem] = os.path.join(fw_dir, name)

    ok = missing = failed = 0
    for stem in sorted(mtrees):
        pup = pups.get(stem)
        if pup is None:
            print(f'MISS {stem}: no matching .PUP under {fw_dir}')
            missing += 1
            continue
        try:
            images = psvpup.partition_images(pup)
        except Exception as e:  # noqa: BLE001
            print(f'FAIL {stem}: {e}')
            failed += 1
            continue
        for partition in sorted(set(mtrees[stem]) | set(images)):
            if partition not in mtrees[stem]:
                failures = [f'extracted {partition} has no mtree']
            elif partition not in images:
                failures = [f'{partition} not present in updater']
            else:
                failures = verify_image(mtrees[stem][partition], images[partition])
            if report(f'{stem} {partition}', failures):
                ok += 1
            else:
                failed += 1
    print(f'{ok} ok, {failed} failed, {missing} missing')
    return 0 if not failed and not missing else 1


if __name__ == '__main__':
    sys.exit(main())
