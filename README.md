# docx2typst

Rust-first DOCX to Typst conversion workspace with a CLI, core library, validation layer, and Node bindings.

The current implementation delivers an end-to-end vertical slice:

- Open DOCX files as OPC packages
- Parse key WordprocessingML parts into a typed intermediate model
- Emit Typst in bundle or single-file mode
- Extract assets and generate machine-readable conversion reports
- Validate emitted Typst with syntax checks and optional PDF compilation
- Expose the same workflow through a CLI and a Node-facing native module

The architecture is intentionally staged so richer DOCX coverage can be added without rewriting the public API.

It powers `.docx` import on [pdfs.build](https://pdfs.build).

See [docs/plan.md](docs/plan.md) for the current feature matrix and roadmap.

## Requirements

- Rust toolchain with Cargo (edition 2024, `rust-version = "1.89"`, tested on 1.94)
- Node.js and npm if you want to build or use the native Node bindings

## Install

This crate is not yet published to crates.io. Until it is, install from source:

```bash
git clone https://github.com/pdfs-build/docx2typst
cd docx2typst
cargo install --path crates/docx2typst-cli
```

That puts a `docx2typst-cli` binary on your `$PATH`. To use the library from another Rust project without a crates.io release, add a git dependency:

```toml
[dependencies]
docx2typst = { git = "https://github.com/pdfs-build/docx2typst" }
```

## Setup

From the repository root:

```bash
cargo check
cargo test
```

That builds and tests the Rust workspace, including the CLI, library crates, and the native Node crate. All 23 tests pass on a clean checkout.

## Build

Build the full Rust workspace:

```bash
cargo build
```

Build only the CLI:

```bash
cargo build -p docx2typst-cli
```

Build the native Node module:

```bash
npm install --prefix crates/docx2typst-node
npm run build --prefix crates/docx2typst-node
```

This produces the native `index.node` binary next to the JS wrapper in [`crates/docx2typst-node`](crates/docx2typst-node).

## Public Corpus

The repository includes a pinned public DOCX corpus manifest at [`tests/corpus/public/manifest.toml`](tests/corpus/public/manifest.toml). It pulls real fixtures from open-source upstream projects under permissive licenses and is meant for broader reliability checks than the synthetic Rust smoke fixtures.

Fetch the fixtures:

```bash
python3 scripts/fetch_public_corpus.py
```

Run the corpus through the CLI and write a summary:

```bash
python3 scripts/run_public_corpus.py --out .artifacts/public-corpus
```

Selection rationale, sources, and baseline semantics are documented in [docs/corpus.md](docs/corpus.md).

## CLI

Run the CLI directly from the workspace:

```bash
cargo run -p docx2typst-cli -- --help
```

### Convert

Bundle output is the default mode.

```bash
cargo run -p docx2typst-cli -- convert ./document.docx
```

That writes a `docx2typst-out/` directory containing:

- `main.typ`
- `docx2typst-runtime.typ`
- `assets/`
- `report.json`
- `validation.json`

Write the bundle somewhere else:

```bash
cargo run -p docx2typst-cli -- convert ./document.docx --out ./out/report-bundle
```

Emit single-file Typst to stdout:

```bash
cargo run -p docx2typst-cli -- convert ./document.docx --mode single-file
```

Use a conversion profile:

```bash
cargo run -p docx2typst-cli -- convert ./document.docx --profile ./profile.toml
```

Write a compiled PDF during conversion:

```bash
cargo run -p docx2typst-cli -- convert ./document.docx --pdf-out ./out/document.pdf
```

`--pdf-out` does not force single-file mode. It uses the selected conversion mode for Typst output and then writes a PDF from the validated in-memory result:

- `bundle` mode keeps `main.typ`, the runtime file, and extracted assets on disk. This is the recommended mode for debugging or editing generated Typst.
- `single-file` mode still emits one `.typ` file if you pass `--mode single-file --out ./document.typ`, but the PDF is written separately to the `--pdf-out` path.

Skip validation during conversion:

```bash
cargo run -p docx2typst-cli -- convert ./document.docx --no-validate
```

`--no-validate` cannot be combined with `--pdf-out`, because PDF generation depends on a successful Typst compile step.

Compare the generated output against an existing reference PDF during validation:

```bash
cargo run -p docx2typst-cli -- convert ./document.docx \
  --out ./out/report-bundle \
  --reference ./reference/document.pdf \
  --reference-threshold 90
```

`--reference` compares against an existing PDF path and adds a `reference` block to `validation.json` with page-count and normalized text-similarity results. If you also pass `--pdf-out`, the reference PDF must already exist before the command starts; the newly written `--pdf-out` file is produced after validation finishes.

### Inspect

Inspect the DOCX package and parsed metadata:

```bash
cargo run -p docx2typst-cli -- inspect ./document.docx
```

This prints JSON with package parts, relationships, styles, fonts, and theme colors.

### Validate

Validate a previously generated bundle:

```bash
cargo run -p docx2typst-cli -- validate ./docx2typst-out
```

Run syntax-only validation:

```bash
cargo run -p docx2typst-cli -- validate ./docx2typst-out --syntax-only
```

Validate against a reference PDF:

```bash
cargo run -p docx2typst-cli -- validate ./docx2typst-out \
  --reference ./reference/document.pdf \
  --reference-threshold 90
```

The current reference check compares compiled page count and normalized extracted text. It is useful for catching pagination and content drift, but it is not a visual diff.

### Explain

Explain a diagnostic code:

```bash
cargo run -p docx2typst-cli -- explain TYPST_COMPILE_ERROR
```

## Library Usage (Rust)

Add `docx2typst` as a dependency (see [Install](#install) for the git-dependency line, since it is not on crates.io yet), then call the top-level `convert_path` / `convert_bytes` functions:

```rust
use docx2typst::{convert_path, ConversionOptions};

fn main() -> anyhow::Result<()> {
    let options = ConversionOptions::default();
    let result = convert_path("document.docx", &options)?;

    println!("{}", result.typst);
    println!("{:?}", result.stats);
    Ok(())
}
```

`ConversionOptions::default()` produces bundle-mode output in memory (nothing is written to disk unless `options.output_path` is set). `ConversionResult` carries the emitted Typst source, extracted assets, diagnostics, and stats. To validate the result against a real Typst compile:

```rust
use docx2typst::{convert_path, validate_result, ConversionOptions, ValidationOptions};

fn main() -> anyhow::Result<()> {
    let result = convert_path("document.docx", &ConversionOptions::default())?;
    let report = validate_result(&result, &ValidationOptions::default())?;

    assert!(report.compile_ok);
    Ok(())
}
```

Both snippets were run against the fixtures in [`tests/corpus/public/fixtures`](tests/corpus/public/fixtures) while writing this README.

## Node Usage

Build the native module first:

```bash
npm install --prefix crates/docx2typst-node
npm run build --prefix crates/docx2typst-node
```

Then require the local package by path.

```js
const docx2typst = require("./crates/docx2typst-node");
```

### Convert a file

```js
const docx2typst = require("./crates/docx2typst-node");

async function main() {
  const result = await docx2typst.convertFile("./document.docx", {
    output_mode: "bundle",
    output_path: "./node-out"
  });

  console.log(result);
}

main().catch(err => {
  console.error(err);
  process.exit(1);
});
```

The Node package accepts the same snake_case option names as the Rust `ConversionOptions` and `ValidationOptions` models, including:

- `output_mode`, `asset_mode`, `warning_policy`, `fallback_policy`
- nested `validation` options like `level`, `emit_pdf`, `reference_path`, and `reference_text_similarity_threshold_percent`
- top-level `reference` and `profile`

When conversion is asked to write output or when non-default validation/reference options are supplied, `convertFile()` and `convertBuffer()` return:

```js
{
  conversion: { /* ConversionResult */ },
  validation: { /* ValidationReport */ }
}
```

Otherwise they return the plain `ConversionResult`.

### Convert a buffer

```js
const fs = require("node:fs/promises");
const docx2typst = require("./crates/docx2typst-node");

async function main() {
  const buffer = await fs.readFile("./document.docx");
  const result = await docx2typst.convertBuffer(buffer, {
    output_mode: "single_file"
  });

  console.log(result.typst);
}

main().catch(err => {
  console.error(err);
  process.exit(1);
});
```

### Inspect and validate

```js
const docx2typst = require("./crates/docx2typst-node");

async function main() {
  const inspection = await docx2typst.inspectFile("./document.docx");
  console.log(inspection);

  const validation = await docx2typst.validateOutput("./docx2typst-out", {
    level: "compile",
    emit_pdf: false,
    reference_path: "./reference/document.pdf",
    reference_text_similarity_threshold_percent: 90
  });
  console.log(validation);
}

main().catch(err => {
  console.error(err);
  process.exit(1);
});
```

The published TypeScript declarations in [`crates/docx2typst-node/index.d.ts`](crates/docx2typst-node/index.d.ts) now describe the full Node option surface instead of `unknown`.

### Explain a diagnostic

```js
const docx2typst = require("./crates/docx2typst-node");

console.log(docx2typst.explain("WML_UNSUPPORTED_BODY_CHILD"));
```

## Profiles

Profiles are TOML files deserialized into the shared `ConversionProfile` type. The current implementation supports:

- paragraph style mappings
- character style mappings
- font substitutions
- theme overrides
- fallback policy preferences
- validation settings
- unsupported feature deny lists

Profiles can also set a default reference threshold:

```toml
[validation]
reference_diff_threshold = 90
```

Minimal example:

```toml
version = 1
warning_threshold = "warning"

[fonts.substitutions]
"Calibri" = "Libertinus Serif"

[paragraph_styles.Heading1]
heading_level = 1
```

## Current Scope

What exists now is a usable first slice, not complete Word fidelity. The current codebase already covers package parsing, basic style extraction, body parsing, Typst emission, validation, a CLI, and Node bindings. More advanced Word features and higher-fidelity layout handling are tracked in [docs/plan.md](docs/plan.md).

## Limitations

- No tracked changes / revision markup support yet. Documents with tracked changes convert, but the changes are not modeled.
- No `settings.xml`, comments, or fields/TOC semantics parsing yet.
- Text boxes are not first-class content yet.
- Charts, SmartArt, equations (OMML), and text boxes render as fallback placeholder assets (generated SVG), not source-faithful reproductions of the original Word object.
- Reference-PDF validation compares page count and normalized extracted text, not a visual/pixel diff.
- Layout fidelity is good for straightforward documents (headings, paragraphs, tables, lists, headers/footers, images, hyperlinks) but is not a pixel-perfect Word renderer for complex layouts.

See [docs/plan.md](docs/plan.md) for the full in-progress feature matrix.

## License

MIT. See [LICENSE](LICENSE).
