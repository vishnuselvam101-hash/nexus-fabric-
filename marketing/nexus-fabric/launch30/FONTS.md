# Fonts used by the launch film

All faces are loaded from local files in `./node_modules` (installed with `npm install` in this folder, versions pinned in `package.json`). Nothing is fetched from the network at render time. `film.html` waits for `document.fonts` before it sets `window.READY`.

| Use | Family | npm package | Version | Licence |
|---|---|---|---|---|
| All Latin text: headlines, labels, wordmark, end card, equation (italic), Latin inside Japanese | Inter (variable, optical-size and weight axes) | `@fontsource-variable/inter` | 5.3.0 | SIL Open Font License 1.1 |
| Japanese text | Noto Sans JP (400, 500) | `@fontsource/noto-sans-jp` | 5.3.1 | SIL Open Font License 1.1 |
| Data: dimensions, layer names, read-outs | JetBrains Mono (400) | `@fontsource/jetbrains-mono` | 5.3.0 | SIL Open Font License 1.1 |

CSS family order is `"Inter Variable", "Noto Sans JP"`, so Latin characters in the Japanese version are set in Inter, the same face as the English version and the wordmark.

The OFL permits use in video and bundling with the project. The licence text ships inside each package (`LICENSE`).
