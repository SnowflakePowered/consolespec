#!/usr/bin/env python3
"""Check the consolespec tree for the breakage that schema validation misses.

TOML Schema validates each file in isolation. This checks the things that
only show up across files: ids that disagree with filenames, references
that point at nothing, collection tables that disagree with the key
listing them, and mtrees that are unreferenced or malformed.

Usage:
    lint-consolespec.py [--schema PATH] [consolespec-dir]

With --schema, each spec file is additionally validated against its .tosd
using a toml-schema binary (https://toml-schema.org). Without it, only the
cross-file checks run.

Exits non-zero if any error is found. Warnings do not affect the exit code.
"""
import argparse
import os
import re
import subprocess
import sys
import tomllib

HEX = {'md5': 32, 'sha1': 40, 'sha256': 64}
MTREE_KEYWORDS = {'type', 'size', 'md5', 'sha1', 'sha256',
                  'md5digest', 'sha1digest', 'sha256digest'}


class Report:
    def __init__(self):
        self.errors, self.warnings = [], []

    def error(self, where, message):
        self.errors.append(f'{where}: {message}')

    def warn(self, where, message):
        self.warnings.append(f'{where}: {message}')


def load(path, report):
    try:
        with open(path, 'rb') as f:
            return tomllib.load(f)
    except (OSError, tomllib.TOMLDecodeError) as e:
        report.error(os.path.basename(path), f'unreadable: {e}')
        return None


def check_collection(doc, table, key, where, report):
    """A collection's listing key must name exactly its sibling tables."""
    section = doc.get(table)
    if not isinstance(section, dict) or key not in section:
        return []
    listed = section[key]
    present = [k for k in section if k != key]
    for name in listed:
        if name not in present:
            report.error(where, f'[{table}] {key} lists "{name}" with no [{table}.{name}] table')
    for name in present:
        if name not in listed:
            report.error(where, f'[{table}.{name}] exists but is not in {key}')
    return listed


def check_inputspecs(root, report):
    """Return the set of declared inputspec ids."""
    ids = {}
    directory = os.path.join(root, 'definitions', 'inputspec')
    for name in sorted(os.listdir(directory)):
        if not name.endswith('.toml'):
            continue
        doc = load(os.path.join(directory, name), report)
        if doc is None:
            continue
        spec_id = doc.get('input', {}).get('id')
        if spec_id is None:
            report.error(name, 'no [input] id')
            continue
        if f'{spec_id}.toml' != name:
            report.error(name, f'id "{spec_id}" does not match filename')
        if spec_id in ids:
            report.error(name, f'id "{spec_id}" also declared by {ids[spec_id]}')
        ids[spec_id] = name
    return ids


def check_bios(doc, where, report):
    for entry in doc.get('bios', []):
        name = entry.get('name', '?')
        lengths = {algo: len(entry[algo]) for algo in HEX if algo in entry}
        if len(set(lengths.values())) > 1:
            report.error(where, f'bios "{name}": checksum arrays differ in length {lengths}')
        for algo, width in HEX.items():
            for digest in entry.get(algo, []):
                if not re.fullmatch(f'[a-f0-9]{{{width}}}', digest):
                    report.error(where, f'bios "{name}": malformed {algo} "{digest}"')


def check_machinespecs(root, input_ids, report):
    """Return (machine ids, set of mtree paths referenced by specs)."""
    ids, referenced = {}, set()
    directory = os.path.join(root, 'definitions', 'machinespec')
    docs = {}
    for name in sorted(os.listdir(directory)):
        if not name.endswith('.toml'):
            continue
        doc = load(os.path.join(directory, name), report)
        if doc is None:
            continue
        docs[name] = doc
        machine_id = doc.get('machine', {}).get('id')
        if machine_id is None:
            report.error(name, 'no [machine] id')
            continue
        if f'{machine_id}.toml' != name:
            report.error(name, f'id "{machine_id}" does not match filename')
        if machine_id in ids:
            report.error(name, f'id "{machine_id}" also declared by {ids[machine_id]}')
        ids[machine_id] = name

    for name, doc in docs.items():
        for group in check_collection(doc, 'input', 'groups', name, report):
            entry = doc['input'].get(group, {})
            for ref in entry.get('inputs', []):
                if ref not in input_ids:
                    report.error(name, f'[input.{group}] references unknown inputspec "{ref}"')
            if not entry.get('inputs') and not entry.get('accessories'):
                report.warn(name, f'[input.{group}] has neither inputs nor accessories')

        for device in check_collection(doc, 'storage', 'devices', name, report):
            for partition in doc['storage'].get(device, {}).get('partition', []):
                for spec in partition.get('spec', []):
                    referenced.add(spec)
                    if not os.path.exists(os.path.join(
                            root, 'definitions', 'partitionspec', spec)):
                        report.error(name, f'partition "{partition.get("id")}" '
                                           f'references missing mtree {spec}')

        for dep in doc.get('machine', {}).get('depends-on', []):
            if dep not in ids:
                report.error(name, f'depends-on references unknown machine "{dep}"')

        # A machine that owns mtrees but declares no storage has lost its
        # [storage] section: partitionspec/<machine>/ exists with nothing
        # pointing at it. Silent section loss is otherwise invisible here,
        # since every remaining reference still resolves.
        owned = os.path.join(root, 'definitions', 'partitionspec',
                             doc.get('machine', {}).get('id', ''))
        if 'storage' not in doc and os.path.isdir(owned):
            report.error(name, f'has {owned and "partitionspec"} mtrees but no [storage] section')

        check_bios(doc, name, report)

    for spec_id, spec_file in input_ids.items():
        used = any(spec_id in doc.get('input', {}).get(g, {}).get('inputs', [])
                   for doc in docs.values()
                   for g in doc.get('input', {}).get('groups', []))
        if not used:
            report.warn(spec_file, f'inputspec "{spec_id}" is not referenced by any machine')
    return ids, referenced


