#!/usr/bin/env python3
"""Collapse partitionspec mtrees that describe identical partition states.

A partition's contents often do not change between firmware versions — the
PS3's dev_flash3 is the same on all 98 of them — yet a generator that writes
one mtree per firmware stores that state 98 times over. This finds mtrees
within a partition whose entries are byte-identical (ignoring the header
comments, which name the firmware), keeps the earliest, and records the whole
set of firmwares in that file's header:

    # ... as installed by 98 firmwares: 1.02, 1.10, 1.11, ...

Grouping is by directory, so only files describing the same partition are ever
compared. When every firmware in a directory agrees, the directory collapses
to a single `<partition>.mtree` beside it, matching the layout the 3DS and DSi
generators produce.

Usage:
    dedupe-mtrees.py [--dry-run] [--collapse] <partitionspec-dir>...

--collapse turns a fully-uniform directory into one flat file. Without it the
directory is kept and just loses its redundant members. --dry-run reports what
would change and writes nothing.
"""
import argparse
import os
import re
import sys


def version_key(stem):
    """Order version stems naturally: 2.0E before 2.10E, 100 before 660go."""
    return [(0, int(p)) if p.isdigit() else (1, p)
            for p in re.split(r'(\d+)', stem) if p]


def body_of(text):
    """The mtree's entries, without the header comments naming the firmware."""
    return '\n'.join(l for l in text.splitlines() if not l.startswith('#'))


ATTRIBUTION = re.compile(
    r'^# as installed by (?:\d+ firmwares: (?P<many>.*)|firmware (?P<one>\S+))$', re.M)


def labels_of(path):
    """Return the firmware labels an mtree stands for.

    After deduplication one file may cover many firmwares, which its header
    records. Falls back to the filename for mtrees that predate that header.
    """
    with open(path) as f:
        m = ATTRIBUTION.search(f.read())
    if m and m.group('many'):
        return [x.strip() for x in m.group('many').split(',')]
    if m:
        return [m.group('one')]
    return [os.path.basename(path)[:-len('.mtree')]]


def index(root, is_partition=lambda name: True):
    """Map {firmware label: {partition: mtree path}} for a machine's tree.

    Handles both layouts: `<partition>.mtree` for a partition whose contents
    never vary, and `<partition>/<firmware>.mtree` where they do.
    """
    out = {}
    for entry in sorted(os.listdir(root)):
        path = os.path.join(root, entry)
        if entry.endswith('.mtree') and is_partition(entry[:-len('.mtree')]):
            partition, members = entry[:-len('.mtree')], [path]
        elif os.path.isdir(path) and is_partition(entry):
            partition = entry
            members = [os.path.join(path, n) for n in sorted(os.listdir(path))
                       if n.endswith('.mtree')]
        else:
            continue
        for member in members:
            for label in labels_of(member):
                out.setdefault(label, {})[partition] = member
    return out


def attribution(labels):
    if len(labels) == 1:
        return f'# as installed by firmware {labels[0]}'
    return f'# as installed by {len(labels)} firmwares: ' + ', '.join(labels)


def relabel(text, labels, partition):
    """Rewrite the header so it names every firmware this state belongs to.

    A description naming one version ("PS3 official firmware 1.02 system
    software") would be a lie once the file stands for 98 of them, so the
    version is dropped from it and a single attribution line carries the set.
    """
    lines = text.splitlines()
    head, rest = [], []
    for i, line in enumerate(lines):
        if line.startswith('#'):
            head.append(line)
        else:
            rest = lines[i:]
            break
    head = [l for l in head
            if ' as installed by ' not in l and 'identical across' not in l]
    # Drop the version this file happens to be named after from the
    # description, so it does not contradict the attribution below. The labels
    # are known exactly, so no guessing at what a version looks like.
    for label in labels:
        head = [re.sub(rf'\s+{re.escape(label)}\b', '', l) for l in head]
    head.append(attribution(labels))
    return '\n'.join(head + rest) + '\n'


def dedupe_dir(directory, dry_run=False, collapse=False):
    """Collapse duplicates in one partition directory. Returns (before, after)."""
    names = sorted((n for n in os.listdir(directory) if n.endswith('.mtree')),
                   key=lambda n: version_key(n[:-len('.mtree')]))
    if len(names) < 2:
        return len(names), len(names)

    groups = {}
    for name in names:
        with open(os.path.join(directory, name)) as f:
            text = f.read()
        groups.setdefault(body_of(text), []).append((name, text))

    partition = os.path.basename(directory)
    if collapse and len(groups) == 1:
        (members,) = groups.values()
        labels = [n[:-len('.mtree')] for n, _ in members]
        if not dry_run:
            flat = os.path.join(os.path.dirname(directory), f'{partition}.mtree')
            with open(flat, 'w') as f:
                f.write(relabel(members[0][1], labels, partition))
            for name, _ in members:
                os.remove(os.path.join(directory, name))
            os.rmdir(directory)
        return len(names), 1

    for members in groups.values():
        labels = [n[:-len('.mtree')] for n, _ in members]
        keeper, text = members[0]
        if not dry_run:
            with open(os.path.join(directory, keeper), 'w') as f:
                f.write(relabel(text, labels, partition))
            for name, _ in members[1:]:
                os.remove(os.path.join(directory, name))
    return len(names), len(groups)


def main():
    ap = argparse.ArgumentParser(add_help=False)
    ap.add_argument('--dry-run', action='store_true')
    ap.add_argument('--collapse', action='store_true')
    ap.add_argument('roots', nargs='*')
    args = ap.parse_args()
    if not args.roots:
        print(__doc__, file=sys.stderr)
        return 2

    roots = {os.path.normpath(r) for r in args.roots}
    total_before = total_after = 0
    for root in args.roots:
        for directory, subdirs, files in os.walk(root):
            subdirs.sort()
            if not any(f.endswith('.mtree') for f in files):
                continue
            # Only a partition directory holds versions of one partition, and
            # only those may be compared. A machine or device directory holds
            # one flat mtree per partition — those describe different things
            # and must never be merged, however alike they look. Both are
            # recognised structurally: they are a passed root, or they have
            # partition directories beneath them.
            if os.path.normpath(directory) in roots or subdirs:
                continue
            before, after = dedupe_dir(directory, args.dry_run, args.collapse)
            if before != after:
                rel = os.path.relpath(directory, os.path.dirname(root.rstrip('/')))
                print(f'  {rel}: {before} -> {after}')
            total_before += before
            total_after += after
    verb = 'would keep' if args.dry_run else 'kept'
    print(f'{total_before} mtrees, {verb} {total_after} '
          f'({total_before - total_after} redundant)')
    return 0


if __name__ == '__main__':
    sys.exit(main())
