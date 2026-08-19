#!/usr/bin/env python3
"""Generate Wii U MLC /sys manifests from downloaded system titles.

Usage:
    gen-wiiu-mtrees.py [--regions EPJ] [--jobs N] <cache-dir> <output-dir>

Each title version is decrypted and hashed once, then cached as a small JSON
manifest.  The cumulative Ninupdates histories select those manifests into one
mtree per complete regional firmware state.  NUS packages remain encrypted in
the cache; no copyrighted content is written to the repository.
"""
import argparse
import concurrent.futures
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import wiiutmd  # noqa: E402
import wiiuupd  # noqa: E402


def manifest_path(cache, tid, version):
    return os.path.join(cache, 'manifests', tid, f'{version}.json')


def build_manifest(job):
    cache, tid, version = job
    path = manifest_path(cache, tid, version)
    if os.path.exists(path):
        return tid, version, False
    dirs, files = wiiutmd.installed_entries(cache, tid, version)
    doc = {
        'directories': sorted(dirs),
        'files': {name: {'size': size, 'sha256': digest}
                  for name, (size, digest) in sorted(files.items())},
    }
    os.makedirs(os.path.dirname(path), exist_ok=True)
    tmp = path + '.part'
    with open(tmp, 'w', encoding='utf-8', newline='\n') as f:
        json.dump(doc, f, sort_keys=True, separators=(',', ':'))
        f.write('\n')
    os.replace(tmp, path)
    return tid, version, True


def load_manifest(cache, tid, version):
    with open(manifest_path(cache, tid, version), encoding='utf-8') as f:
        return json.load(f)


def mtree_for(cache, label, titles):
    dirs = {'.', 'title'}
    files = {}
    for tid, version in sorted(titles.items()):
        high, low = tid[:8], tid[8:]
        base = f'title/{high}/{low}'
        dirs.update((f'title/{high}', base))
        doc = load_manifest(cache, tid, version)
        dirs.update(f'{base}/{path}' for path in doc['directories'])
        for path, attrs in doc['files'].items():
            files[f'{base}/{path}'] = attrs
    lines = [
        '#mtree',
        f'# Nintendo Wii U {label} installed MLC /sys state',
        '# title membership and versions are from Ninupdates archives of Nintendo GetSystemUpdate replies',
        '# file paths and sizes come from each title FST; sha256 is calculated from decrypted NUS content',
    ]
    lines.extend(f'./{path} type=dir' if path != '.' else '. type=dir'
                 for path in sorted(dirs))
    lines.extend(f'./{path} type=file size={attrs["size"]} sha256={attrs["sha256"]}'
                 for path, attrs in sorted(files.items()))
    return '\n'.join(lines) + '\n'


def main():
    ap = argparse.ArgumentParser(add_help=False)
    ap.add_argument('--regions', default='EPJ')
    ap.add_argument('--jobs', type=int, default=4)
    ap.add_argument('cache', nargs='?')
    ap.add_argument('output', nargs='?')
    args = ap.parse_args()
    if not args.cache or not args.output or any(r not in wiiuupd.REGIONS for r in args.regions):
        print(__doc__, file=sys.stderr)
        return 2
    cache, output = os.path.abspath(args.cache), os.path.abspath(args.output)
    updates = {region: wiiuupd.read_updates(wiiuupd.history_path(cache, region), region)
               for region in args.regions}
    pairs = wiiuupd.all_pairs(updates)
    jobs = [(cache, tid, version) for tid, version in pairs]
    failures, made = [], 0
    with concurrent.futures.ProcessPoolExecutor(max_workers=args.jobs) as pool:
        futures = {pool.submit(build_manifest, job): job for job in jobs}
        for done, future in enumerate(concurrent.futures.as_completed(futures), 1):
            try:
                _tid, _version, created = future.result()
                made += created
            except Exception as e:  # noqa: BLE001
                failures.append((futures[future], str(e)))
            if done % 25 == 0 or done == len(jobs):
                print(f'title manifests: {done}/{len(jobs)} ({made} new)', flush=True)
    for job, error in failures[:30]:
        print(f'FAIL {job[1]} v{job[2]}: {error}', file=sys.stderr)
    if failures:
        return 1

    os.makedirs(output, exist_ok=True)
    written = 0
    for region in args.regions:
        for label, titles in updates[region].items():
            path = os.path.join(output, f'{label}.mtree')
            with open(path, 'w', encoding='utf-8', newline='\n') as f:
                f.write(mtree_for(cache, label, titles))
            written += 1
    print(f'{written} Wii U MLC /sys mtrees written to {output}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
