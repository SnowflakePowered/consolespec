#!/usr/bin/env python3
"""Verify Wii U MLC mtrees against cached Nintendo NUS system titles.

Usage:
    verify-wiiu-mtrees.py [--regions EPJ] [--jobs N] <cache-dir> <mtree-dir>

Every title is decrypted and hashed again, its compact intermediate manifest
is checked, and every cumulative firmware mtree is then reproduced byte for
byte from Ninupdates' archived update history.
"""
import argparse
import concurrent.futures
import importlib.util
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import wiiutmd  # noqa: E402
import wiiuupd  # noqa: E402

spec = importlib.util.spec_from_file_location(
    'gen_wiiu_mtrees', os.path.join(HERE, 'gen-wiiu-mtrees.py'))
gen = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gen)


def verify_title(job):
    cache, tid, version = job
    dirs, files = wiiutmd.installed_entries(cache, tid, version)
    actual = {
        'directories': sorted(dirs),
        'files': {name: {'size': size, 'sha256': digest}
                  for name, (size, digest) in sorted(files.items())},
    }
    with open(gen.manifest_path(cache, tid, version), encoding='utf-8') as f:
        expected = json.load(f)
    if actual != expected:
        raise ValueError('cached title manifest differs')
    return tid, version


def main():
    ap = argparse.ArgumentParser(add_help=False)
    ap.add_argument('--regions', default='EPJ')
    ap.add_argument('--jobs', type=int, default=4)
    ap.add_argument('cache', nargs='?')
    ap.add_argument('mtree_dir', nargs='?')
    args = ap.parse_args()
    if not args.cache or not args.mtree_dir or any(
            r not in wiiuupd.REGIONS for r in args.regions):
        print(__doc__, file=sys.stderr)
        return 2
    cache = os.path.abspath(args.cache)
    mtree_dir = os.path.abspath(args.mtree_dir)
    updates = {r: wiiuupd.read_updates(wiiuupd.history_path(cache, r), r)
               for r in args.regions}
    jobs = [(cache, tid, version)
            for tid, version in wiiuupd.all_pairs(updates)]
    failures = []
    with concurrent.futures.ProcessPoolExecutor(max_workers=args.jobs) as pool:
        futures = {pool.submit(verify_title, job): job for job in jobs}
        for done, future in enumerate(concurrent.futures.as_completed(futures), 1):
            try:
                future.result()
            except Exception as e:  # noqa: BLE001
                failures.append((futures[future], str(e)))
            if done % 50 == 0 or done == len(jobs):
                print(f'titles: {done}/{len(jobs)}', flush=True)

    checked = 0
    for region in args.regions:
        for label, titles in updates[region].items():
            path = os.path.join(mtree_dir, f'{label}.mtree')
            try:
                with open(path, encoding='utf-8') as f:
                    actual = f.read()
                expected = gen.mtree_for(cache, label, titles)
                if actual != expected:
                    failures.append(((label,), 'mtree differs from regenerated state'))
            except OSError as e:
                failures.append(((label,), str(e)))
            checked += 1

    for job, error in failures[:30]:
        print(f'FAIL {" ".join(map(str, job))}: {error}', file=sys.stderr)
    print(f'{len(jobs)} title versions and {checked} firmware mtrees checked: '
          f'{len(failures)} failures')
    return 1 if failures else 0


if __name__ == '__main__':
    sys.exit(main())
