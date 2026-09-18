use docx2typst_emit::{RUNTIME_FILE_NAME, runtime_source};
use docx2typst_model::{
    ConversionResult, Diagnostic, EmbeddedFont, ReferenceValidation, Severity, ValidationLevel,
    ValidationOptions, ValidationReport,
};
use indexmap::IndexMap;
use lopdf::Document as PdfDocument;
use std::collections::BTreeMap;
use std::path::PathBuf;
use typst::layout::PagedDocument;
use typst_as_lib::{TypstAsLibError, TypstEngine, typst_kit_options::TypstKitFontOptions};
use typst_syntax::{SyntaxKind, parse};

pub fn validate_result(result: &ConversionResult, options: &ValidationOptions) -> ValidationReport {
    let mut diagnostics = Vec::new();
    let syntax_tree = parse(&result.typst);
    let syntax_ok = !contains_error_kind(&syntax_tree);
    if !syntax_ok {
        diagnostics.push(diagnostic(
            "TYPST_SYNTAX_ERROR",
            Severity::Error,
            "generated Typst contains syntax errors".to_string(),
        ));
    }

    let mut compile_ok = false;
    let mut pdf_bytes = Vec::new();
    let mut page_count = None;
    let mut reference = None;
    let temp_font_dir = materialize_embedded_fonts(result).ok().flatten();
    let needs_pdf = options.emit_pdf || options.reference_path.is_some();

    if syntax_ok && matches!(options.level, ValidationLevel::Compile) {
        let mut font_dirs = result.font_search_paths.clone();
        if let Some(dir) = temp_font_dir.as_ref().map(|dir| dir.path.clone()) {
            if !font_dirs.iter().any(|existing| existing == &dir) {
                font_dirs.push(dir);
            }
        }
        let mut builder = TypstEngine::builder()
            .search_fonts_with(
                TypstKitFontOptions::default()
                    .include_system_fonts(true)
                    .include_dirs(font_dirs)
                    .include_embedded_fonts(true),
            )
            .with_static_source_file_resolver([
                ("main.typ", result.typst.as_str()),
                (RUNTIME_FILE_NAME, runtime_source()),
            ]);

        if matches!(result.mode, docx2typst_model::OutputMode::Bundle) {
            let binaries = result.assets.iter().filter_map(|asset| {
                asset
                    .emitted_path
                    .as_deref()
                    .map(|path| (path, asset.bytes.as_slice()))
            });
            builder = builder.with_static_file_resolver(binaries);
        }

        let engine = builder.build();
        let warned = engine.compile::<_, PagedDocument>("main.typ");
        for warning in warned.warnings {
            diagnostics.push(diagnostic(
                "TYPST_WARNING",
                Severity::Warning,
                warning.message.to_string(),
            ));
        }

        match warned.output {
            Ok(document) => {
                compile_ok = true;
                page_count = Some(document.pages.len());
                if needs_pdf {
                    match typst_pdf::pdf(&document, &Default::default()) {
                        Ok(bytes) => pdf_bytes = bytes,
                        Err(errors) => {
                            for error in errors {
                                diagnostics.push(diagnostic(
                                    "TYPST_PDF_ERROR",
                                    Severity::Error,
                                    error.message.to_string(),
                                ));
                            }
                        }
                    }
                }
            }
            Err(error) => push_typst_compile_diagnostics(&mut diagnostics, error),
        }
    }

    if compile_ok
        && !pdf_bytes.is_empty()
        && let Some(reference_path) = options.reference_path.as_ref()
    {
        reference = compare_against_reference_pdf(
            &pdf_bytes,
            reference_path,
            page_count,
            options.reference_text_similarity_threshold_percent,
            &mut diagnostics,
        );
    }

    ValidationReport {
        level: options.level,
        syntax_ok,
        compile_ok,
        page_count,
        diagnostics,
        reference,
        pdf_bytes,
    }
}

