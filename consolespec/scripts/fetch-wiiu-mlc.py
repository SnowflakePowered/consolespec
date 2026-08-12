#!/usr/bin/env python3
"""Download every recorded Wii U MLC system-title package from Nintendo NUS.

The title/version history is fetched from Ninupdates' preserved final Wii U
GetSystemUpdate scan.  TMDs, tickets and encrypted contents come from
Nintendo's CDN and are kept in a cache outside the repository by convention.

Usage:
    fetch-wiiu-mlc.py [--date TIMESTAMP] [--regions EPJ] [--jobs N] <cache-dir>

The default snapshot reconstructs all 75 complete regional firmware states
from 2.0.0-U / 2.0.0-E / 2.1.0-J through the final releases.  Re-running is
resumable and leaves already complete files untouched.
"""
import argparse
import concurrent.futures
import os
import shutil
import sys
import time
import urllib.error
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import wiiutmd  # noqa: E402
import wiiuupd  # noqa: E402


CDN = 'http://ccs.cdn.c.shop.nintendowifi.net/ccs/download'


def fetch(url, path, expected=None, retries=4):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    if os.path.exists(path) and (expected is None or os.path.getsize(path) == expected):
        return 0
    part = path + '.part'
    for attempt in range(retries):
        have = os.path.getsize(part) if os.path.exists(part) else 0
        headers = {'User-Agent': 'consolespec/1.0'}
        if have:
            headers['Range'] = f'bytes={have}-'
        try:
            req = urllib.request.Request(url, headers=headers)
            with urllib.request.urlopen(req, timeout=120) as src:
                append = have and src.status == 206
                if not append:
                    have = 0
                with open(part, 'ab' if append else 'wb') as dst:
                    shutil.copyfileobj(src, dst, 1024 * 1024)
            size = os.path.getsize(part)
            if expected is not None and size != expected:
                raise OSError(f'size {size} != {expected}')
            os.replace(part, path)
            return size - have
        except (OSError, urllib.error.URLError) as e:
            if attempt + 1 == retries:
                raise OSError(f'{url}: {e}') from e
            time.sleep(2 ** attempt)


def get_histories(cache, regions, date):
    updates = {}
    for region in regions:
        path = wiiuupd.history_path(cache, region)
        fetch(wiiuupd.history_url(region, date), path)
        updates[region] = wiiuupd.read_updates(path, region)
    return updates


def metadata_job(cache, pair):
    tid, version = pair
    path = os.path.join(cache, 'metadata', f'{tid}.{version}.tmd')
    return fetch(f'{CDN}/{tid}/tmd.{version}', path)


def ticket_job(cache, tid):
    path = os.path.join(cache, 'tickets', f'{tid}.tik')
    return fetch(f'{CDN}/{tid}/cetk', path)


def content_jobs(cache, pairs):
    jobs = []
    for tid, version in pairs:
        tmd = wiiutmd.load_tmd(os.path.join(cache, 'metadata', f'{tid}.{version}.tmd'))
        if f'{tmd.title_id:016x}' != tid or tmd.title_version != version:
            raise ValueError(f'{tid} v{version}: TMD identity mismatch')
        directory = os.path.join(cache, 'titles', tid, str(version))
        for content in tmd.contents:
            name = f'{content["id"]:08x}'
            size = content['size'] if content['type'] & wiiutmd.HASHED \
                else (content['size'] + 15) & ~15
            jobs.append((f'{CDN}/{tid}/{name}', os.path.join(directory, name + '.app'), size))
            if content['type'] & wiiutmd.HASHED:
                chunks = (content['size'] + 0xFFFF) // 0x10000
                h3_size = ((chunks + 4095) // 4096) * 20
                jobs.append((f'{CDN}/{tid}/{name}.h3',
                             os.path.join(directory, name + '.h3'), h3_size))
    return jobs


def run_jobs(label, jobs, fn, workers):
    failures, downloaded = [], 0
    with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as pool:
        futures = {pool.submit(fn, job): job for job in jobs}
        for done, future in enumerate(concurrent.futures.as_completed(futures), 1):
            job = futures[future]
            try:
                downloaded += future.result()
            except Exception as e:  # noqa: BLE001
                failures.append((job, str(e)))
            if done % 100 == 0 or done == len(jobs):
                print(f'{label}: {done}/{len(jobs)}, {downloaded / 2**30:.2f} GiB new',
                      flush=True)
    for job, error in failures[:30]:
        print(f'FAIL {job}: {error}', file=sys.stderr)
    if len(failures) > 30:
        print(f'... and {len(failures) - 30} more failures', file=sys.stderr)
    return not failures


def main():
    ap = argparse.ArgumentParser(add_help=False)
    ap.add_argument('--date', default=wiiuupd.REPORT_DATE)
    ap.add_argument('--regions', default='EPJ')
    ap.add_argument('--jobs', type=int, default=8)
    ap.add_argument('cache', nargs='?')
    args = ap.parse_args()
    if not args.cache or any(r not in wiiuupd.REGIONS for r in args.regions):
        print(__doc__, file=sys.stderr)
        return 2
    cache = os.path.abspath(args.cache)
    os.makedirs(cache, exist_ok=True)
    updates = get_histories(cache, args.regions, args.date)
    pairs = wiiuupd.all_pairs(updates)
    tids = sorted({tid for tid, _version in pairs})
    print(f'{len(pairs)} unique title versions ({len(tids)} titles)')

    ok = run_jobs('metadata', pairs,
                  lambda pair: metadata_job(cache, pair), args.jobs)
    ok &= run_jobs('tickets', tids,
                   lambda tid: ticket_job(cache, tid), args.jobs)
    if not ok:
        return 1
    jobs = content_jobs(cache, pairs)
    ok &= run_jobs('contents', jobs,
                   lambda job: fetch(*job), args.jobs)
    return 0 if ok else 1


if __name__ == '__main__':
    sys.exit(main())
