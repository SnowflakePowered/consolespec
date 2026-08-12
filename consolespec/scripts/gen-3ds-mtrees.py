#!/usr/bin/env python3
"""Generate partitionspec mtrees for Nintendo 3DS firmware versions.

The 3DS has no public firmware archive, so the manifests are built from
Nintendo's own update servers: for every system title and version that
ninupdates recorded, the signed TMD is fetched from NUS, and the SHA-256 it
records for each content is the hash of that content as installed on
CTRNAND. Contents themselves are never downloaded — the TMD is authoritative
and signed, so the manifest is exact without them.

One mtree is written per firmware version per region, describing the title
contents present on CTRNAND once that firmware is installed:

    title/<title id high>/<title id low>/content/<content id>.app

    <output-dir>/ctrnand/<firmware>_<REGION>.mtree

A firmware's title set is reconstructed by taking, for each title in that
region, the newest version recorded at or before that firmware — ninupdates
records a title only on the firmwares where its version changed.

Usage:
    gen-3ds-mtrees.py --system {ctr,ktr} [--min-titles N]
                      <titlelist-csv-dir> <tmd-dir> <output-dir>

--system picks which console's title list to build from: `ctr` for the
original 3DS family, `ktr` for New 3DS. They ship different title sets, so
they must be generated separately. The CSV directory holds the ninupdates
title lists as 3ds_<system>.csv; the TMD directory holds
<title id>.<version>.tmd files. --min-titles skips firmware/region
combinations with fewer titles than N (early partial scans), default 1.
"""
import argparse
import collections
import csv
import hashlib
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ctrtmd  # noqa: E402


# The directories a clean CTRNAND carries besides title/, and the files that
# live in them. Both were read from reference ctrtransfer images and confirmed
# identical between an o3DS and an n3DS one, so they are properties of the
# filesystem layout rather than of either console.
#
# The files themselves are console-unique: SecureInfo_A holds the region and
# serial, LocalFriendCodeSeed_B and MOVABLE.SED are per-console key material,
# HWCAL* is that unit's hardware calibration, and the dbs/ databases hold its
# install state. Their digests are therefore never recorded — a digest would
# fingerprint one particular console.
CTRNAND_DIRS = [
    'data', 'dbs', 'fixdata', 'fixdata/sysdata', 'private', 'ro', 'ro/private',
    'ro/shared', 'ro/sys', 'rw', 'rw/shared', 'rw/sys', 'ticket', 'title', 'tmp',
]

# Sizes are given only where they are structurally fixed: fixed-layout signed
# blobs, and the pre-allocated databases that do not grow with use. Both
# reference images agree on every size below.
CTRNAND_FILES = {
    'dbs/certs.db': 24576,
    'dbs/title.db': 412672,
    'private/MOVABLE.SED': 320,
    'ro/sys/HWCAL0.dat': 2512,
    'ro/sys/HWCAL1.dat': 2512,
    'rw/sys/LocalFriendCodeSeed_B': 272,
    'rw/sys/SecureInfo_A': 273,
    'rw/sys/rand_seed': 64,
}

# Present on every CTRNAND but with no dependable size: the FAT journal is
# sized to its volume (the two reference images disagree, 114688 vs 98304),
# ticket.db and import.db grow with what the console has installed, and the
# tmp_* databases hold whatever an in-flight install left behind.
CTRNAND_FILES_UNSIZED = [
    '__journal.nn_', 'dbs/import.db', 'dbs/ticket.db',
    'dbs/tmp_i.db', 'dbs/tmp_t.db',
]


def firmware_key(label):
    """Sort key placing firmware labels in release order.

    Build numbers alone are not unique (5.0.0-11 and 6.1.0-11 both exist), so
    the version triple leads and the build number breaks ties.
    """
    m = re.match(r'^(\d+)\.(\d+)\.(\d+)-(\d+)', label)
    if not m:
        return (1, 0, 0, 0, 0, label)
    return (0, int(m.group(1)), int(m.group(2)), int(m.group(3)), int(m.group(4)), label)


def is_release(label):
    return firmware_key(label)[0] == 0


