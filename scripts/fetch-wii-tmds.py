#!/usr/bin/env python3
"""Fetch the signed metadata every Wii system update is made of, from NUS.

Reads the update title lists out of WiiQt's nusdownloader.cpp, then
downloads each title version's TMD, and its ticket if --tickets is given,
into a cache directory:

    <tmd-dir>/<title id>.<version>.tmd
    <tmd-dir>/<title id>.<version>.tik

Contents themselves are never downloaded — the TMD is signed and records
the size and SHA-1 of each content, so the manifests are exact without them.

WiiQt leaves the stub IOSes' version unpinned, meaning "whatever NUS serves
as newest". Those are fetched from the unversioned TMD path and saved as
<title id>.latest.tmd, so the generator resolves them the same way offline.

Usage:
    fetch-wii-tmds.py [--tickets] [--jobs N] <nusdownloader.cpp> <tmd-dir>

Already-cached files are left alone, so re-running only fetches what is
missing. Exits non-zero if anything failed to download.
"""
import argparse
import concurrent.futures
import os
import sys
import urllib.error
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import wiiupd  # noqa: E402

NUS = 'http://nus.cdn.shop.wii.com/ccs/download'


def fetch(url):
    req = urllib.request.Request(url, headers={'User-Agent': 'wii libnup/1.0'})
    with urllib.request.urlopen(req, timeout=60) as r:
        return r.read()


def jobs_for(updates):
    """Return sorted unique (title id, version) pairs across all updates."""
    return sorted({(tid, version)
                   for titles in updates.values()
                   for tid, version in titles.items()})


def download(tmd_dir, tid, version, tickets):
    """Fetch one title version's metadata. Returns (ok, message or None)."""
    tid_hex = f'{tid:016x}'
    latest = version == wiiupd.LATEST
    tag = 'latest' if latest else str(version)
    want = [('tmd' if latest else f'tmd.{version}', f'{tid_hex}.{tag}.tmd')]
    if tickets:
        want.append(('cetk', f'{tid_hex}.{tag}.tik'))
    for remote, local in want:
        path = os.path.join(tmd_dir, local)
        if os.path.exists(path) and os.path.getsize(path):
            continue
        try:
            blob = fetch(f'{NUS}/{tid_hex}/{remote}')
        except (urllib.error.URLError, OSError) as e:
            reason = getattr(e, 'code', None) or e
            return False, f'{tid_hex} v{tag} {remote}: {reason}'
        tmp = path + '.part'
        with open(tmp, 'wb') as f:
            f.write(blob)
        os.replace(tmp, path)
    return True, None


def main():
    ap = argparse.ArgumentParser(add_help=False)
    ap.add_argument('--tickets', action='store_true')
    ap.add_argument('--jobs', type=int, default=8)
    ap.add_argument('source', nargs='?')
    ap.add_argument('tmd_dir', nargs='?')
    args = ap.parse_args()
    if not args.source or not args.tmd_dir:
        print(__doc__, file=sys.stderr)
        return 2

    try:
        updates = wiiupd.parse(args.source)
    except OSError as e:
        print(f'error: {e}', file=sys.stderr)
        return 2
    if not updates:
        print(f'error: no update lists in {args.source}', file=sys.stderr)
        return 2

    os.makedirs(args.tmd_dir, exist_ok=True)
    pairs = jobs_for(updates)
    done = failures = 0
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        futures = {pool.submit(download, args.tmd_dir, tid, version, args.tickets):
                   (tid, version) for tid, version in pairs}
        for future in concurrent.futures.as_completed(futures):
            ok, message = future.result()
            if ok:
                done += 1
            else:
                failures += 1
                print(f'FAIL {message}', file=sys.stderr)
    print(f'{done}/{len(pairs)} title versions cached in {args.tmd_dir}'
          + (f', {failures} failed' if failures else ''))
    return 1 if failures else 0


if __name__ == '__main__':
    sys.exit(main())
