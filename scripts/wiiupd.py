"""Wii system update title lists, read from WiiQt's NUS downloader.

Nintendo never published which titles a Wii system update installs, but
WiiQt transcribed the lists from the update partitions of retail discs into
`WiiQt/nusdownloader.cpp`, one `ListXXr()` function per update. Each returns
a title id -> title version map, usually by starting from the previous
update's list and overriding the entries that changed:

    QMap< quint64, quint16 > NusDownloader::List43u()
    {
        QMap< quint64, quint16 > titles = List42u();
        titles.insert( 0x100000050ull, 0x1b20 );  // IOS80
        ...

This module reads those functions and resolves the inheritance, so the
generator works from WiiQt's data directly rather than a copy of it that
could drift.

Two things in that file are deliberately not stock, and are dropped here:

  - `GetUpdate()` adds the News and Weather channels to every update
    ("hell, give everybody these") and forces IOS4/IOS9 in. Only the
    `ListXXr()` tables are read, so those additions never appear.
  - The 4.2 lists carry cIOS slots 222, 223, 249 and 250, which are
    Waninkoko's, not Nintendo's. They are listed in CIOS_SLOTS below and
    dropped, and would fail the NUS check anyway since Nintendo never
    served them.

Two transcription errors are repaired, both in TYPO_FIXES:

  - The 4.3E/J/U lists insert title id 0x100000021 twice, the second time
    under a comment reading `IOS34`. 0x21 is IOS33, so IOS34 (0x100000022)
    is missing from those updates.
  - List22e() installs system menu 193, which is 2.2 USA — the same value
    List22u() uses, and its neighbouring entries carry USA comments against
    PAL title ids. 2.2 PAL is 194: wiibrew documents it, and every other
    version in the file numbers its menus JPN, USA, PAL consecutively.
"""
import re

# title version meaning "whatever NUS serves as newest", from WiiQt's
# TITLE_LATEST_VERSION. Used for the stub IOSes, whose version WiiQt did not
# pin; the generator resolves it against NUS.
LATEST = 0xff00

# cIOS slots WiiQt seeds into the 4.2 lists for homebrew users' convenience
CIOS_SLOTS = frozenset({0x1000000de, 0x1000000df, 0x1000000f9, 0x1000000fa})

# update label -> {title id: version} overrides correcting WiiQt's tables
TYPO_FIXES = {
    '2.2E': {0x100000002: 194},      # system menu 2.2 PAL, not 193 (2.2 USA)
    '4.3E': {0x100000022: 0xe18},    # IOS34, lost to a duplicate 0x100000021
    '4.3J': {0x100000022: 0xe18},
    '4.3U': {0x100000022: 0xe18},
}

REGIONS = {'e': 'EUR', 'u': 'USA', 'j': 'JPN', 'k': 'KOR'}


# --- vWii -------------------------------------------------------------------
#
# The Wii U carries a Wii NAND of its own in the SLCCMPT partition, holding a
# vWii build of the Wii system software. Nintendo delivers those titles under a
# separate NUS namespace — 00000007 for the essential titles, 00070002 and
# 00070008 for channels — but the TMDs and tickets inside carry the ordinary
# Wii title ids, so on SLCCMPT they land at the same paths a Wii uses. Keying
# this table by the NUS id and reading the installed path back out of the TMD
# therefore needs no special handling, and it keeps vWii's System Menu distinct
# from the Wii's: both are title 0000000100000002 and both have versions 512,
# 513 and 514, but the content differs.
#
# There is no wiiqt equivalent listing vWii update contents, so the table was
# built from wiiubrew's title database, with every id and version confirmed
# against NUS, and checked against a retail SLCCMPT dump.
#
# Most vWii titles have exactly one version on NUS and resolve through LATEST.
# Five do not — IOS59, IOS62 and BC-NAND, in VWII_MULTI below — and nothing
# records which of their versions ships with which vWii release. Only 5.2.0 is
# settled, because the dump is a 5.2.0E console and matched every entry of the
# derived manifest; those titles are therefore pinned for 5.2.0 and left out of
# 1.0.0 and 4.0.0 rather than guessed at. The Wii Menu Manual and WagonCompat
# Transfer Tool are left out entirely for the same reason (see VWII_CHANNELS).
VWII_SYSMENU = {
    '1.0.0J': 512, '1.0.0U': 513, '1.0.0E': 514,
    '4.0.0J': 544, '4.0.0U': 545, '4.0.0E': 546,
    '5.2.0J': 608, '5.2.0U': 609, '5.2.0E': 610,
}
VWII_SYSMENU_ID = 0x0000000700000002
VWII_IOS = [
    0x0000000700000009, 0x000000070000000c, 0x000000070000000d, 0x000000070000000e,
    0x000000070000000f, 0x0000000700000011, 0x0000000700000015, 0x0000000700000016,
    0x000000070000001c, 0x000000070000001f, 0x0000000700000021, 0x0000000700000022,
    0x0000000700000023, 0x0000000700000024, 0x0000000700000025, 0x0000000700000026,
    0x0000000700000029, 0x000000070000002b, 0x000000070000002d, 0x000000070000002e,
    0x0000000700000030, 0x0000000700000035, 0x0000000700000037, 0x0000000700000038,
    0x0000000700000039, 0x000000070000003a, 0x0000000700000050,
    0x0000000700000201,   # BC-WFS, the Wii U filesystem shim; only ever v1
]
# Titles NUS serves more than one version of, and the release each version is
# known to belong to. 5.2.0 is the only one a dump settles; the other versions
# listed in the comments exist on NUS but are unattributed.
VWII_MULTI = {
    0x000000070000003b: {'5.2.0': 9249},   # IOS59, also v7201 and v8737
    0x000000070000003e: {'5.2.0': 6942},   # IOS62, also v6430 and v6686
    0x0000000700000200: {'5.2.0': 7},      # BC-NAND, also v6
}
# {nus title id: region letter, or None for every region}. Only channels with a
# single version on NUS are listed. The Wii Menu Manual (HCUE/HCUJ/HCUP, five
# or six versions each) and the WagonCompat Transfer Tool (HCZE/HCZJ/HCZP, v29
# and v31) are left out: nothing records which of their versions belongs to
# which vWii release, and picking one would be a guess.
VWII_CHANNELS = {
    0x0007000248414241: None,   # HABA Wii Shop Channel, v21
    0x0007000248414341: None,   # HACA Mii Channel, v6
    0x0007000248435641: None,   # HCVA Wii U Menu, v0
    0x0007000848414c45: 'U',    # HALE rgnsel, v2
    0x0007000848414c4a: 'J',    # HALJ rgnsel, v2
    0x0007000848414c50: 'E',    # HALP rgnsel, v2
}


