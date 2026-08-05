#!/usr/bin/env python3
"""Generate partitionspec mtrees for Nintendo DSi firmware versions.

Each firmware version is distributed as a zip of retail-signed TADs, one per
system title. This reads those zips, decrypts each TAD's contents, and writes
one mtree per firmware version describing the state of the TWL_MAIN partition
once that firmware is installed:

    ticket/<title id high>/<title id low>.tik
    title/<title id high>/<title id low>/content/title.tmd
    title/<title id high>/<title id low>/content/<content id>.app
    title/<title id high>/<title id low>/data/{public,private}.sav
    sys/{cert.sys,TWLFontTable.dat,HWINFO_N.dat}
    shared1/TWLCFG{0,1}.dat
    shared2/{0000,launcher/wrap.bin}

    <output-dir>/twl_main/<version>.mtree

Every content is checked against the SHA-1 Nintendo signed into its TMD, so a
generated mtree is only written when the whole firmware verifies. Save files
are the zero-filled ones a freshly installed title starts with, sized from the
title's own ROM header. The system data files come from a directory of the
shared NAND files (cert.sys, TWLFontTable.dat, TWLFontTable_CN.dat,
TWLFontTable_KR.dat, TWLCFG0.dat, TWLCFG1.dat, wrap.bin, 0000, HWINFO_N.dat);
the font table is chosen by the firmware's region.

Usage:
    gen-dsi-mtrees.py [--sysdata DIR] <firmware-zip-dir> <output-dir>

--sysdata defaults to a `sysdata` directory inside the firmware zip directory.
Without it, only the ticket/title trees are described.

Requires pycryptodome (pip install pycryptodome).
"""
import argparse
import hashlib
import os
import sys
import zipfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import dsitad  # noqa: E402


def digests(data):
    return (hashlib.md5(data).hexdigest(),
            hashlib.sha1(data).hexdigest(),
            hashlib.sha256(data).hexdigest())


# NAND path -> filename in the system data directory. The font table varies by
# region and is resolved separately.
SYSTEM_DATA = {
    'sys/cert.sys': 'cert.sys',
    'sys/HWINFO_N.dat': 'HWINFO_N.dat',
    'shared1/TWLCFG0.dat': 'TWLCFG0.dat',
    'shared1/TWLCFG1.dat': 'TWLCFG1.dat',
    'shared2/0000': '0000',
    'shared2/launcher/wrap.bin': 'wrap.bin',
}
FONT_TABLE = {'C': 'TWLFontTable_CN.dat', 'K': 'TWLFontTable_KR.dat'}


def region_of(label):
    """'1.4.5J' -> 'J'; '1.4.1U_NZB' -> 'U'; '1.4J_KST' -> 'J'."""
    for ch in label:
        if ch.isalpha():
            return ch.upper()
    return ''


def system_data_files(sysdata_dir, label):
    if not sysdata_dir:
        return {}
    files = {}
    wanted = dict(SYSTEM_DATA)
    wanted['sys/TWLFontTable.dat'] = FONT_TABLE.get(region_of(label), 'TWLFontTable.dat')
    for nand_path, name in wanted.items():
        path = os.path.join(sysdata_dir, name)
        if not os.path.exists(path):
            raise ValueError(f'system data file not found: {path}')
        with open(path, 'rb') as f:
            files[nand_path] = f.read()
    return files


def firmware_files(zip_path, sysdata_dir=None, label=None):
    """Return {nand_path: bytes} for a firmware's installed NAND state."""
    files = {}
    with zipfile.ZipFile(zip_path) as z:
        names = [n for n in z.namelist()
                 if n.lower().endswith('.tad') and not os.path.basename(n).startswith('._')]
        if not names:
            raise ValueError('no TADs in archive')
        for name in sorted(names):
            tad = dsitad.Tad(z.read(name))
            for path, data in dsitad.nand_files(tad).items():
                if path in files and files[path] != data:
                    raise ValueError(f'{name}: conflicting content for {path}')
                files[path] = data
    if label is None:
        label = os.path.splitext(os.path.basename(zip_path))[0].lstrip('vV')
    files.update(system_data_files(sysdata_dir, label))
    return files


def mtree_lines(files, label):
    lines = ['#mtree',
             f'# Nintendo DSi firmware {label} installed NAND state (TWL_MAIN)',
             '# save files are the zero-filled ones a freshly installed title starts with',
             '. type=dir']
    dirs = set()
    for path in files:
        parts = path.split('/')[:-1]
        for i in range(1, len(parts) + 1):
            dirs.add('/'.join(parts[:i]))
    entries = [(d, None) for d in dirs] + list(files.items())
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
    ap.add_argument('--sysdata')
    ap.add_argument('firmware_dir', nargs='?')
    ap.add_argument('output_dir', nargs='?')
    args = ap.parse_args()
    if not args.firmware_dir or not args.output_dir:
        print(__doc__, file=sys.stderr)
        return 2

    sysdata = args.sysdata
    if sysdata is None:
        default = os.path.join(args.firmware_dir, 'sysdata')
        sysdata = default if os.path.isdir(default) else None
    if sysdata is None:
        print('warning: no sysdata directory; describing ticket/title trees only',
              file=sys.stderr)

    zips = sorted(n for n in os.listdir(args.firmware_dir) if n.lower().endswith('.zip'))
    out_dir = os.path.join(args.output_dir, 'twl_main')
    os.makedirs(out_dir, exist_ok=True)

    written, failed = 0, []
    for name in zips:
        label = os.path.splitext(name)[0].lstrip('vV')
        try:
            files = firmware_files(os.path.join(args.firmware_dir, name), sysdata, label)
            lines = mtree_lines(files, label)
        except Exception as e:  # noqa: BLE001 - report and continue over the set
            print(f'FAIL {label}: {e}', file=sys.stderr)
            failed.append(label)
            continue
        with open(os.path.join(out_dir, f'{label}.mtree'), 'w') as f:
            f.write('\n'.join(lines) + '\n')
        titles = len({p.split('/')[2] for p in files if p.startswith('title/')})
        print(f'ok   {label}: {titles} titles, {len(files)} files')
        written += 1
    print(f'{written} mtrees written to {out_dir}'
          + (f', {len(failed)} failed: {", ".join(failed)}' if failed else ''))
    return 0 if written and not failed else 1


if __name__ == '__main__':
    sys.exit(main())
