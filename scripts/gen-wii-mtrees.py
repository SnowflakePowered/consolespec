#!/usr/bin/env python3
"""Generate partitionspec mtrees for Nintendo Wii system menu versions.

Like the 3DS generator, this never downloads content: the signed TMD records
the size and SHA-1 of every content as the console stores it, so the
manifests are exact from metadata alone. What the Wii lacks is a public
record of which titles each system update installs, so that comes from
WiiQt's transcription of the update partitions (see wiiupd.py), and the TMDs
are then fetched from NUS by fetch-wii-tmds.py.

One mtree is written per system menu version per region, describing the NAND
state a console is left in by that update:

    <output-dir>/title/<version>.mtree     title/<hi>/<lo>/content/*.app, title.tmd
    <output-dir>/ticket/<version>.mtree    ticket/<hi>/<lo>.tik
    <output-dir>/shared1/<version>.mtree   shared1/*.app, content.map

plus one version-independent skeleton per partition that holds console state
rather than update content:

    <output-dir>/{shared2,sys,meta,import,tmp}.mtree

Most system content is *shared* — IOS modules and the system menu's fonts and
sound banks are stored once in shared1 rather than under title/, which is why
an IOS contributes only its title.tmd to the title partition. shared1 is
listed by name but without sizes or digests, for the reason given at
shared1_entries(); the skeletons carry no digests either, for the different
reason given at SKELETONS.

Usage:
    gen-wii-mtrees.py [--system {wii,vwii}] [--no-tickets]
                      [<nusdownloader.cpp>] <tmd-dir> <output-dir>

<nusdownloader.cpp> is WiiQt/nusdownloader.cpp from github.com/trapexit/wiiqt
and <tmd-dir> is the cache fetch-wii-tmds.py populated from it.

--system vwii instead builds the Wii U's SLCCMPT partition, the Wii NAND it
carries for Wii mode, from the vWii table in wiiupd.py; the source file is not
needed and must be omitted. SLCCMPT is a Wii NAND filesystem, so title, ticket
and shared1 come out exactly as they do for a Wii. The console-state skeletons
are Wii-only: their layouts came from sources describing a Wii, and nothing
comparable documents what vWii leaves in shared2, sys or meta.
"""
import argparse
import collections
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import wiitmd  # noqa: E402
import wiiupd  # noqa: E402