def vwii_updates():
    """Return {vWii version label: {NUS title id: version}}, same shape as parse()."""
    out = {}
    for label, menu_version in VWII_SYSMENU.items():
        region, release = label[-1], label[:-1]
        titles = {VWII_SYSMENU_ID: menu_version}
        titles.update({tid: LATEST for tid in VWII_IOS})
        titles.update({tid: LATEST for tid, r in VWII_CHANNELS.items()
                       if r is None or r == region})
        titles.update({tid: versions[release]
                       for tid, versions in VWII_MULTI.items()
                       if release in versions})
        out[label] = titles
    return out


def vwii_unattributed(label):
    """Titles left out of `label` because their version is not known for it."""
    return sorted(tid for tid, versions in VWII_MULTI.items()
                  if label[:-1] not in versions)

FUNC = re.compile(r'NusDownloader::List(\d)(\d)([eujk])\(\)')
BASE = re.compile(r'titles\s*=\s*List(\d)(\d)([eujk])\(\)')
INSERT = re.compile(r'^\s*titles\.insert\(\s*(0x[0-9a-fA-F]+)ull\s*,\s*'
                    r'(0x[0-9a-fA-F]+|\d+)\s*\)')


def _label(major, minor, region):
    return f'{major}.{minor}{region.upper()}'


def parse(path):
    """Return {update label: {title id: version}} from nusdownloader.cpp.

    Labels are of the form `4.3U` — the system menu version and its region
    letter, as WiiQt names them.
    """
    raw, current = {}, None
    with open(path, errors='replace') as f:
        for line in f:
            m = FUNC.search(line)
            if m:
                current = _label(*m.groups())
                raw[current] = {'base': None, 'inserts': {}}
                continue
            if current is None:
                continue
            if line.startswith('}'):
                current = None
                continue
            m = BASE.search(line)
            if m:
                raw[current]['base'] = _label(*m.groups())
                continue
            m = INSERT.match(line)
            if m:
                raw[current]['inserts'][int(m.group(1), 16)] = int(m.group(2), 0)

    resolved = {}

    def resolve(label, seen=()):
        if label in resolved:
            return resolved[label]
        if label in seen or label not in raw:
            return {}
        entry = raw[label]
        titles = dict(resolve(entry['base'], seen + (label,))) if entry['base'] else {}
        titles.update(entry['inserts'])
        titles.update(TYPO_FIXES.get(label, {}))
        for tid in CIOS_SLOTS:
            titles.pop(tid, None)
        resolved[label] = titles
        return titles

    return {label: resolve(label) for label in raw}


def label_key(label):
    """Sort key placing update labels in release order, region last.

    Covers both the Wii's two-part labels (4.3U) and vWii's three-part ones
    (5.2.0U).
    """
    m = re.match(r'^(\d+)\.(\d+)(?:\.(\d+))?([EUJK])$', label)
    if not m:
        return (1, 0, 0, 0, label)
    return (0, int(m.group(1)), int(m.group(2)),
            int(m.group(3) or 0), m.group(4))


def region_of(label):
    return REGIONS[label[-1].lower()]
