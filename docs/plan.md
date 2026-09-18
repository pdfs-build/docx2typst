# docx2typst Plan

This file tracks what is already implemented and what should be picked up in future sessions.

## Foundation

- [x] Cargo workspace with separate crates for model, OPC, WML, emitter, validation, CLI, top-level API, and Node bindings
- [x] Shared model types for document IR, conversion options, validation reports, inspection reports, diagnostics, assets, and profiles
- [x] README with setup and usage documentation
- [ ] CI workflow for workspace build, test, and Node native build matrix
- [ ] Published crates and packaged Node distribution

## OPC / DOCX Package Layer

- [x] Open DOCX from path or bytes
- [x] Read ZIP parts and normalize part names
- [x] Parse `[Content_Types].xml`
- [x] Parse package and part relationships
- [x] Resolve relationship targets relative to source parts
- [x] Compute per-part SHA-256 checksums
- [ ] Stream large packages instead of holding all parts in memory
- [ ] Preserve source span mappings with richer XML path/index metadata
- [ ] Fuzz/property tests for malformed OPC containers

## WordprocessingML Parsing

- [x] Parse `word/document.xml`
- [x] Parse `word/styles.xml`
- [x] Parse `word/fontTable.xml`
- [x] Parse `word/numbering.xml`
- [x] Parse `word/footnotes.xml`
- [x] Parse `word/endnotes.xml` when present
- [x] Parse theme colors from `word/theme/theme1.xml` when present
- [x] Parse section page setup
- [x] Parse header/footer references and content
- [x] Parse paragraphs, headings, hyperlinks, line breaks, bookmarks, notes, and tables
- [x] Parse DrawingML and VML image references
- [x] Extract media assets
- [x] Record unsupported inline/body structures as diagnostics
- [x] Treat `auto`/`pct` OOXML table widths as non-fixed widths instead of literal zero-twip widths
- [ ] Parse `settings.xml`
- [ ] Parse comments
- [ ] Parse tracked changes
- [ ] Parse fields and TOC semantics
- [ ] Parse text boxes as first-class content
- [x] Parse charts, SmartArt, and other drawing objects into dedicated fallback records
- [ ] Parse OMML equations into a dedicated math IR
- [ ] Preserve unknown XML fragments for future fallback rendering

## Style Resolution

- [x] Read document defaults
- [x] Apply style inheritance through `basedOn`
- [x] Merge paragraph and run properties into resolved styles
- [x] Resolve direct font family, size, emphasis, color, highlight, vertical align, and alignment
- [x] Track fonts seen in the source document
- [x] Preserve paragraph spacing and page-break hints through style/default resolution
- [x] Respect explicit run-level "off" formatting like `w:b w:val="0"` when overriding paragraph run defaults
- [ ] Fully implement the intended precedence order:
  document defaults -> theme -> table/paragraph/character styles -> numbering/section context -> direct formatting
- [x] Resolve character styles separately from paragraph styles
- [x] Resolve direct table cell borders, shading, spacing, and vertical alignment
- [x] Resolve common table style inheritance for borders and cell margins
- [x] Preserve table width modes needed for layout fidelity, including absolute vs percent widths
- [x] Resolve theme font mappings
- [x] Record font decisions in conversion output and carry profile font search paths into validation
- [x] Resolve embedded fonts and licensing constraints from DOCX packages
- [x] Emit explicit font substitution diagnostics with source locations

## Intermediate Representation

- [x] Typed document IR with sections, blocks, inlines, notes, assets, and page setup
- [x] Source-location-aware diagnostics
- [x] Asset inventory carried through conversion results
- [ ] Dedicated IR nodes for equations, charts, SmartArt, text boxes, and positioned objects
- [x] Fallback metadata rich enough to distinguish native Typst vs rendered asset fallback
- [ ] Stable IDs across more object classes for regression testing

## Typst Emission

- [x] Emit companion runtime file
- [x] Emit bundle output with `main.typ`, runtime file, assets, and `report.json`
- [x] Emit single-file Typst mode
- [x] Emit headings, paragraphs, hyperlinks, notes, tables, page setup, header/footer hooks, and images
- [x] Emit machine-readable conversion reports
- [x] Reserve safe body margins for floating header/footer artwork emitted in page backgrounds
- [x] Emit native list blocks inside tables and other nested block containers
- [x] Emit native table spans, column widths, header rows, and cell shading
- [x] Emit native table borders, cell padding, and vertical alignment from direct formatting
- [x] Emit explicit table width/alignment/indent wrappers and exact fixed row heights
- [x] Infer table column widths from cell widths when `tblGrid` is absent
- [x] Emit table gutter from OOXML cell spacing and support percent-based table widths
- [x] Apply safe row-height locking for explicit exact rows using parsed Word row heights
- [x] Emit nested lists correctly instead of only grouping consecutive paragraphs by level/kind
- [x] Emit common table style-derived borders and cell margins
- [x] Close remaining table layout fidelity gaps beyond common Word table styles
- [ ] Emit paragraph line spacing/leading inside non-empty table cells
- [ ] Clamp and negotiate table widths against available page content width for mixed `dxa`/`pct`/`auto` inputs
- [ ] Extend table style conditional formatting beyond borders to fills, alignment, emphasis, and padding
- [ ] Model additional row/page behavior such as `atLeast`, `cantSplit`, and safer page-breaking around large rows
- [ ] Harden nested cell content with real nested-list handling and mixed paragraph/list flows
- [ ] Expand the synthetic table corpus to cover oversized tables, mixed unit widths, nested tables, percent cell widths, and malformed span cases
- [ ] Emit exact header/footer variants for first/even/default pages
- [ ] Emit equations as native Typst math when possible
- [x] Emit rendered fallback assets for unsupported drawings and equations
- [ ] Improve escaping and inline formatting fidelity for edge cases
- [ ] Reduce runtime helper surface and harden generated Typst structure

