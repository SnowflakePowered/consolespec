#!/usr/bin/env python3
"""Verify Nintendo DSi firmware TAD collections against their mtrees.

Reads each firmware zip, decrypts its TADs, and checks the resulting NAND
title layout against the partitionspec mtree for that firmware version:

    <mtree-root>/twl_main/<version>.mtree

Every mtree entry must exist with matching size and md5/sha1/sha256 digests,
and the firmware must not install files absent from the mtree. Contents are
additionally checked against the SHA-1 Nintendo signed into each TMD.

Single mode:
    verify-dsi-mtrees.py <.../twl_main/version.mtree> <firmware.zip>

Batch mode (matches <version>.mtree to v<version>.zip):
    verify-dsi-mtrees.py --all <mtree-root> <firmware-zip-dir>

Exits non-zero if any check fails or a firmware zip is missing.
Requires pycryptodome (pip install pycryptodome).
"""
import argparse
import hashlib
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from importlib import import_module  # noqa: E402

gen = import_module('gen-dsi-mtrees')

CHECKS = ('md5', 'sha1', 'sha256')


def _default_sysdata(firmware_dir):
    path = os.path.join(firmware_dir or '.', 'sysdata')
    return path if os.path.isdir(path) else None


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


def verify(mtree, files):
    failures = []
    want = parse_mtree(mtree)
    for name, kv in sorted(want.items()):
        data = files.get(name)
        if data is None:
            failures.append(f'missing from firmware: {name}')
            continue
        if gen.is_blank(data):
            # a blank save carries a name only; recording a size or digest for
            # one would describe an install that has never been used
            if 'size' in kv or any(kv.get(a) for a in CHECKS):
                failures.append(f'{name}: blank save should carry no size or digest')
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


def check(mtree, zip_path, label, sysdata=None):
    try:
        failures = verify(mtree, gen.firmware_files(zip_path, sysdata, label))
    except Exception as e:  # noqa: BLE001
        failures = [str(e)]
    return report(label, failures)


def main():
    ap = argparse.ArgumentParser(add_help=False)
    ap.add_argument('--all', action='store_true')
    ap.add_argument('--sysdata')
    ap.add_argument('a', nargs='?')
    ap.add_argument('b', nargs='?')
    args = ap.parse_args()
    if not args.a or not args.b:
        print(__doc__, file=sys.stderr)
        return 2

    if not args.all:
        sysdata = args.sysdata or _default_sysdata(os.path.dirname(args.b))
        return 0 if check(args.a, args.b, os.path.basename(args.b), sysdata) else 1

    mtree_dir = os.path.join(args.a, 'twl_main')
    if not os.path.isdir(mtree_dir):
        print(f'error: no twl_main/ directory under {args.a}', file=sys.stderr)
        return 2
    sysdata = args.sysdata or _default_sysdata(args.b)
    zips = {os.path.splitext(n)[0].lstrip('vV'): os.path.join(args.b, n)
            for n in os.listdir(args.b) if n.lower().endswith('.zip')}

    # mtrees live at twl_main/<partition>.mtree when a partition's content
    # never varies, or twl_main/<partition>/<firmware>.mtree when it does.
    # Each is rooted at its partition; a file may stand for several firmwares.
    targets = []
    for entry in sorted(os.listdir(mtree_dir)):
        path = os.path.join(mtree_dir, entry)
        if entry.endswith('.mtree'):
            targets.append((entry[:-len('.mtree')], path, None))
        elif os.path.isdir(path):
            for name in sorted(os.listdir(path)):
                if name.endswith('.mtree'):
                    targets.append((entry, os.path.join(path, name), name[:-len('.mtree')]))

    ok = missing = failed = 0
    for partition, path, label in targets:
        # a static partition is checked against every firmware, a versioned one
        # against the firmware it is named for
        labels = sorted(zips) if label is None else [label]
        if label is not None and label not in zips:
            print(f'MISS {partition}/{label}: no matching firmware zip under {args.b}')
            missing += 1
            continue
        failures = []
        for one in labels:
            try:
                files = gen.firmware_files(zips[one], sysdata, one)
            except Exception as e:  # noqa: BLE001
                failures.append(f'{one}: {e}')
                continue
            scoped = {p.partition('/')[2]: d for p, d in files.items()
                      if p.partition('/')[0] == partition}
            failures += [f'{one}: {f}' for f in verify(path, scoped)]
        if report(f'{partition}' + (f'/{label}' if label else ''), failures):
            ok += 1
        else:
            failed += 1
    print(f'{ok} ok, {failed} failed, {missing} missing')
    return 0 if not failed and not missing else 1


if __name__ == '__main__':
    sys.exit(main())
