# ALSA PCM field guide

An independent, local-first reader for the [ALSA PCM documentation](https://www.alsa-project.org/alsa-doc/alsa-lib/pcm.html). No changes to the Rust application, no frontend framework, no build step, and no runtime dependencies.

## Open it

From the repository root:

```sh
python3 -m http.server 8765 --bind 127.0.0.1 --directory docs/alsa
```

Visit **http://localhost:8765**. You can also open `index.html` directly; clipboard support depends on browser permissions and secure-context rules. Deploy the directory unchanged to any static host. Hash routes need no server rewrites.

## What's included

- A guided start page, a complete C playback tutorial, frame/buffer concepts, error recovery, and a “how to use these docs” page.
- The full PCM overview divided into 16 chapters.
- 325 documented API members from the PCM interface and its 16 direct topic pages, including signatures, parameters, returns, and enumerations.
- Full-text search with `/` or Ctrl/⌘ K; arrow keys + Enter select results, Escape closes search.
- A symbol browser with topic filtering, deep links, in-page contents, upstream attribution, code copying, light/dark themes, and mobile navigation.
- Bundled waveform images and example source. No analytics, external fonts, CDN assets, or remote search.

This is **not all of ALSA**. Unimported structure definitions, source pages, upstream examples, and deeper topic pages remain links to alsa-project.org. Imported text remains in upstream wording, including historical material; the new learning guides are explicitly marked editorial. The snapshot date is displayed in the reader and is not an alsa-lib release number.

## Files

| File | Purpose |
| --- | --- |
| `index.html`, `style.css`, `app.js` | Static reader, navigation, search, and presentation |
| `guides.js` | Newly written learning material |
| `data.js` | Generated upstream snapshot and bundled C example |
| `examples/playback.c` | Downloadable, complete playback program |
| `assets/` | Local branding and upstream waveform images |
| `scripts/import_docs.py` | Repeatable import from the upstream site |
| `scripts/check.mjs` | Dependency-free content/link validation |

## Refresh the upstream snapshot

Python 3.10+ and Beautiful Soup are needed **only for import**, not for serving the site:

```sh
python3 -m venv /tmp/alsa-docs-venv
/tmp/alsa-docs-venv/bin/pip install 'beautifulsoup4>=4.12,<5'
/tmp/alsa-docs-venv/bin/python docs/alsa/scripts/import_docs.py
node docs/alsa/scripts/check.mjs
```

The importer fetches the overview plus its 17 API topic pages, extracts documented members, removes active markup, maps included cross-references to local routes, and leaves other links pointing upstream. It preserves each entry's original source URL and bundles the current `playback.c`. Review the generated diff after refreshing: upstream structure and wording can change. Run the importer after changing the C example to keep the displayed and downloadable versions identical.

## Validate

```sh
node --check docs/alsa/app.js
node --check docs/alsa/guides.js
node docs/alsa/scripts/check.mjs
cc -std=c11 -Wall -Wextra -Werror docs/alsa/examples/playback.c \
  -o /tmp/alsa-guide-playback $(pkg-config --cflags --libs alsa) -lm
/tmp/alsa-guide-playback null
```

The `null` PCM test produces **no sound**; hardware playback is a separate, manual test. The example otherwise plays a quiet two-second tone on `default`; lower the output volume first.

Browser checks performed: desktop and 390px mobile layouts; all 346 content routes render without page-width overflow on mobile; full-text search and keyboard selection; symbol filters and empty results; chapter anchors; mobile menu; theme switching; no browser console errors. The static validator checks 785 internal content links, fragment targets, local assets, unique routes, and example consistency.

## Attribution and licensing

Original reference prose and waveform images are from the **ALSA project and its contributors**, retrieved from alsa-project.org. Each imported entry links to its source. The reference is reformatted rather than presented as newly authored prose. New editorial material lives separately in `guides.js`; the reader is not endorsed by ALSA.

See the [alsa-lib source repository](https://github.com/alsa-project/alsa-lib) for original sources and per-file notices. `UPSTREAM-COPYING` is a copy of that repository's LGPL 2.1 license text, downloaded from `https://raw.githubusercontent.com/alsa-project/alsa-lib/master/COPYING`; it does not replace per-file notices or relicense third-party content.