def check_mtrees(root, referenced, report):
    directory = os.path.join(root, 'definitions', 'partitionspec')
    if not os.path.isdir(directory):
        return
    for dirpath, _dirnames, filenames in os.walk(directory):
        for name in sorted(filenames):
            if not name.endswith('.mtree'):
                continue
            path = os.path.join(dirpath, name)
            rel = os.path.relpath(path, directory)
            if rel not in referenced:
                report.warn(rel, 'mtree is not referenced by any machinespec')
            seen = set()
            for lineno, line in enumerate(open(path), 1):
                line = line.strip()
                if not line or line.startswith('#'):
                    continue
                entry, *keywords = line.split()
                where = f'{rel}:{lineno}'
                if entry in seen:
                    report.error(where, f'duplicate entry {entry}')
                seen.add(entry)
                kv = {}
                for keyword in keywords:
                    if '=' not in keyword:
                        report.error(where, f'malformed keyword "{keyword}"')
                        continue
                    key, value = keyword.split('=', 1)
                    kv[key] = value
                    if key not in MTREE_KEYWORDS:
                        report.warn(where, f'unknown keyword "{key}"')
                if kv.get('type') not in ('dir', 'file'):
                    report.error(where, f'entry has no valid type= ({kv.get("type")})')
                # `size` is optional in mtree: a file whose size is not fixed
                # (a journal sized to its volume, say) legitimately omits it.
                for algo, width in HEX.items():
                    for key in (algo, f'{algo}digest'):
                        if key in kv and not re.fullmatch(f'[a-f0-9]{{{width}}}', kv[key]):
                            report.error(where, f'malformed {key} "{kv[key]}"')


def check_schema(root, binary, report):
    for kind in ('inputspec', 'machinespec'):
        schema = os.path.join(root, 'schema', f'{kind}.tosd')
        if not os.path.exists(schema):
            report.warn(f'{kind}.tosd', 'schema not found; skipping validation')
            continue
        directory = os.path.join(root, 'definitions', kind)
        for name in sorted(os.listdir(directory)):
            if not name.endswith('.toml'):
                continue
            result = subprocess.run([binary, 'validate', schema, os.path.join(directory, name)],
                                    capture_output=True, text=True)
            if result.returncode != 0:
                detail = ' / '.join(l.strip() for l in result.stdout.splitlines()[1:] if l.strip())
                report.error(name, f'schema: {detail or result.stderr.strip()}')


def main():
    ap = argparse.ArgumentParser(add_help=False)
    ap.add_argument('--schema', help='path to a toml-schema binary')
    ap.add_argument('root', nargs='?', default=os.path.join(
        os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
    args = ap.parse_args()

    root = args.root
    if not os.path.isdir(os.path.join(root, 'definitions', 'inputspec')):
        print(f'error: {root} does not look like a consolespec directory', file=sys.stderr)
        return 2

    report = Report()
    input_ids = check_inputspecs(root, report)
    machine_ids, referenced = check_machinespecs(root, input_ids, report)
    check_mtrees(root, referenced, report)
    if args.schema:
        check_schema(root, args.schema, report)

    for warning in report.warnings:
        print(f'warn  {warning}')
    for error in report.errors:
        print(f'ERROR {error}')
    print(f'{len(input_ids)} inputspecs, {len(machine_ids)} machinespecs, '
          f'{len(referenced)} referenced mtrees: '
          f'{len(report.errors)} errors, {len(report.warnings)} warnings')
    return 1 if report.errors else 0


if __name__ == '__main__':
    sys.exit(main())
