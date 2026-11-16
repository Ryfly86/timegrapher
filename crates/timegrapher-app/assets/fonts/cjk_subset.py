"""Cut NotoSansCJK-Subset.ttf down to the characters the calibre data uses
that Inter lacks.

Usage: python3 -I cjk_subset.py <fontsource dir> <repo>

<fontsource dir> holds the unpacked npm packages @fontsource/noto-sans-sc,
@fontsource/noto-sans-kr and @fontsource/noto-sans, as
<dir>/noto-sans-sc/package and so on. Noto Sans SC carries the Chinese;
Noto Sans KR (the same design) only Ⅱ and Ⅲ, which the SC files lack; Noto
Sans only φ. Needs fontTools. Run it again when the theme test
`every_character_in_the_calibre_data_has_a_glyph` fails.
"""
import glob
import sys

from fontTools import subset
from fontTools.merge import Merger
from fontTools.ttLib import TTFont

src, repo = sys.argv[1], sys.argv[2]
fonts = f"{repo}/crates/timegrapher-app/assets/fonts"
inter = set(TTFont(f"{fonts}/Inter-Regular.ttf").getBestCmap())
need = set()
for p in glob.glob(f"{repo}/crates/timegrapher-core/data/*.json"):
    need |= {ord(c) for c in open(p, encoding="utf-8").read() if ord(c) > 127 and ord(c) not in inter}

ONLY = {"noto-sans-kr": {0x2161, 0x2162}, "noto-sans": {0x3C6}}
parts, got = [], set()
for pkg in ("noto-sans-sc", "noto-sans-kr", "noto-sans"):
    for f in sorted(glob.glob(f"{src}/{pkg}/package/files/{pkg}-*-400-normal.woff")):
        hit = (need & set(TTFont(f).getBestCmap()) & ONLY.get(pkg, need)) - got
        if not hit:
            continue
        o = subset.Options()
        o.flavor = None
        o.layout_features = []
        o.drop_tables += ["BASE", "STAT", "GDEF", "GPOS", "GSUB", "vhea", "vmtx"]
        o.name_IDs = ["*"]
        o.notdef_outline = True
        font = TTFont(f)
        s = subset.Subsetter(o)
        s.populate(unicodes=hit)
        s.subset(font)
        font.flavor = None
        part = f"{src}/part{len(parts)}.ttf"
        font.save(part)
        parts.append(part)
        got |= hit
if need - got:
    sys.exit("no glyph for " + "".join(chr(c) for c in sorted(need - got)))
Merger().merge(parts).save(f"{fonts}/NotoSansCJK-Subset.ttf")
print(len(got), "characters from", len(parts), "files")
