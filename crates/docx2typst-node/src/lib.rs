use docx2typst::{
    ConversionOptions, ConversionResult, ValidationOptions, convert_bytes, convert_path,
    explain_diagnostic, inspect_bytes, inspect_path, validate_path, validate_result,
};
use docx2typst_emit;
use napi::bindgen_prelude::{Buffer, Error, Result};
use napi_derive::napi;

#[napi]
pub fn convert_file(path: String, options_json: Option<String>) -> Result<String> {
    let options = parse_options(options_json)?;
    let result = convert_path(path, &options).map_err(to_napi_error)?;
    serialize_conversion_response(result, &options)
}

#[napi]
pub fn convert_buffer(buffer: Buffer, options_json: Option<String>) -> Result<String> {
    let options = parse_options(options_json)?;
    let result = convert_bytes(buffer.to_vec(), &options).map_err(to_napi_error)?;
    serialize_conversion_response(result, &options)
}

#[napi]
pub fn validate_output(path: String, options_json: Option<String>) -> Result<String> {
    let options = parse_validation_options(options_json)?;
    let result = validate_path(path, &options).map_err(to_napi_error)?;
    serde_json::to_string(&result).map_err(to_napi_error)
}

#[napi]
pub fn inspect_file(path: String) -> Result<String> {
    let result = inspect_path(path).map_err(to_napi_error)?;
    serde_json::to_string(&result).map_err(to_napi_error)
}

#[napi]
pub fn inspect_buffer(buffer: Buffer) -> Result<String> {
    let result = inspect_bytes(buffer.to_vec()).map_err(to_napi_error)?;
    serde_json::to_string(&result).map_err(to_napi_error)
}

#[napi]
pub fn explain(code: String) -> String {
    explain_diagnostic(&code).to_string()
}

#[napi]
pub fn runtime_source() -> String {
    docx2typst_emit::runtime_source().to_string()
}

fn parse_options(options_json: Option<String>) -> Result<ConversionOptions> {
    match options_json {
        Some(raw) => serde_json::from_str(&raw).map_err(to_napi_error),
        None => Ok(ConversionOptions::default()),
    }
}

fn parse_validation_options(options_json: Option<String>) -> Result<ValidationOptions> {
    match options_json {
        Some(raw) => serde_json::from_str(&raw).map_err(to_napi_error),
        None => Ok(ValidationOptions::default()),
    }
}

fn serialize_conversion_response(
    result: ConversionResult,
    options: &ConversionOptions,
) -> Result<String> {
    if should_include_validation(options) {
        let validation_options = validation_options_for_conversion(options);
        let validation = validate_result(&result, &validation_options).map_err(to_napi_error)?;
        let payload = serde_json::json!({
            "conversion": result,
            "validation": validation,
        });
        serde_json::to_string(&payload).map_err(to_napi_error)
    } else {
        serde_json::to_string(&result).map_err(to_napi_error)
    }
}

fn should_include_validation(options: &ConversionOptions) -> bool {
    options.output_path.is_some()
        || options.reference.is_some()
        || options.validation != ValidationOptions::default()
}

fn validation_options_for_conversion(options: &ConversionOptions) -> ValidationOptions {
    let mut validation = options.validation.clone();
    if validation.reference_path.is_none() {
        validation.reference_path = options.reference.clone();
    }
    if validation
        .reference_text_similarity_threshold_percent
        .is_none()
    {
        validation.reference_text_similarity_threshold_percent = options
            .profile
            .as_ref()
            .and_then(|profile| profile.validation.reference_diff_threshold)
            .or(ValidationOptions::default().reference_text_similarity_threshold_percent);
    }
    validation
}

fn to_napi_error(error: impl std::fmt::Display) -> Error {
    Error::from_reason(error.to_string())
}