# Partitions holding console state rather than update content. Nothing in them
# comes from NUS — the System Menu and IOS write them at runtime, or Nintendo's
# factory line wrote them at manufacture — so these are skeletons of known
# paths, never derived manifests. They carry no digests at all, because a
# digest here would fingerprint one particular console: SYSCONF holds that
# unit's nickname and paired Wiimotes, RFL_DB.dat its owner's Miis, uid.sys its
# install history, testlog.txt its factory test run. A size is recorded only
# where a source fixes one.
#
# Layouts come from wiibrew's partition pages, which are stubs, cross-checked
# against a real retail Wii NAND dump. A size is recorded only where two
# independent sources agree, since the dump is a single console and cannot on
# its own distinguish a fixed format from that unit's state:
#
#   SYSCONF          Dolphin's SYSCONF_SIZE, which it enforces on load, and the
#                    dump both say 0x4000.
#   NANDBOOTINFO     wiibrew's struct is a 0x20 header plus a 0x1000 argbuf,
#                    and the dump agrees at 0x1020.
#   cert.sys         the three certificates NUS appends to every TMD and ticket
#                    — CA00000001, CP00000004 and XS00000003 — total 2560, and
#                    the dump agrees.
#   title.met        wiiqt's GenMeta writes 0x40 bytes; the dump agrees, and
#                    carries exactly the three titles wiiqt writes them for.
#   wc24 *.bin/*.ctl the defaults Dolphin ships in Data/Sys/Wii/shared2/wc24
#                    match the dump.
#
# Files the dump shows to vary are left unsized even though a size was
# observed: the wc24 mailboxes had grown to 7MB and 2MB against Dolphin's
# 48-byte fresh copies, and uid.sys and space.sys track install history.
# RFL_DB.dat is left unsized because the two sources conflict — wiibrew's
# offset map ends it at 0x1F1E0, the dump is 779968 bytes — and one console
# cannot settle which is general.
#
# Paths wiibrew lists that the dump does not have are kept: several are
# conditional (a WiiConnect24 download counter, a play-time limit), and one
# console's absence is not evidence they never exist. Artifacts of that
# console's homebrew are deliberately not imported.
#
# None of these are resolved per system menu version: no source records which
# of these files a given version creates.
SKELETONS = {
    'shared2': {
        'note': 'setup data and logs for WiiConnect24, Miis and the factory test line',
        'dirs': [
            'shared2', 'shared2/DIAG', 'shared2/FaceLib', 'shared2/aging',
            'shared2/diag', 'shared2/ec', 'shared2/ec/sync', 'shared2/menu',
            'shared2/menu/FaceLib', 'shared2/menu/vc', 'shared2/succession',
            'shared2/sys', 'shared2/sys/net', 'shared2/sys/net/02',
            'shared2/test', 'shared2/test2', 'shared2/title', 'shared2/wc24',
            'shared2/wc24/mbox',
        ],
        'files': {
            'shared2/sys/NANDBOOTINFO': 4128,
            'shared2/sys/SYSCONF': 16384,
            'shared2/wc24/mbox/wc24recv.ctl': 32768,
            'shared2/wc24/mbox/wc24send.ctl': 16384,
            'shared2/wc24/misc.bin': 1024,
            'shared2/wc24/nwc24dl.bin': 63488,
            'shared2/wc24/nwc24fl.bin': 32864,
            'shared2/wc24/nwc24fls.bin': 12800,
            'shared2/wc24/nwc24msg.cbk': 1024,
            'shared2/wc24/nwc24msg.cfg': 1024,
        },
        'unsized': [
            'shared2/DWC_AUTHDATA', 'shared2/cntcache.txt',
            'shared2/ec/shopsetu.log', 'shared2/expired',
            'shared2/menu/FaceLib/RFL_DB.dat', 'shared2/menu/vc/settings.sav',
            'shared2/succession/shop.log', 'shared2/succession/transfer.id',
            'shared2/sys/flags.dat', 'shared2/sys/net/02/config.dat',
            'shared2/sys/net/dhcp.dat', 'shared2/test/testlog.txt',
            'shared2/test2/dvderror.dat', 'shared2/test2/nanderr.log',
            'shared2/wc24/dlcnt.bin', 'shared2/wc24/mbox/wc24recv.mbx',
            'shared2/wc24/mbox/wc24send.mbx',
        ],
    },
    'sys': {
        'note': 'IOS storage; uid.sys and the .sys files track this console\'s own state',
        'dirs': ['sys'],
        'files': {'sys/cert.sys': 2560},
        'unsized': [
            'sys/boot.sys', 'sys/cc.sys', 'sys/disc.sys', 'sys/launch.sys',
            'sys/space.sys', 'sys/uid.sys',
        ],
    },
    'meta': {
        'note': 'title.met blobs written by the factory install disc, not by any update',
        'dirs': [
            'meta', 'meta/00000001', 'meta/00000001/00000002',
            'meta/00000001/00000004', 'meta/00000001/00000009',
        ],
        'files': {
            'meta/00000001/00000002/title.met': 64,
            'meta/00000001/00000004/title.met': 64,
            'meta/00000001/00000009/title.met': 64,
        },
        'unsized': [],
    },
    'import': {
        'note': 'scratch space for an in-progress title install; empty unless one was interrupted',
        'dirs': ['import'],
        'files': {},
        'unsized': [],
    },
    'tmp': {
        'note': 'scratch space cleared by the System Menu on boot; empty at rest',
        'dirs': ['tmp'],
        'files': {},
        'unsized': [],
    },
}


def tmd_path(tmd_dir, tid, version):
    tag = 'latest' if version == wiiupd.LATEST else str(version)
    return os.path.join(tmd_dir, f'{tid:016x}.{tag}.tmd')


def tik_path(tmd_dir, tid, version):
    tag = 'latest' if version == wiiupd.LATEST else str(version)
    return os.path.join(tmd_dir, f'{tid:016x}.{tag}.tik')


TITLES = {
    'wii': 'Nintendo Wii system menu {label} installed NAND state ({partition})',
    'vwii': 'Nintendo Wii U vWii {label} installed SLCCMPT state ({partition})',
}


def mtree_lines(entries, label, partition, header, missing, system='wii'):
    lines = ['#mtree',
             '# ' + TITLES[system].format(label=label, partition=partition),
             f'# {header}']
    if missing:
        lines.append(f'# {missing} title(s) omitted: no metadata cached from NUS')
    lines.append('. type=dir')
    dirs = set()
    for path in entries:
        parts = path.split('/')[:-1]
        for i in range(1, len(parts) + 1):
            dirs.add('/'.join(parts[:i]))
    rows = [(d, None) for d in dirs] + list(entries.items())
    for path, value in sorted(rows):
        if value is None:
            lines.append(f'./{path} type=dir')
            continue
        size, sha1 = value
        line = f'./{path} type=file'
        if size is not None:
            line += f' size={size}'
        if sha1:
            line += f' sha1={sha1}'
        lines.append(line)
    return lines


