# Open Timegrapher brand

The logo, colours, type and tokens of the Open Timegrapher project, for
anyone who writes about it, builds on it or makes something that works
with it.

![Open Timegrapher](logo/lockup.svg)

- **[GUIDE.md](GUIDE.md)**: how the project looks outside the app:
  colours, type, the mark, layout and print.
- **[POLICY.md](POLICY.md)**: when you may use the name and logo.
- How the app itself looks is in the
  [design guide](../docs/developer/design/design-guide.md).

## What's here

| Folder | Files |
|---|---|
| `logo/` | `mark.svg` (light backgrounds), `mark-dark.svg` (dark), `mark-black.svg` and `mark-white.svg` (one colour); the same four as `mark-small-*.svg` for 16 to 32 px; `lockup*.svg` (mark and name); `app-icon.svg`, `app-icon-small.svg`; `favicon.svg` (follows the browser's light or dark mode) |
| `logo/png/` | the mark at 16 to 1024 px, light and dark; the app icon at 16 to 1024 px; one-colour marks at 512 px; lockups 80 and 160 px tall |
| `tokens/` | `tokens.css` (custom properties, light and dark) and `tokens.json` (the same values, in the W3C design tokens format) |
| `guide/` | the pictures in GUIDE.md |

The SVG files are the source. The `mark` and `lockup` files include the
clear space the guide asks for; the small mark, favicon and app icon are
cropped to the drawing. The PNG files are exported from them at
exactly those sizes. The name in the lockups is Inter SemiBold, turned into
outlines, so the files need no font.

## Fonts

The project uses two fonts, both under the SIL Open Font License, so they
can be bundled in the app and embedded in PDF reports:

- [Inter](https://github.com/rsms/inter) for everything. The app ships it in
  `crates/timegrapher-app/assets/fonts/`.
- [JetBrains Mono](https://github.com/JetBrains/JetBrainsMono), only for
  commands and code.

## Licence

Everything in this folder is under the
[Creative Commons Attribution 4.0 International](LICENSE) licence (CC BY
4.0). The code in the rest of the repository is GPL-3.0-or-later.

Credit it as "Open Timegrapher brand, CC BY 4.0", with a link to this
folder. CC BY 4.0 does not grant rights to the name or the logo as a mark:
[POLICY.md](POLICY.md) covers those.