def read_titlelist(csv_dir, system):
    """Return rows of {title, region, versions[], firmwares[]} for one console."""
    rows = []
    path = os.path.join(csv_dir, f'3ds_{system}.csv')
    with open(path) as f:
        for r in csv.DictReader(f):
            versions = [v.lstrip('v') for v in r['Title versions'].split()]
            firmwares = r['Update versions'].split()
            if len(versions) != len(firmwares):
                continue
            rows.append({'title': r['TitleID'], 'region': r['Region'],
                         'versions': versions, 'firmwares': firmwares})
    return rows


def title_sets(rows):
    """Return {(firmware, region): {title: version}} of installed state."""
    # newest version of each title at or before each firmware it appears in
    by_region = {}
    for r in rows:
        history = [(f, v) for f, v in zip(r['firmwares'], r['versions']) if is_release(f)]
        history.sort(key=lambda fv: firmware_key(fv[0]))
        by_region.setdefault(r['region'], {}).setdefault(r['title'], []).extend(history)

    firmwares = sorted({f for r in rows for f in r['firmwares'] if is_release(f)},
                       key=firmware_key)
    out = {}
    for region, titles in by_region.items():
        for firmware in firmwares:
            state = {}
            for title, history in titles.items():
                seen = [v for f, v in sorted(history, key=lambda fv: firmware_key(fv[0]))
                        if firmware_key(f) <= firmware_key(firmware)]
                if seen:
                    state[title] = seen[-1]
            if state:
                out[(firmware, region)] = state
    return out


def mtree_lines(entries, label, device, partition, missing):
    """Render one partition's mtree, rooted at that partition."""
    lines = ['#mtree',
             f'# Nintendo 3DS {device}:/{partition} as installed by firmware {label}',
             '# title content sizes and digests are those Nintendo signed into each TMD']
    if partition in ('dbs', 'ro', 'rw', 'private'):
        lines.append('# console-unique files carry name and size only, never a digest')
    if missing:
        lines.append(f'# {missing} title(s) omitted: no TMD available from NUS')
    lines.append('. type=dir')
    dirs = set()
    for path in entries:
        parts = path.split('/')[:-1]
        for i in range(1, len(parts) + 1):
            dirs.add('/'.join(parts[:i]))
        if partition == 'title' and path.endswith('.app'):
            dirs.add('/'.join(path.split('/')[:2]) + '/cmd')
    merged = {d: None for d in dirs}
    for path, value in entries.items():
        merged[path] = None if value == 'dir' else value
    rows = list(merged.items())
    for path, value in sorted(rows):
        if value is None:
            lines.append(f'./{path} type=dir')
        else:
            size, sha256 = value
            line = f'./{path} type=file'
            if size is not None:
                line += f' size={size}'
            if sha256:
                line += f' sha256={sha256}'
            lines.append(line)
    return lines


def split_by_partition(files, dirs=(), partition_files=None, partition_unsized=()):
    """Group paths into {top-level folder: {path below it: value}}.

    Directories seed the result so that a folder which is empty on a clean
    install still gets a partition. Files sitting at the volume root (the FAT
    journal) belong to no folder and are left out.
    """
    out = {d.partition('/')[0]: {} for d in dirs}
    for d in dirs:
        head, _, rest = d.partition('/')
        if rest:
            out.setdefault(head, {})[rest] = 'dir'
    for path, value in files.items():
        head, _, rest = path.partition('/')
        if rest:
            out.setdefault(head, {})[rest] = value
    for path, size in (partition_files or {}).items():
        head, _, rest = path.partition('/')
        if rest:
            out.setdefault(head, {})[rest] = (size, None)
    for path in partition_unsized:
        head, _, rest = path.partition('/')
        if rest:
            out.setdefault(head, {})[rest] = (None, None)
    return out


