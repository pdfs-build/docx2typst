use anyhow::{Context, Result as AnyResult};
use docx2typst_emit::{single_file_font_dir, write_bundle};
pub use docx2typst_model::*;
use docx2typst_opc::OpcPackage;
use std::path::Path;

pub fn convert_path(
    path: impl AsRef<Path>,
    options: &ConversionOptions,
) -> AnyResult<ConversionResult> {
    let package = OpcPackage::open_path(path.as_ref()).context("failed to open DOCX package")?;
    convert_package(&package, options)
}

pub fn convert_bytes(bytes: Vec<u8>, options: &ConversionOptions) -> AnyResult<ConversionResult> {
    let package = OpcPackage::open_bytes(bytes).context("failed to open DOCX bytes")?;
    convert_package(&package, options)
}

pub fn convert_package(
    package: &OpcPackage,
    options: &ConversionOptions,
) -> AnyResult<ConversionResult> {
    let document = docx2typst_wml::parse_document(package, options.profile.as_ref())
        .context("failed to parse WordprocessingML")?;
    let mut result = docx2typst_emit::emit(&document, options);
    result.stats.parts = package.part_names().len();

    if let Some(output_path) = &options.output_path {
        persist_outputs(&result, output_path)?;
    }
    Ok(result)
}

pub fn validate_result(
    result: &ConversionResult,
    options: &ValidationOptions,
) -> AnyResult<ValidationReport> {
    Ok(docx2typst_validate::validate_result(result, options))
}

pub fn validate_path(
    path: impl AsRef<Path>,
    options: &ValidationOptions,
) -> AnyResult<ValidationReport> {
    let root = path.as_ref();
    let typst =
        std::fs::read_to_string(root.join("main.typ")).context("failed to read main.typ")?;
    let report_json = std::fs::read_to_string(root.join("report.json")).unwrap_or_default();
    let mut result = if report_json.is_empty() {
        ConversionResult {
            mode: OutputMode::Bundle,
            typst: typst.clone(),
            bundle_root: Some(root.to_path_buf()),
            assets: Vec::new(),
            embedded_fonts: Vec::new(),
            fallbacks: Vec::new(),
            diagnostics: Vec::new(),
            font_decisions: Vec::new(),
            font_search_paths: Vec::new(),
            fallback_inventory: Vec::new(),
            stats: ConversionStats {
                parts: 0,
                assets: 0,
                paragraphs: 0,
                tables: 0,
                sections: 0,
            },
        }
    } else {
        serde_json::from_str::<ConversionResult>(&report_json)
            .context("failed to parse report.json")?
    };
    if result.typst.is_empty() {
        result.typst = typst;
    }
    Ok(docx2typst_validate::validate_result(&result, options))
}

pub fn inspect_path(path: impl AsRef<Path>) -> AnyResult<InspectionReport> {
    let package = OpcPackage::open_path(path.as_ref()).context("failed to open DOCX package")?;
    Ok(docx2typst_wml::inspect_package(&package)?)
}

pub fn inspect_bytes(bytes: Vec<u8>) -> AnyResult<InspectionReport> {
    let package = OpcPackage::open_bytes(bytes).context("failed to open DOCX bytes")?;
    Ok(docx2typst_wml::inspect_package(&package)?)
}

pub fn explain_diagnostic(code: &str) -> &'static str {
    match code {
        "WML_UNSUPPORTED_BODY_CHILD" => {
            "An unsupported element was found directly under the Word document body."
        }
        "WML_UNSUPPORTED_INLINE_CONTAINER" => {
            "An inline WordprocessingML container could not be translated faithfully and may need a fallback."
        }
        "FONT_SUBSTITUTION" => {
            "A DOCX font request was replaced during conversion; inspect the diagnostic location for the original use site."
        }
        "DOCX_EMBEDDED_FONT_LICENSE" => {
            "A DOCX-embedded font was found, but its embedding rights do not allow safe extraction for conversion output."
        }
        "DOCX_EMBEDDED_FONT_MISSING_RELATIONSHIP" => {
            "A DOCX font table entry referenced an embedded font relationship that could not be resolved."
        }
        "DOCX_EMBEDDED_FONT_MISSING_PART" => {
            "A DOCX font table entry referenced an embedded font part that does not exist in the package."
        }
        "DOCX_EMBEDDED_FONT_DEOBFUSCATION_FAILED" => {
            "A DOCX embedded font was obfuscated but could not be decoded into a usable font file."
        }
        "DOCX_FALLBACK_RENDERED_ASSET" => {
            "An unsupported drawing or equation was preserved as a generated fallback asset rather than native Typst content."
        }
        "REFERENCE_PDF_LOAD_ERROR" => {
            "The reference PDF could not be opened for comparison during validation."
        }
        "REFERENCE_PDF_PARSE_ERROR" => {
            "The generated PDF could not be parsed for reference comparison."
        }
        "REFERENCE_PAGE_COUNT_MISMATCH" => {
            "The generated PDF page count does not match the reference PDF page count."
        }
        "REFERENCE_TEXT_EXTRACT_ERROR" => {
            "Text extraction failed for one of the PDFs used in reference-aware validation."
        }
        "REFERENCE_TEXT_BELOW_THRESHOLD" => {
            "The generated PDF text similarity fell below the configured reference threshold."
        }
        "TYPST_SYNTAX_ERROR" => "The generated Typst could not be parsed successfully.",
        "TYPST_WARNING" => "Typst compiled with warnings that may affect fidelity.",
        "TYPST_COMPILE_ERROR" => "Typst failed to compile the generated document.",
        "TYPST_PDF_ERROR" => "Typst compiled the document but failed during PDF export.",
        _ => "Unknown diagnostic code.",
    }
}

fn persist_outputs(result: &ConversionResult, output_path: &Path) -> AnyResult<()> {
    match result.mode {
        OutputMode::Bundle => {
            write_bundle(result, output_path).context("failed to write bundle output")?;
        }
        OutputMode::SingleFile => {
            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent).context("failed to create parent directory")?;
            }
            std::fs::write(output_path, &result.typst).context("failed to write Typst output")?;
            if !result.embedded_fonts.is_empty() {
                write_single_file_fonts(result, output_path)
                    .context("failed to write embedded fonts for single-file output")?;
            }
            let report_path = output_path.with_extension("report.json");
            std::fs::write(report_path, result.report_json()?)
                .context("failed to write report output")?;
        }
    }
    Ok(())
}

fn write_single_file_fonts(result: &ConversionResult, output_path: &Path) -> AnyResult<()> {
    let root = single_file_font_dir(output_path);
    std::fs::create_dir_all(&root).context("failed to create single-file font directory")?;
    for font in &result.embedded_fonts {
        let Some(path) = &font.emitted_path else {
            continue;
        };
        let relative = std::path::Path::new(path);
        let relative = relative.strip_prefix("fonts").unwrap_or(relative);
        let target = root.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        std::fs::write(&target, &font.bytes)
            .with_context(|| format!("failed to write {}", target.display()))?;
    }
    Ok(())
}
