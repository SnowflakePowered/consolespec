#!/usr/bin/env python3
"""Generate partitionspec mtrees for the system software contained in PS3
official firmware updaters.

Each updater (PS3UPDAT.PUP) is decrypted with the public SCEPKG keys and
its flash filesystem contents extracted, exactly as RPCS3 installs
firmware. One mtree is written per flash device per firmware version,
rooted at the device, under a per-device directory:

    <output-dir>/dev_flash/<version>.mtree
    <output-dir>/dev_flash3/<version>.mtree

Firmware directories are expected to be named like the official archive
("Firmware 4.91", "Firmware 3.56 v1"); the version stem is derived from
the directory name and cross-checked against the version the PUP itself
declares.

Usage:
    gen-ps3-mtrees.py <firmware-dir> <output-dir>

Requires pycryptodome (pip install pycryptodome).
"""
import argparse
import hashlib
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ps3pup  # noqa: E402


def version_stem(dir_name):
    """'Firmware 3.56 v1' -> '356v1'; 'Firmware 4.91' -> '491'."""
    name = re.sub(r'^Firmware\s+', '', dir_name).strip().lower()
    stem = re.sub(r'[^a-z0-9]', '', name)
    return stem or None


def declared_version(stem):
    """'356v1' -> '3.56' for cross-checking against the PUP's version.txt."""
    m = re.match(r'^(\d)(\d\d)', stem)
    return f'{m.group(1)}.{m.group(2)}' if m else None


def digests(data):
    return (hashlib.md5(data).hexdigest(),
            hashlib.sha1(data).hexdigest(),
            hashlib.sha256(data).hexdigest())


def mtree_lines(files, label, device):
    lines = ['#mtree', f'# PS3 official firmware {label} system software ({device})',
             '. type=dir']
    dirs = set()
    for path in files:
        parts = path.split('/')[:-1]
        for i in range(1, len(parts) + 1):
            dirs.add('/'.join(parts[:i]))
    entries = [(d, None) for d in dirs] + [(p, files[p]) for p in files]
    for path, data in sorted(entries):
        if data is None:
            lines.append(f'./{path} type=dir')
        else:
            md5, sha1, sha256 = digests(data)
            lines.append(f'./{path} type=file size={len(data)} '
                         f'md5={md5} sha1={sha1} sha256={sha256}')
    return lines


def main():
    ap = argparse.ArgumentParser(add_help=False)
    ap.add_argument('firmware_dir', nargs='?')
    ap.add_argument('output_dir', nargs='?')
    args = ap.parse_args()
    if not args.firmware_dir or not args.output_dir:
        print(__doc__, file=sys.stderr)
        return 2

    pups = []
    for entry in sorted(os.listdir(args.firmware_dir)):
        pup = os.path.join(args.firmware_dir, entry, 'PS3UPDAT.PUP')
        stem = version_stem(entry)
        if stem and os.path.isfile(pup):
            pups.append((stem, entry, pup))

    written, failed = 0, []
    for stem, dir_name, pup in pups:
        try:
            label = ps3pup.pup_version(pup)
            expect = declared_version(stem)
            if expect and label and not label.startswith(expect):
                print(f'warn {stem}: PUP declares {label}, directory says {expect}',
                      file=sys.stderr)
            trees = ps3pup.flash_trees(pup)
        except Exception as e:  # noqa: BLE001 - report and continue over the set
            print(f'FAIL {stem}: {e}', file=sys.stderr)
            failed.append(stem)
            continue
        parts = []
        for device, files in sorted(trees.items()):
            out_dir = os.path.join(args.output_dir, device)
            os.makedirs(out_dir, exist_ok=True)
            lines = mtree_lines(files, label or stem, device)
            with open(os.path.join(out_dir, f'{stem}.mtree'), 'w') as f:
                f.write('\n'.join(lines) + '\n')
            parts.append(f'{device}: {len(files)} files')
            written += 1
        print(f'ok   {stem} ({label}): ' + ', '.join(parts))
    print(f'{written} mtrees written to {args.output_dir}'
          + (f', {len(failed)} failed: {", ".join(failed)}' if failed else ''))
    return 0 if written and not failed else 1


if __name__ == '__main__':
    sys.exit(main())
