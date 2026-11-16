# Fonts

Inter 4 by Rasmus Andersson and the Inter Project Authors, under the SIL Open
Font License 1.1 (OFL.txt). These copies are cut down to Latin, Latin-1,
Latin Extended-A and the symbols the app shows (° ± − µ × · … arrows), and
their digits and plus and minus signs are mapped to the tabular
forms, so readings keep their width as they change.

- Inter-Regular.ttf: body text
- Inter-SemiBold.ttf: headings, tab titles and buttons that need weight
- InterDisplay-SemiBold.ttf: the large readings

NotoSansCJK-Subset.ttf: the characters the calibre data quotes that Inter
lacks, mostly Chinese from the makers' drawings and manuals. Its glyphs come
from Noto Sans SC, with Ⅱ and Ⅲ from Noto Sans KR (the same design) and φ
from Noto Sans, all under the SIL Open Font License 1.1 (OFL-Noto.txt), taken
from the @fontsource npm packages. It sits behind Inter in every family, so
only those characters use it. cjk_subset.py rebuilds it; run it when the
test `every_character_in_the_calibre_data_has_a_glyph` names a new character.
