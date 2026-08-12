"""Nintendo Wii U MLC system-title update history.

Ninupdates archived Nintendo's GetSystemUpdate replies and associates every
title version with the system update that first shipped it.  The CSV snapshot
from the final 2022-08-30 scan is cumulative, so it is sufficient to rebuild
every recorded retail MLC state without guessing from title-version numbers.

Only MLC titles are selected here.  The update reply also contains five SLC
titles (Cafe OS, IOSU and update infrastructure) and three hardware-firmware
titles.  The split below is the 52-title MLC set used by
Xpl0itU/MLCRestorerDownloader, cross-checked against the title IDs in the
archived Nintendo replies.
"""
import csv
import os
import re
import urllib.parse


REPORT_DATE = '2022-08-30_00-00-38'
HISTORY_URL = 'https://yls8.mtheall.com/ninupdates/titlelist.php'

REGIONS = {
    'E': ('USA', 'U'),
    'P': ('EUR', 'E'),
    'J': ('JPN', 'J'),
}

# Region-specific low words follow a regular 00/01/02 convention, but keeping
# the explicit list makes the MLC/SLC boundary auditable and avoids treating a
# newly observed title as MLC merely because its ID looks similar.
MLC_REGIONAL = {
    'EUR': '''
0005001010040200 0005001010041200 0005001010043200 0005001010044200
0005001010045200 0005001010047200 0005001010048200 0005001010049200
000500101004a200 000500101004b200 000500101004c200 000500101004d200
000500101004e200 000500101005a200 0005001010062200 0005001b10059200
0005001b10067200 0005001b10069200 0005003010010209 000500301001020a
0005003010011209 000500301001120a 00050030100112ff 000500301001220a
000500301001320a 000500301001420a 000500301001520a 000500301001620a
0005003010017209 000500301001720a 000500301001820a 000500301001920a
000500301006d20a'''.split(),
    'USA': '''
0005001010040100 0005001010041100 0005001010043100 0005001010044100
0005001010045100 0005001010047100 0005001010048100 0005001010049100
000500101004a100 000500101004b100 000500101004c100 000500101004d100
000500101004e100 000500101005a100 0005001010062100 0005001b10059100
0005001b10067100 0005001b10069100 0005003010010109 0005003010011109
000500301001010a 000500301001110a 00050030100111ff 000500301001210a
000500301001310a 000500301001410a 000500301001510a 000500301001610a
0005003010017109 000500301001710a 000500301001810a 000500301001910a
000500301006d10a'''.split(),
    'JPN': '''
0005001010040000 0005001010041000 0005001010043000 0005001010044000
0005001010045000 0005001010047000 0005001010048000 0005001010049000
000500101004a000 000500101004b000 000500101004c000 000500101004d000
000500101004e000 000500101005a000 0005001010062000 0005001b10059000
0005001b10067000 0005001b10069000 0005003010010009 0005003010011009
000500301001000a 000500301001100a 00050030100110ff 000500301001200a
000500301001300a 000500301001400a 000500301001500a 000500301001600a
0005003010017009 000500301001700a 000500301001800a 000500301001900a
000500301006d00a'''.split(),
}

MLC_COMMON = '''
0005001010066000 0005001b10042300 0005001b10042400 0005001b1004f000
0005001b10050000 0005001b10051000 0005001b10052000 0005001b10053000
0005001b10054000 0005001b10056000 0005001b10057000 0005001b10058000
0005001b1005c000 0005001b1005f000 0005001b10063000 0005001b10065000
0005001b10068000 0005001b1006c000 000500301001a10a'''.split()

MLC_REGIONAL = {region: frozenset(x.lower() for x in titles)
                for region, titles in MLC_REGIONAL.items()}
MLC_COMMON = frozenset(x.lower() for x in MLC_COMMON)


def history_url(region, date=REPORT_DATE):
    return HISTORY_URL + '?' + urllib.parse.urlencode({
        'csv': 1, 'date': date, 'reg': region, 'soap': 1, 'sys': 'wup',
    })


def history_path(cache_dir, region):
    return os.path.join(cache_dir, 'history', f'{region}.csv')


def allowed_titles(region):
    region_name, _suffix = REGIONS[region]
    return MLC_COMMON | MLC_REGIONAL[region_name]


def _event_name(raw, region):
    """Normalize Ninupdates scan labels to the displayed firmware version."""
    raw = raw.removesuffix('_Initial_scan')
    # A preliminary PAL scan contains VersionData v17, followed by the full
    # 2.0.0-E scan.  They are two observations of the same installed release.
    if region == 'P' and raw in ('2.0.0', '2.0.0-E'):
        return '2.0.0'
    # Japan's archive begins with VersionData v17 at 2.0.0 but has no complete
    # title list until the 2.1.0-J scan; fold the seed into that first state.
    if region == 'J' and raw == '2.0.0':
        return '2.1.0'
    if raw == '10-07-13_JPN':
        return '4.0.1'
    raw = re.sub(r'-(?:U|E|J)$', '', raw)
    return raw


def version_key(version):
    return tuple(int(x) for x in version.split('.'))


def read_updates(csv_path, region):
    """Return ordered {firmware-region label: {title id: title version}}."""
    changes = {}
    allowed = allowed_titles(region)
    with open(csv_path, newline='', encoding='utf-8-sig') as f:
        for row in csv.DictReader(f):
            tid = row['TitleID'].lower()
            if tid not in allowed:
                continue
            versions = row['Title versions'].split()
            events = row['Update versions'].split()
            if len(versions) != len(events):
                raise ValueError(f'{csv_path}: {tid}: title/update count differs')
            for version, event in zip(versions, events):
                if not version.startswith('v') or not version[1:].isdigit():
                    raise ValueError(f'{csv_path}: {tid}: invalid version {version}')
                event = _event_name(event, region)
                changes.setdefault(event, {})[tid] = int(version[1:])

    _region_name, suffix = REGIONS[region]
    state, out = {}, {}
    for event in sorted(changes, key=version_key):
        state.update(changes[event])
        out[f'{event}-{suffix}'] = dict(state)
    return out


def all_pairs(updates_by_region):
    return sorted({(tid, version)
                   for updates in updates_by_region.values()
                   for state in updates.values()
                   for tid, version in state.items()})
