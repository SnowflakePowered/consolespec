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

    <output-dir>/title/<version>.mtree     <hi>/<lo>/content/*.app, title.tmd
    <output-dir>/ticket/<version>.mtree    <hi>/<lo>.tik
    <output-dir>/shared1/<version>.mtree   *.app, content.map

Each mtree is rooted at its own partition — `.` is the partition, so entries
do not repeat its name; see rooted().

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
and shared1 come out exactly as they do for a Wii, and it gets its own set of
console-state skeletons — see VWII_SKELETONS, which differ from the Wii's.
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
# A second, unrelated console — the vWii dump behind VWII_SKELETONS — settled
# several more. RFL_DB.dat, config.dat, dhcp.dat, nanderr.log, space.sys and
# both WiiConnect24 mailboxes carry the same size on both, so they are
# preallocated formats rather than one unit's state, and are sized here.
# wiibrew's offset map for RFL_DB.dat, which ends it at 0x1F1E0, describes only
# its Mii and Mii Parade sections and not the whole file. Dolphin's 48-byte
# mailboxes are the empty copies it ships, not the size a console allocates.
# uid.sys still varies between the two dumps (2616 against 672) and stays
# unsized, as do the files only one console has.
#
# Paths wiibrew lists that the dumps do not have are kept: several are
# conditional (a WiiConnect24 download counter, a play-time limit), and their
# absence on two consoles is not evidence they never exist. Artifacts of a
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
            'shared2/menu/FaceLib/RFL_DB.dat': 779968,
            'shared2/menu/vc/settings.sav': 32,
            'shared2/succession/transfer.id': 32,
            'shared2/sys/NANDBOOTINFO': 4128,
            'shared2/sys/SYSCONF': 16384,
            'shared2/sys/flags.dat': 32,
            'shared2/sys/net/02/config.dat': 7004,
            'shared2/sys/net/dhcp.dat': 96,
            'shared2/test2/nanderr.log': 16384,
            'shared2/wc24/mbox/wc24recv.ctl': 32768,
            'shared2/wc24/mbox/wc24recv.mbx': 7340032,
            'shared2/wc24/mbox/wc24send.ctl': 16384,
            'shared2/wc24/mbox/wc24send.mbx': 2097152,
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
            'shared2/succession/shop.log', 'shared2/test/testlog.txt',
            'shared2/test2/dvderror.dat', 'shared2/wc24/dlcnt.bin',
        ],
    },
    'sys': {
        'note': 'IOS storage; uid.sys and the .sys files track this console\'s own state',
        'dirs': ['sys'],
        'files': {'sys/cert.sys': 2560, 'sys/space.sys': 19140},
        'unsized': [
            'sys/boot.sys', 'sys/cc.sys', 'sys/disc.sys', 'sys/launch.sys',
            'sys/uid.sys',
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


# The same idea for vWii's SLCCMPT, read from a retail 5.2.0E dump. vWii's
# console state is a subset of a Wii's plus two additions of its own — a
# `shared2/sys/compat` directory and a top-level `wfs` one for the Wii U
# filesystem shim — and it has no `meta`, which on a Wii is written by a
# factory install disc vWii never had.
#
# Sizes here are the ones the vWii dump and the Wii dump agree on, which is
# what promoted several of them out of the Wii table's `unsized` list: two
# unrelated consoles landing on 779968 for RFL_DB.dat, 7004 for config.dat and
# 7340032 for the WiiConnect24 inbox means those are preallocated formats
# rather than one unit's state. uid.sys differs between them (672 against 2616)
# and stays unsized.
VWII_SKELETONS = {
    'shared2': {
        'note': 'setup data for WiiConnect24, Miis and the Wii U compatibility layer',
        'dirs': [
            'shared2', 'shared2/menu', 'shared2/menu/FaceLib', 'shared2/menu/vc',
            'shared2/succession', 'shared2/sys', 'shared2/sys/compat',
            'shared2/sys/net', 'shared2/sys/net/02', 'shared2/test2',
            'shared2/wc24', 'shared2/wc24/mbox',
        ],
        'files': {
            'shared2/menu/FaceLib/RFL_DB.dat': 779968,
            'shared2/menu/vc/settings.sav': 32,
            'shared2/succession/transfer.id': 32,
            'shared2/sys/SYSCONF': 16384,
            'shared2/sys/flags.dat': 32,
            'shared2/sys/net/02/config.dat': 7004,
            'shared2/sys/net/dhcp.dat': 96,
            'shared2/test2/nanderr.log': 16384,
            'shared2/wc24/mbox/wc24recv.ctl': 32768,
            'shared2/wc24/mbox/wc24recv.mbx': 7340032,
            'shared2/wc24/mbox/wc24send.ctl': 16384,
            'shared2/wc24/mbox/wc24send.mbx': 2097152,
            'shared2/wc24/misc.bin': 1024,
            'shared2/wc24/nwc24dl.bin': 63488,
            'shared2/wc24/nwc24fl.bin': 32864,
            'shared2/wc24/nwc24fls.bin': 12800,
            'shared2/wc24/nwc24msg.cbk': 1024,
            'shared2/wc24/nwc24msg.cfg': 1024,
        },
        'unsized': [],
    },
    'sys': {
        'note': 'IOS storage; uid.sys tracks this console\'s own install history',
        'dirs': ['sys'],
        'files': {'sys/cert.sys': 2560, 'sys/space.sys': 19140},
        'unsized': ['sys/uid.sys'],
    },
    'meta': {
        'note': 'present but empty: vWii has no factory install disc to write title.met',
        'dirs': ['meta'], 'files': {}, 'unsized': [],
    },
    'import': {
        'note': 'scratch space for an in-progress title install; empty unless one was interrupted',
        'dirs': ['import'], 'files': {}, 'unsized': [],
    },
    'tmp': {
        'note': 'scratch space cleared by the System Menu on boot; empty at rest',
        'dirs': ['tmp'], 'files': {}, 'unsized': [],
    },
    'wfs': {
        'note': 'mount point for the Wii U filesystem BC-WFS exposes; empty on NAND',
        'dirs': ['wfs'], 'files': {}, 'unsized': [],
    },
}


def skeletons_for(system):
    return VWII_SKELETONS if system == 'vwii' else SKELETONS


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


def rooted(partition, paths):
    """Re-root NAND paths at their partition, dropping its leading name.

    Every mtree describes one partition, so `.` is that partition's root and
    repeating its name in each entry would be redundant — `title/00000001/...`
    is written `00000001/...` in the title mtree. Matches how the PS3 and Vita
    partitionspecs are rooted.
    """
    prefix = partition + '/'
    out = {}
    for path, value in paths.items():
        if path == partition:
            continue      # the partition directory is the mtree's own root
        out[path[len(prefix):] if path.startswith(prefix) else path] = value
    return out


def mtree_lines(entries, label, partition, header, missing, system='wii'):
    lines = ['#mtree',
             '# ' + TITLES[system].format(label=label, partition=partition),
             f'# {header}']
    if missing:
        lines.append(f'# {missing} title(s) omitted: no metadata cached from NUS')
    lines.append('. type=dir')
    entries = rooted(partition, entries)
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
    names is still fixed — N contents always occupy 00000000.app upwards — so
    the names are recorded, but none can be given a size or a digest.

    content.map is one 28-byte record per shared content, but the count is of
    everything installed, not just the system software: a console with any
    channel or game on it has a longer one than the system titles alone imply.
    It is therefore listed without a size too, rather than asserting a figure
    that only holds for a console carrying nothing else.
    """
    entries = {'shared1/content.map': (None, None)}
    for i in range(len(shared_hashes)):
        entries[f'shared1/{i:08x}.app'] = (None, None)
    return entries


def write_skeleton(output_dir, partition, system='wii'):
    """Write one console-state partition skeleton. Returns its path."""
    spec = skeletons_for(system)[partition]
    lines = ['#mtree',
             '# ' + ('Nintendo Wii U vWii SLCCMPT' if system == 'vwii'
                    else 'Nintendo Wii') + f' {partition} partition skeleton',
             f'# {spec["note"]}',
             '# console state, not update content: paths only, never a digest,',
             '# and sizes only where a source fixes them',
             '. type=dir']
    entries = {p: (s, None) for p, s in spec['files'].items()}
    entries.update({p: (None, None) for p in spec['unsized']})
    entries = rooted(partition, entries)
    dirs = rooted(partition, {d: None for d in spec['dirs']})
    rows = [(d, None) for d in dirs] + list(entries.items())
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
    'shared1': 'names are allocated in install order and content.map counts '
               'everything installed, not just the system software, so nothing '
               'here carries a size or a digest',
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

    for partition in skeletons_for(args.system):
        write_skeleton(output_dir, partition, args.system)
        written[partition] += 1

    total = sum(written.values())
    detail = ', '.join(f'{n} {p}' for p, n in sorted(written.items()))
    print(f'{total} mtrees written to {output_dir} ({detail})'
          + (f', {absent} title/version pairs had no TMD' if absent else ''))
    return 0 if total else 1


if __name__ == '__main__':
    sys.exit(main())