## Validation

- [x] Parse generated Typst with `typst-syntax`
- [x] Compile generated Typst through Typst-as-lib
- [x] Export PDF during validation when enabled
- [x] Surface Typst warnings and compile failures as diagnostics
- [x] Report compiled page count in validation output
- [x] Reference-aware validation against an external/rendered source PDF
- [ ] Structured fidelity scoring and diff thresholds
- [ ] Validation of output bundle completeness and asset references
- [ ] Large-document performance and memory budget checks

## CLI

- [x] `convert`
- [x] `inspect`
- [x] `validate`
- [x] `explain`
- [x] `--profile`
- [x] `--mode bundle|single-file`
- [x] `--out`
- [x] `--pdf-out`
- [x] `--no-validate`
- [x] `--syntax-only`
- [x] CLI support for richer validation/reference options
- [ ] CLI support for explicit asset mode and fallback policy flags
- [ ] CLI support for font search paths and profile discovery

## Rust Library API

- [x] Convert from file path
- [x] Convert from bytes
- [x] Inspect from file path
- [x] Inspect from bytes
- [x] Validate an in-memory conversion result
- [x] Validate a previously written bundle path
- [x] Diagnostic explanation helper
- [ ] Stable semver-ready public API review
- [ ] Better separation between high-level API and lower-level parser/emitter internals

## Node Bindings

- [x] Native N-API crate using `napi-rs`
- [x] JS wrapper with async helpers
- [x] File and buffer conversion APIs
- [x] Inspect and validate APIs
- [x] Diagnostic explanation API
- [ ] Published npm package
- [ ] Prebuilt binaries for common platforms
- [ ] Typed JS result interfaces beyond `unknown`
- [ ] End-to-end Node tests

## Tests

- [x] OPC unit test for a minimal package
- [x] End-to-end smoke test converting a synthetic DOCX and validating Typst output
- [x] Synthetic regression fixture for floating header/footer artwork, merged cells, list-in-cell content, mixed bold/plain runs, split hyperlinks, and page-count validation
- [x] Synthetic regression fixture for nested numbered/bulleted lists and common numbering styles
- [x] Synthetic regression fixture for embedded font extraction/licensing and source-located font substitution diagnostics
- [x] Synthetic regression fixture for unsupported drawing/equation fallback assets
- [x] Synthetic regression fixture for reference-PDF matching and mismatch diagnostics
- [x] Initial pinned public corpus manifest plus fetch/run scripts for real upstream DOCX fixtures
- [x] Manual acceptance on local letterhead and template documents
- [ ] Golden corpus for headings, styles, numbering, tables, images, headers/footers, notes, and page setup
- [ ] Regression fixtures for template/profile-specific documents
- [ ] Negative tests for malformed ZIP, broken relationships, corrupted XML, and missing media
- [ ] Snapshot tests for emitted Typst and `report.json`
- [ ] Performance tests for large multi-section documents

## Recommended Next Work

- [ ] Expand fixture coverage with real DOCX samples
- [ ] Add structured fidelity scoring and diff thresholds beyond page-count and text similarity
- [ ] Promote fallback placeholders into source-faithful renderers where possible
- [ ] Finish the remaining font precedence work
  system-font vs profile-substitution precedence, `settings.xml` font embedding flags, and better missing-font reporting

## Recent Findings

- [ ] Current reference validation is page-count plus normalized extracted-text similarity; it does not perform a visual/PDF raster diff yet
- [ ] Reference-PDF load failures currently surface as diagnostics but do not make the CLI exit non-zero; decide whether missing/invalid references should hard-fail validation commands
- [ ] The initial public corpus run shows `powertools-textbox` compiling without an explicit fallback record; textbox coverage needs to preserve fallback visibility instead of silently passing the baseline
- [ ] Preserve list continuation paragraphs that omit `w:numPr` by using indentation/style context instead of treating only explicit list paragraphs as item content
- [ ] Model numbered-list restart semantics and custom `lvlText` markers (`lvlOverride`, `startOverride`, non-default bullet glyphs) beyond the common decimal/alpha/roman mappings
- [ ] Aggregate or deduplicate repeated Typst validation warnings such as missing-font messages so large documents stay readable in `validation.json`
- [ ] Distinguish system-font retention from profile substitution at resolution time instead of assuming every configured substitution should win before validation
- [ ] Current drawing/equation fallback assets are generated SVG placeholders, not source-faithful renders of the original Word objects
