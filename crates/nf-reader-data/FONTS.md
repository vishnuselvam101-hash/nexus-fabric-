# Fonts used by nf-reader-data (generator v2)

Only fonts installed from the Debian packages below are used. No proprietary or downloaded fonts. Nothing is embedded
in the repository; the renderer calls the host fonts by family name.

| Family (CSS name) | Used for | Package on this host | Licence |
|---|---|---|---|
| DejaVu Sans Mono | Latin text, numbers, title block | fonts-dejavu-mono (DejaVuSansMono.ttf) | Bitstream Vera Fonts licence, with DejaVu changes in the public domain (fonts-dejavu-core copyright file) |
| Noto Sans CJK JP | Japanese text (kana, kanji, full-width) | fonts-noto-cjk (NotoSansCJK-Regular.ttc) | SIL Open Font License 1.1 (fonts-noto-cjk copyright file) |

Hand-written-looking notes (degradation `hand`) use DejaVu Sans Mono in italic with a rotation of -4 to +4 degrees.
No handwriting font is installed, and none was added.

`render.rs` sets `FONT = "DejaVu Sans Mono, Noto Sans CJK JP"`. Glyph advances come from the same two faces, which is
why the printed text boxes are exact without rendering.

Before any binary is redistributed, check the upstream licence texts for the exact version in use.