def shared1_entries(shared_hashes):
    """Return {nand_path: (size, sha1)} for shared1 given the contents installed.

    A shared content's filename is not a property of the content: the console
    hands out the lowest unused number as each one is installed, so which
    content lands in which .app depends on the order they went in. The set of
    names is still fixed — N contents always occupy 00000000.app upwards — and
    content.map is one 28-byte record per content, so both are recorded, but no
    name can be given a size or a digest.
    """
    entries = {'shared1/content.map': (28 * len(shared_hashes), None)}
    for i in range(len(shared_hashes)):
        entries[f'shared1/{i:08x}.app'] = (None, None)
    return entries


def write_skeleton(output_dir, partition):
    """Write one console-state partition skeleton. Returns its path."""
    spec = SKELETONS[partition]
    lines = ['#mtree',
             f'# Nintendo Wii {partition} partition skeleton',
             f'# {spec["note"]}',
             '# console state, not update content: paths only, never a digest,',
             '# and sizes only where a source fixes them',
             '. type=dir']
    entries = {p: (s, None) for p, s in spec['files'].items()}
    entries.update({p: (None, None) for p in spec['unsized']})
    rows = [(d, None) for d in spec['dirs']] + list(entries.items())
    for entry, value in sorted(rows):
        if value is None:
            lines.append(f'./{entry} type=dir')
        else:
            size, _ = value
            lines.append(f'./{entry} type=file'
                         + (f' size={size}' if size is not None else ''))
    path = os.path.join(output_dir, f'{partition}.mtree')
    os.makedirs(output_dir, exist_ok=True)
    with open(path, 'w') as f:
        f.write('\n'.join(lines) + '\n')
    return path


HEADERS = {
    'title': 'sizes and digests are those Nintendo signed into each title\'s TMD; '
             'shared contents live in shared1 and are not listed',
    'ticket': 'each ticket is the signed 0x2A4-byte prefix of the title\'s NUS cetk',
    'shared1': 'names are allocated in install order, so no .app can be given a '
               'size or digest; content.map is 28 bytes per content',
}


def main():
    ap = argparse.ArgumentParser(add_help=False)
    ap.add_argument('--system', choices=('wii', 'vwii'), default='wii')
    ap.add_argument('--no-tickets', action='store_true')
    ap.add_argument('args', nargs='*')
    args = ap.parse_args()
    # vWii needs no source file, so it takes two positional arguments, not three
    wanted = 2 if args.system == 'vwii' else 3
    if len(args.args) != wanted:
        print(__doc__, file=sys.stderr)
        return 2
    source, tmd_dir, output_dir = ([None] + args.args) if wanted == 2 else args.args

    if args.system == 'vwii':
        updates = wiiupd.vwii_updates()
    else:
        try:
            updates = wiiupd.parse(source)
        except OSError as e:
            print(f'error: {e}', file=sys.stderr)
            return 2
    if not updates:
        print(f'error: no update lists in {source}', file=sys.stderr)
        return 2

    cache = {}
    written, absent = collections.Counter(), 0
    for label in sorted(updates, key=wiiupd.label_key):
        by_partition, missing = collections.defaultdict(dict), 0
        shared_hashes = set()
        for tid, version in sorted(updates[label].items()):
            path = tmd_path(tmd_dir, tid, version)
            if path not in cache:
                try:
                    cache[path] = wiitmd.load(path)
                except (OSError, ValueError):
                    cache[path] = None
            tmd = cache[path]
            if tmd is None:
                missing += 1
                continue
            by_partition['title'].update(tmd.app_files())
            by_partition['title'].update(tmd.tmd_file())
            shared_hashes.update(sha1 for _, sha1 in tmd.shared_contents())
            if args.no_tickets:
                continue
            path = tik_path(tmd_dir, tid, version)
            if path not in cache:
                try:
                    cache[path] = wiitmd.load_ticket(path)
                except (OSError, ValueError):
                    cache[path] = None
            if cache[path] is not None:
                by_partition['ticket'].update(cache[path].tik_file())
        absent += missing
        by_partition['shared1'] = shared1_entries(sorted(shared_hashes))
        for partition, entries in by_partition.items():
            out_dir = os.path.join(output_dir, partition)
            os.makedirs(out_dir, exist_ok=True)
            lines = mtree_lines(entries, label, partition, HEADERS[partition],
                                missing, args.system)
            with open(os.path.join(out_dir, f'{label}.mtree'), 'w') as f:
                f.write('\n'.join(lines) + '\n')
            written[partition] += 1

    # the skeletons describe a Wii's console state; nothing sources vWii's
    if args.system == 'wii':
        for partition in SKELETONS:
            write_skeleton(output_dir, partition)
            written[partition] += 1

    total = sum(written.values())
    detail = ', '.join(f'{n} {p}' for p, n in sorted(written.items()))
    print(f'{total} mtrees written to {output_dir} ({detail})'
          + (f', {absent} title/version pairs had no TMD' if absent else ''))
    return 0 if total else 1


if __name__ == '__main__':
    sys.exit(main())
