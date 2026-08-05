#!/usr/bin/env python3
"""Generate partitionspec mtrees for the system software contained in PSP
official firmware updaters.

Each updater (EBOOT.PBP, or a bare DATA.PSAR for the firmwares that never
shipped as a PBP) is extracted with pspdecrypt in extract-only mode, so
files are kept exactly as they are installed on the flash filesystem (PRX
modules stay encrypted) and digests match a dump of a real console.

One mtree is written per flash partition per firmware version, rooted at
the partition, under a per-partition directory:

    <output-dir>/flash0/<version>.mtree
    <output-dir>/flash1/<version>.mtree

Inputs are matched by their version-stem filename: 100.PBP, 200v1.PBP,
610go.PBP (PSP Go), 360.PSAR, ...

Usage:
    gen-psp-mtrees.py [--pspdecrypt=PATH] <firmware-dir> <output-dir>

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

# pspdecrypt output directories -> flash device names; everything else
# (PSARDUMPER intermediate artifacts, ipl stages) is not installed content
FLASH_DIRS = re.compile(r'^F(\d+)$')


def digests(path):
    md5, sha1, sha256 = hashlib.md5(), hashlib.sha1(), hashlib.sha256()
    with open(path, 'rb') as f:
        for chunk in iter(lambda: f.read(1 << 20), b''):
            md5.update(chunk)
            sha1.update(chunk)
            sha256.update(chunk)
    return md5.hexdigest(), sha1.hexdigest(), sha256.hexdigest()


def version_label(stem):
    m = re.match(r'^(\d)(\d\d)(v\d)?(go)?$', stem)
    if not m:
        return None
    major, minor, variant, go = m.groups()
    label = f'{major}.{minor}'
    if variant:
        label += f' ({variant})'
    if go:
        label += ' (PSP Go)'
    return label


def extract(pspdecrypt, package, workdir):
    """Extract the updater's PSAR; return {flashN: extracted_dir}."""
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


def mtree_lines(root, label, flash):
    lines = ['#mtree', f'# PSP official firmware {label} system software ({flash})',
             '. type=dir']
    entries = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames.sort()
        rel = os.path.relpath(dirpath, root)
        for name in sorted(filenames):
            entries.append((os.path.normpath(os.path.join(rel, name)),
                            os.path.join(dirpath, name)))
        if rel != '.':
            entries.append((rel, None))
    for path, file in sorted(entries):
        if file is None:
            lines.append(f'./{path} type=dir')
        else:
            md5, sha1, sha256 = digests(file)
            lines.append(f'./{path} type=file size={os.path.getsize(file)} '
                         f'md5={md5} sha1={sha1} sha256={sha256}')
    return lines


def main():
    ap = argparse.ArgumentParser(add_help=False)
    ap.add_argument('--pspdecrypt', default=shutil.which('pspdecrypt'))
    ap.add_argument('firmware_dir', nargs='?')
    ap.add_argument('output_dir', nargs='?')
    args = ap.parse_args()
    if not args.firmware_dir or not args.output_dir:
        print(__doc__, file=sys.stderr)
        return 2
    if not args.pspdecrypt:
        print('error: pspdecrypt not found on PATH; pass --pspdecrypt=PATH', file=sys.stderr)
        return 2

    packages = []
    for root, _, names in os.walk(args.firmware_dir):
        for name in sorted(names):
            stem, ext = os.path.splitext(name)
            if ext.lower() in ('.pbp', '.psar') and version_label(stem.lower()):
                packages.append((stem.lower(), os.path.join(root, name)))

    written, failed = 0, []
    for stem, package in sorted(packages):
        label = version_label(stem)
        try:
            with tempfile.TemporaryDirectory() as workdir:
                trees = extract(args.pspdecrypt, package, workdir)
                parts = []
                for flash, root in sorted(trees.items()):
                    lines = mtree_lines(root, label, flash)
                    out_dir = os.path.join(args.output_dir, flash)
                    os.makedirs(out_dir, exist_ok=True)
                    with open(os.path.join(out_dir, f'{stem}.mtree'), 'w') as f:
                        f.write('\n'.join(lines) + '\n')
                    nfiles = sum(1 for l in lines if ' type=file ' in l)
                    parts.append(f'{flash}: {nfiles} files')
                    written += 1
        except (RuntimeError, OSError) as e:
            print(f'FAIL {stem}: {e}', file=sys.stderr)
            failed.append(stem)
            continue
        print(f'ok   {stem}: ' + ', '.join(parts))
    print(f'{written} mtrees written to {args.output_dir}'
          + (f', {len(failed)} failed: {", ".join(failed)}' if failed else ''))
    return 0 if written and not failed else 1


if __name__ == '__main__':
    sys.exit(main())
