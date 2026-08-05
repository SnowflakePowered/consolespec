#!/usr/bin/env python3
"""Generate partitionspec mtrees for the system software contained in PS Vita
official firmware updaters.

Each updater (PSVUPDAT.PUP) has its SPKG segments decrypted with the public
SPKG keysets and its flash partition images (FAT filesystems) read directly,
so the digests describe the files as installed on the console's partitions.

One mtree is written per partition per firmware version, rooted at the
partition, under a per-partition directory:

    <output-dir>/os0/<version>.mtree
    <output-dir>/vs0/<version>.mtree

Updaters are matched by their version-stem filename: 374.PUP, 160.PUP,
0945.PUP, 1000v1.PUP, ...

Usage:
    gen-psv-mtrees.py <firmware-dir> <output-dir>

Requires pycryptodome (pip install pycryptodome).
"""
import argparse
import hashlib
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import fatfs  # noqa: E402
import psvpup  # noqa: E402

STEM = re.compile(r'^\d+(v\d)?$')


def digests(data):
    return (hashlib.md5(data).hexdigest(),
            hashlib.sha1(data).hexdigest(),
            hashlib.sha256(data).hexdigest())


def mtree_lines(image, label, partition):
    lines = ['#mtree', f'# PS Vita official firmware {label} system software ({partition})',
             '. type=dir']
    entries = []
    for path, size, get in fatfs.Fat(image).walk():
        entries.append((path, None if size is None else get()))
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
    for name in sorted(os.listdir(args.firmware_dir)):
        stem, ext = os.path.splitext(name)
        if ext.lower() == '.pup' and STEM.match(stem):
            pups.append((stem, os.path.join(args.firmware_dir, name)))

    written, failed = 0, []
    for stem, pup in pups:
        try:
            label = psvpup.pup_version(pup) or stem
            images = psvpup.partition_images(pup)
            parts = []
            for partition, image in images.items():
                lines = mtree_lines(image, label, partition)
                out_dir = os.path.join(args.output_dir, partition)
                os.makedirs(out_dir, exist_ok=True)
                with open(os.path.join(out_dir, f'{stem}.mtree'), 'w') as f:
                    f.write('\n'.join(lines) + '\n')
                nfiles = sum(1 for l in lines if ' type=file ' in l)
                parts.append(f'{partition}: {nfiles} files')
                written += 1
        except Exception as e:  # noqa: BLE001 - report and continue over the set
            print(f'FAIL {stem}: {e}', file=sys.stderr)
            failed.append(stem)
            continue
        print(f'ok   {stem} ({label}): ' + ', '.join(parts))
    print(f'{written} mtrees written to {args.output_dir}'
          + (f', {len(failed)} failed: {", ".join(failed)}' if failed else ''))
    return 0 if written and not failed else 1


if __name__ == '__main__':
    sys.exit(main())
