# Contributing

Thanks for looking at docx2typst.

## Setup

```bash
git clone https://github.com/pdfs-build/docx2typst
cd docx2typst
cargo test
```

That builds and tests the full workspace (CLI, core library, and the Node native crate). No external services or credentials are needed.

## Making changes

- Keep the crate split intact: `docx2typst-model` (types), `docx2typst-opc` (package/zip), `docx2typst-wml` (WordprocessingML parsing), `docx2typst-emit` (Typst emission), `docx2typst-validate` (compile/reference checks), `docx2typst` (top-level API), `docx2typst-cli` (CLI), `docx2typst-node` (Node bindings). Put logic in the crate that owns it rather than reaching across layers.
- Add or extend a test alongside any behavior change. The end-to-end smoke tests in [`crates/docx2typst/tests/smoke.rs`](crates/docx2typst/tests/smoke.rs) are the easiest place to add a synthetic DOCX regression fixture.
- If you touch DOCX interoperability, consider running the public corpus (`python3 scripts/run_public_corpus.py`) to see the effect on real-world fixtures pulled from open-source projects. See [`docs/corpus.md`](docs/corpus.md).
- Run `cargo fmt` and `cargo clippy --workspace` before opening a PR.

## Reporting issues

Open a GitHub issue with the DOCX feature or structure involved. If you can, attach a minimal, synthetic `.docx` that reproduces the problem (not a real document with personal or confidential content) or point to a fixture from an open-source project.

## Pull requests

Small, focused PRs are easier to review. Describe what changed and why, and note which tests you ran (`cargo test`, the public corpus script, or both).
