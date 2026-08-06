#!/usr/bin/env python3
"""Verify PSP firmware updaters against their system-software mtrees.

Extracts each updater (EBOOT.PBP or bare DATA.PSAR) with pspdecrypt in
extract-only mode and checks the extracted flash partition trees against
the partitionspec mtrees, laid out one per partition per firmware:

    <mtree-root>/flash0/<version>.mtree
    <mtree-root>/flash1/<version>.mtree

Every mtree entry must exist with matching size and md5/sha1/sha256
digests, and the extracted tree must not contain files absent from the
mtree.

Single mode (the partition is taken from the mtree's parent directory):
    verify-psp-mtrees.py [--pspdecrypt=PATH] <.../flashN/version.mtree> <updater>

Batch mode (matches NNN.mtree to NNN.PBP/.PSAR by name, searching the
firmware directory recursively; each firmware is extracted once and
checked against all of its partition mtrees):
    verify-psp-mtrees.py [--pspdecrypt=PATH] --all <mtree-root> <firmware-dir>

Exits non-zero if any check fails or an updater is missing.
pspdecrypt is https://github.com/hrydgard/pspdecrypt (build with make); it
is looked up on PATH when --pspdecrypt is not given.
"""
import argparse
import hashlib
import os
import re
import shutil
import subprocess
import sys
import tempfile
from importlib import import_module

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
dedupe = import_module('dedupe-mtrees')

FLASH_DIRS = re.compile(r'^F(\d+)$')
FLASH_NAME = re.compile(r'^flash\d+$')
CHECKS = ('md5', 'sha1', 'sha256')


def digests(path):
    hashers = {algo: hashlib.new(algo) for algo in CHECKS}
    with open(path, 'rb') as f:
        for chunk in iter(lambda: f.read(1 << 20), b''):
            for h in hashers.values():
                h.update(chunk)
    return {algo: h.hexdigest() for algo, h in hashers.items()}


def extract(pspdecrypt, package, workdir):
    out = os.path.join(workdir, 'out')
    cmd = [pspdecrypt, '-e', '-O', out]
    if package.lower().endswith('.pbp'):
        cmd.append('-A')
    cmd.append(package)
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        raise RuntimeError(f'pspdecrypt failed: {proc.stderr.strip() or proc.stdout.strip()}')
    trees = {}
    for entry in sorted(os.listdir(out)) if os.path.isdir(out) else []:
        m = FLASH_DIRS.match(entry)
        if m:
            trees[f'flash{m.group(1)}'] = os.path.join(out, entry)
    if not trees:
        raise RuntimeError('no flash directories in extracted output')
    return trees


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


def verify_tree(mtree, root):
    """Check one partition mtree against one extracted partition tree."""
    failures = []
    want = parse_mtree(mtree)
    have = {}
    for dirpath, _, filenames in os.walk(root):
        for name in filenames:
            full = os.path.join(dirpath, name)
            have[os.path.relpath(full, root)] = full

    for name, kv in sorted(want.items()):
        path = have.get(name)
        if path is None:
            failures.append(f'missing from extraction: {name}')
            continue
        if 'size' in kv and os.path.getsize(path) != int(kv['size']):
            failures.append(f'{name}: size {os.path.getsize(path)} != {kv["size"]}')
        actual = digests(path)
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
    ap.add_argument('--pspdecrypt', default=shutil.which('pspdecrypt'))
    ap.add_argument('--all', action='store_true')
    ap.add_argument('a', nargs='?')
    ap.add_argument('b', nargs='?')
    args = ap.parse_args()
    if not args.a or not args.b:
        print(__doc__, file=sys.stderr)
        return 2
    if not args.pspdecrypt:
        print('error: pspdecrypt not found on PATH; pass --pspdecrypt=PATH', file=sys.stderr)
        return 2

    if not args.all:
        mtree, package = args.a, args.b
        flash = os.path.basename(os.path.dirname(os.path.abspath(mtree)))
        if not FLASH_NAME.match(flash):
            print(f'error: cannot tell which partition {mtree} describes; '
                  'expected it inside a flashN directory', file=sys.stderr)
            return 2
        try:
            with tempfile.TemporaryDirectory() as workdir:
                trees = extract(args.pspdecrypt, package, workdir)
                if flash not in trees:
                    failures = [f'{flash} not present in extracted updater']
                else:
                    failures = verify_tree(mtree, trees[flash])
        except (RuntimeError, OSError) as e:
            failures = [str(e)]
        return 0 if report(f'{package} ({flash})', failures) else 1

    mtree_root, fw_dir = args.a, args.b
    # one mtree may cover several firmwares once identical states are
    # deduplicated, so the index is built from each file's attribution header
    mtrees = dedupe.index(mtree_root, FLASH_NAME.match)
    packages = {}
    for root, _, names in os.walk(fw_dir):
        for name in names:
            stem, ext = os.path.splitext(name)
            if ext.lower() in ('.pbp', '.psar'):
                packages[stem.lower()] = os.path.join(root, name)

    ok = missing = failed = 0
    for stem in sorted(mtrees):
        package = packages.get(stem)
        if package is None:
            print(f'MISS {stem}: no matching updater under {fw_dir}')
            missing += 1
            continue
        try:
            with tempfile.TemporaryDirectory() as workdir:
                trees = extract(args.pspdecrypt, package, workdir)
                for flash in sorted(set(mtrees[stem]) | set(trees)):
                    if flash not in mtrees[stem]:
                        failures = [f'extracted {flash} has no mtree']
                    elif flash not in trees:
                        failures = [f'{flash} not present in extracted updater']
                    else:
                        failures = verify_tree(mtrees[stem][flash], trees[flash])
                    if report(f'{stem} {flash} ({os.path.relpath(package, fw_dir)})', failures):
                        ok += 1
                    else:
                        failed += 1
        except (RuntimeError, OSError) as e:
            print(f'FAIL {stem}: {e}')
            failed += 1
    print(f'{ok} ok, {failed} failed, {missing} missing')
    return 0 if not failed and not missing else 1


if __name__ == '__main__':
    sys.exit(main())