fn compare_against_reference_pdf(
    generated_pdf: &[u8],
    reference_path: &PathBuf,
    generated_page_count: Option<usize>,
    threshold_percent: Option<u32>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ReferenceValidation> {
    let generated_doc = match PdfDocument::load_mem(generated_pdf) {
        Ok(document) => document,
        Err(error) => {
            diagnostics.push(diagnostic(
                "REFERENCE_PDF_PARSE_ERROR",
                Severity::Error,
                format!("failed to parse generated PDF for reference comparison: {error}"),
            ));
            return None;
        }
    };
    let reference_doc = match PdfDocument::load(reference_path) {
        Ok(document) => document,
        Err(error) => {
            diagnostics.push(diagnostic(
                "REFERENCE_PDF_LOAD_ERROR",
                Severity::Error,
                format!(
                    "failed to open reference PDF `{}`: {error}",
                    reference_path.display()
                ),
            ));
            return None;
        }
    };

    let reference_page_count = Some(reference_doc.get_pages().len());
    let compared_page_count =
        generated_page_count.or_else(|| Some(generated_doc.get_pages().len()));
    let page_count_delta = compared_page_count
        .zip(reference_page_count)
        .map(|(generated, reference)| generated as isize - reference as isize);
    let page_count_match = page_count_delta == Some(0);
    if matches!(page_count_delta, Some(delta) if delta != 0) {
        let mut context = IndexMap::new();
        context.insert(
            "generated_pages".to_string(),
            compared_page_count.unwrap_or_default().to_string(),
        );
        context.insert(
            "reference_pages".to_string(),
            reference_page_count.unwrap_or_default().to_string(),
        );
        diagnostics.push(diagnostic_with_context(
            "REFERENCE_PAGE_COUNT_MISMATCH",
            Severity::Warning,
            format!(
                "generated PDF page count ({}) does not match reference PDF page count ({})",
                compared_page_count.unwrap_or_default(),
                reference_page_count.unwrap_or_default()
            ),
            context,
        ));
    }

    let generated_text = match extract_pdf_text_from_bytes(generated_pdf) {
        Ok(text) => text,
        Err(error) => {
            diagnostics.push(diagnostic(
                "REFERENCE_TEXT_EXTRACT_ERROR",
                Severity::Warning,
                format!("failed to extract text from generated PDF: {error}"),
            ));
            String::new()
        }
    };
    let reference_text = match extract_pdf_text_from_path(reference_path) {
        Ok(text) => text,
        Err(error) => {
            diagnostics.push(diagnostic(
                "REFERENCE_TEXT_EXTRACT_ERROR",
                Severity::Warning,
                format!(
                    "failed to extract text from reference PDF `{}`: {error}",
                    reference_path.display()
                ),
            ));
            String::new()
        }
    };

    let text_similarity_percent = if generated_text.is_empty() && reference_text.is_empty() {
        Some(100)
    } else if generated_text.is_empty() || reference_text.is_empty() {
        Some(0)
    } else {
        Some(multiset_dice_similarity_percent(
            &normalize_pdf_text(&generated_text),
            &normalize_pdf_text(&reference_text),
        ))
    };
    let within_threshold = threshold_percent
        .zip(text_similarity_percent)
        .map(|(threshold, similarity)| similarity >= threshold);
    if let Some(false) = within_threshold {
        let mut context = IndexMap::new();
        context.insert(
            "similarity_percent".to_string(),
            text_similarity_percent.unwrap_or_default().to_string(),
        );
        context.insert(
            "threshold_percent".to_string(),
            threshold_percent.unwrap_or_default().to_string(),
        );
        diagnostics.push(diagnostic_with_context(
            "REFERENCE_TEXT_BELOW_THRESHOLD",
            Severity::Warning,
            format!(
                "generated PDF text similarity ({}) is below the configured threshold ({})",
                text_similarity_percent.unwrap_or_default(),
                threshold_percent.unwrap_or_default()
            ),
            context,
        ));
    }

    Some(ReferenceValidation {
        reference_path: reference_path.clone(),
        reference_page_count,
        compared_page_count,
        page_count_delta,
        page_count_match,
        text_similarity_percent,
        threshold_percent,
        within_threshold: within_threshold.map(|within| within && page_count_match),
    })
}

fn materialize_embedded_fonts(result: &ConversionResult) -> std::io::Result<Option<TempFontDir>> {
    let fonts = result
        .embedded_fonts
        .iter()
        .filter(|font| font.usable && !font.bytes.is_empty())
        .collect::<Vec<_>>();
    if fonts.is_empty() {
        return Ok(None);
    }

    let dir = TempFontDir::new()?;
    for font in fonts {
        write_embedded_font(&dir.path, font)?;
    }
    Ok(Some(dir))
}

fn write_embedded_font(root: &PathBuf, font: &EmbeddedFont) -> std::io::Result<()> {
    let relative = font
        .emitted_path
        .as_deref()
        .and_then(|path| {
            let path = std::path::Path::new(path);
            path.file_name().map(PathBuf::from)
        })
        .unwrap_or_else(|| PathBuf::from(format!("{}-{}.ttf", font.family, font.variant)));
    let target = root.join(relative);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(target, &font.bytes)
}

struct TempFontDir {
    path: PathBuf,
}

impl TempFontDir {
    fn new() -> std::io::Result<Self> {
        let unique = format!(
            "docx2typst-fonts-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&path)?;
        Ok(Self { path })
    }
}

impl Drop for TempFontDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn contains_error_kind(root: &typst_syntax::SyntaxNode) -> bool {
    if root.kind() == SyntaxKind::Error {
        return true;
    }
    for child in root.children() {
        if contains_error_kind(child) {
            return true;
        }
    }
    false
}

fn diagnostic(code: &str, severity: Severity, message: String) -> Diagnostic {
    Diagnostic {
        code: code.to_string(),
        severity,
        message,
        location: None,
        context: IndexMap::new(),
    }
}

fn diagnostic_with_context(
    code: &str,
    severity: Severity,
    message: String,
    context: IndexMap<String, String>,
) -> Diagnostic {
    Diagnostic {
        code: code.to_string(),
        severity,
        message,
        location: None,
        context,
    }
}

fn extract_pdf_text_from_bytes(bytes: &[u8]) -> Result<String, pdf_extract::OutputError> {
    pdf_extract::extract_text_from_mem(bytes)
}

fn extract_pdf_text_from_path(path: &PathBuf) -> Result<String, pdf_extract::OutputError> {
    pdf_extract::extract_text(path)
}

fn normalize_pdf_text(text: &str) -> String {
    text.chars()
        .map(|ch| {
            if ch.is_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
}

fn multiset_dice_similarity_percent(left: &str, right: &str) -> u32 {
    let left_tokens = token_multiset(left);
    let right_tokens = token_multiset(right);
    let left_total = left_tokens.values().sum::<usize>();
    let right_total = right_tokens.values().sum::<usize>();
    if left_total == 0 && right_total == 0 {
        return 100;
    }
    if left_total == 0 || right_total == 0 {
        return 0;
    }
    let intersection = left_tokens
        .iter()
        .map(|(token, count)| {
            right_tokens
                .get(token)
                .copied()
                .unwrap_or_default()
                .min(*count)
        })
        .sum::<usize>();
    ((200.0 * intersection as f32) / (left_total + right_total) as f32).round() as u32
}

fn token_multiset(text: &str) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for token in text.split_whitespace().filter(|token| !token.is_empty()) {
        *counts.entry(token.to_string()).or_insert(0) += 1;
    }
    counts
}

fn push_typst_compile_diagnostics(diagnostics: &mut Vec<Diagnostic>, error: TypstAsLibError) {
    match error {
        TypstAsLibError::TypstSource(errors) => {
            for error in errors {
                diagnostics.push(diagnostic(
                    "TYPST_COMPILE_ERROR",
                    Severity::Error,
                    error.message.to_string(),
                ));
            }
        }
        other => diagnostics.push(diagnostic(
            "TYPST_COMPILE_ERROR",
            Severity::Error,
            other.to_string(),
        )),
    }
}