def main():
    ap = argparse.ArgumentParser(add_help=False)
    ap.add_argument('--system', choices=('ctr', 'ktr'))
    ap.add_argument('--min-titles', type=int, default=1)
    ap.add_argument('csv_dir', nargs='?')
    ap.add_argument('tmd_dir', nargs='?')
    ap.add_argument('output_dir', nargs='?')
    args = ap.parse_args()
    if not args.system or not args.csv_dir or not args.tmd_dir or not args.output_dir:
        print(__doc__, file=sys.stderr)
        return 2

    try:
        rows = read_titlelist(args.csv_dir, args.system)
    except OSError as e:
        print(f'error: {e}', file=sys.stderr)
        return 2
    if not rows:
        print(f'error: no titles in 3ds_{args.system}.csv', file=sys.stderr)
        return 2
    sets = title_sets(rows)

    cache = {}
    # per (device, partition, firmware label) rendered mtree body
    rendered = collections.defaultdict(dict)
    absent = 0
    for (firmware, region), state in sorted(sets.items(),
                                            key=lambda kv: (firmware_key(kv[0][0]), kv[0][1])):
        by_device, missing = collections.defaultdict(dict), 0
        for title, version in sorted(state.items()):
            path = os.path.join(args.tmd_dir, f'{title}.{version}.tmd')
            if path not in cache:
                try:
                    cache[path] = ctrtmd.load(path)
                except (OSError, ValueError):
                    cache[path] = None
            tmd = cache[path]
            if tmd is None:
                missing += 1
                continue
            by_device[tmd.partition].update(tmd.app_files())
        absent += missing
        if len(state) - missing < args.min_titles:
            continue
        label = f'{firmware}_{region}'
        for device, files in by_device.items():
            if device == 'ctrnand':
                parts = split_by_partition(files, CTRNAND_DIRS, CTRNAND_FILES,
                                           CTRNAND_FILES_UNSIZED)
            else:
                parts = split_by_partition(files)
            for partition, entries in parts.items():
                body = '\n'.join(mtree_lines(entries, label, device, partition,
                                             missing if partition == 'title' else 0)) + '\n'
                rendered[(device, partition)][label] = body

    # A partition whose content never varies is written once; only the ones that
    # differ between firmwares get a file per firmware.
    static, versioned, files_written = 0, 0, 0
    for (device, partition), bodies in sorted(rendered.items()):
        # compare ignoring the header comment naming the firmware
        def strip(b):
            return '\n'.join(l for l in b.splitlines() if not l.startswith('#'))
        distinct = {strip(b) for b in bodies.values()}
        out_dir = os.path.join(args.output_dir, device)
        os.makedirs(out_dir, exist_ok=True)
        if len(distinct) == 1:
            label = sorted(bodies, key=lambda s: (firmware_key(s.rsplit('_', 1)[0]), s))[0]
            body = bodies[label].splitlines()
            body[1] = f'# Nintendo 3DS {device}:/{partition}, identical across every firmware'
            with open(os.path.join(out_dir, f'{partition}.mtree'), 'w') as f:
                f.write('\n'.join(body) + '\n')
            static += 1
            files_written += 1
        else:
            os.makedirs(os.path.join(out_dir, partition), exist_ok=True)
            # firmwares whose content is byte-identical share one file, named
            # for the earliest of them and listing the rest in its header
            groups = collections.defaultdict(list)
            for label, body in bodies.items():
                groups[strip(body)].append(label)
            for labels in groups.values():
                labels.sort(key=lambda s: (firmware_key(s.rsplit('_', 1)[0]), s))
                body = bodies[labels[0]].splitlines()
                body[1] = (f'# Nintendo 3DS {device}:/{partition} as installed by '
                           + (f'firmware {labels[0]}' if len(labels) == 1
                              else f'{len(labels)} firmwares: ' + ', '.join(labels)))
                with open(os.path.join(out_dir, partition, f'{labels[0]}.mtree'), 'w') as f:
                    f.write('\n'.join(body) + '\n')
                files_written += 1
            versioned += 1
    print(f'{files_written} mtrees written to {args.output_dir}: '
          f'{static} static partition(s), {versioned} versioned'
          + (f', {absent} title/version pairs had no TMD' if absent else ''))
    return 0 if files_written else 1


if __name__ == '__main__':
    sys.exit(main())
