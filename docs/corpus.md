# Public Corpus

This repository now includes a pinned public DOCX corpus under [`tests/corpus/public`](../tests/corpus/public).

## Why This Exists

The synthetic Rust fixtures in [`crates/docx2typst/tests/smoke.rs`](../crates/docx2typst/tests/smoke.rs) are good for targeted regressions, but they are not enough for product reliability. The public corpus adds real upstream DOCX files from open-source projects that already exercise WordprocessingML edge cases in the wild.

The initial corpus is intentionally:

- reproducible: every fixture is pinned to an exact upstream commit
- attributable: each fixture carries repo, license, and upstream path metadata
- curated: the selection maps to docx2typst roadmap areas instead of importing entire third-party test suites

## Source Research

The initial selection came from these upstream projects:

- [Open XML SDK](https://github.com/dotnet/Open-XML-SDK): MIT, large Microsoft-maintained DOCX asset library. Researched as a strong source, but not used for the first import because its fixture set is very broad and granular.
- [Open-Xml-PowerTools](https://github.com/OfficeDev/Open-Xml-PowerTools): MIT, rich `NewDocxDocuments` fixture set spanning headings, lists, tables, notes, images, fields, themes, charts, equations, SmartArt, text boxes, and revision tracking.
- [python-docx](https://github.com/python-openxml/python-docx): MIT, focused feature fixtures for headers/footers, character styles, sections, and table styles.
- [docxjs](https://github.com/VolodymyrBaydalka/docxjs): Apache-2.0, rendering-oriented DOCX fixtures for page layout, line spacing, merged tables, and header/footer behavior.
- [docx4j](https://github.com/plutext/docx4j): Apache-2.0, researched as a follow-up source for future expansion, especially around headers/footers, multilingual content, and VML/text box edge cases.

## Current Coverage

The manifest at [`tests/corpus/public/manifest.toml`](../tests/corpus/public/manifest.toml) currently covers:

- headings and paragraph styles
- hyperlinks
- inline images
- tables, table styles, merged cells, and hierarchical numbered lists
- sections and page setup
- footnotes and endnotes
- fields and page-number style content
- theme and font declarations
- content controls
- headers/footers, including odd/even variants
- line spacing and page layout
- unsupported-object fallback scenarios: charts, equations, SmartArt, and text boxes
- exploratory tracked-revision coverage

## Baselines

Each fixture declares one of three baselines:

- `compile`: the current converter is expected to compile the output successfully
- `fallback`: the current converter is expected to compile successfully and record at least one rendered fallback asset
- `exploratory`: the fixture is present for visibility and future hardening, but it does not currently gate success

## Commands

Fetch the pinned fixtures:

```bash
python3 scripts/fetch_public_corpus.py
```

Run the corpus through the CLI and write a summary:

```bash
python3 scripts/run_public_corpus.py --out .artifacts/public-corpus
```

Run only a subset:

```bash
python3 scripts/run_public_corpus.py \
  --ids powertools-table python-docx-table-style docxjs-table-spans
```

The runner writes per-fixture bundle output plus `summary.json` into the chosen output directory.

## How To Extend It

When adding more fixtures:

1. Prefer sources with clear reuse licenses and stable raw URLs.
2. Pin to a commit, not a moving branch, in the manifest.
3. Add tags that map directly to roadmap areas or known gaps.
4. Default new uncertain documents to `exploratory` until their behavior is understood.
5. Keep the public corpus complementary to synthetic fixtures; use synthetic fixtures for exact edge-case isolation and public fixtures for broader reliability coverage.
