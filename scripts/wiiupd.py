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
    """Sort key placing update labels in release order, region last."""
    m = re.match(r'^(\d+)\.(\d+)([EUJK])', label)
    if not m:
        return (1, 0, 0, label)
    return (0, int(m.group(1)), int(m.group(2)), m.group(3))


def region_of(label):
    return REGIONS[label[-1].lower()]
