use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, ModelError>;

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("failed to serialize report: {0}")]
    SerializeReport(#[from] serde_json::Error),
    #[error("failed to parse profile: {0}")]
    ParseProfile(#[from] toml::de::Error),
    #[error("failed to serialize profile: {0}")]
    SerializeProfile(#[from] toml::ser::Error),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OutputMode {
    #[default]
    Bundle,
    SingleFile,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AssetMode {
    #[default]
    Auto,
    Extract,
    Inline,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum WarningPolicy {
    Ignore,
    #[default]
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FallbackPolicy {
    Omit,
    #[default]
    PreserveAsAsset,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Default)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    #[default]
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ValidationLevel {
    Syntax,
    #[default]
    Compile,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceLocation {
    pub part: String,
    pub path: String,
    pub range: Option<(usize, usize)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    #[serde(default)]
    pub location: Option<SourceLocation>,
    #[serde(default)]
    pub context: IndexMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FontDecision {
    pub requested: String,
    pub resolved: String,
    pub reason: String,
    #[serde(default)]
    pub location: Option<SourceLocation>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FontEmbeddingRights {
    #[default]
    Unknown,
    Installable,
    Restricted,
    PreviewPrint,
    Editable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct EmbeddedFontLicense {
    #[serde(default)]
    pub rights: FontEmbeddingRights,
    #[serde(default)]
    pub no_subsetting: bool,
    #[serde(default)]
    pub bitmap_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct EmbeddedFont {
    pub family: String,
    pub variant: String,
    pub original_path: String,
    pub media_type: String,
    #[serde(default)]
    pub emitted_path: Option<String>,
    #[serde(default)]
    pub subsetted: bool,
    #[serde(default)]
    pub obfuscated: bool,
    #[serde(default)]
    pub usable: bool,
    #[serde(default)]
    pub license: EmbeddedFontLicense,
    #[serde(default)]
    pub location: Option<SourceLocation>,
    #[serde(default)]
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssetReference {
    pub id: String,
    pub original_path: String,
    pub media_type: String,
    pub emitted_path: Option<String>,
    #[serde(default)]
    pub alt_text: Option<String>,
    #[serde(default)]
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConversionStats {
    pub parts: usize,
    pub assets: usize,
    pub paragraphs: usize,
    pub tables: usize,
    pub sections: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConversionResult {
    pub mode: OutputMode,
    pub typst: String,
    #[serde(default)]
    pub bundle_root: Option<PathBuf>,
    #[serde(default)]
    pub assets: Vec<AssetReference>,
    #[serde(default)]
    pub embedded_fonts: Vec<EmbeddedFont>,
    #[serde(default)]
    pub fallbacks: Vec<FallbackRecord>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
    #[serde(default)]
    pub font_decisions: Vec<FontDecision>,
    #[serde(default)]
    pub font_search_paths: Vec<PathBuf>,
    #[serde(default)]
    pub fallback_inventory: Vec<String>,
    pub stats: ConversionStats,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationReport {
    pub level: ValidationLevel,
    pub syntax_ok: bool,
    pub compile_ok: bool,
    #[serde(default)]
    pub page_count: Option<usize>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
    #[serde(default)]
    pub reference: Option<ReferenceValidation>,
    #[serde(default, skip_serializing)]
    pub pdf_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ReferenceValidation {
    pub reference_path: PathBuf,
    #[serde(default)]
    pub reference_page_count: Option<usize>,
    #[serde(default)]
    pub compared_page_count: Option<usize>,
    #[serde(default)]
    pub page_count_delta: Option<isize>,
    #[serde(default)]
    pub page_count_match: bool,
    #[serde(default)]
    pub text_similarity_percent: Option<u32>,
    #[serde(default)]
    pub threshold_percent: Option<u32>,
    #[serde(default)]
    pub within_threshold: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InspectionReport {
    pub package_parts: Vec<String>,
    pub relationships: IndexMap<String, Vec<String>>,
    pub styles: Vec<String>,
    pub fonts: Vec<String>,
    #[serde(default)]
    pub embedded_fonts: Vec<String>,
    pub theme_colors: IndexMap<String, String>,
    #[serde(default)]
    pub theme_fonts: IndexMap<String, String>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConversionOptions {
    #[serde(default)]
    pub profile: Option<ConversionProfile>,
    #[serde(default)]
    pub output_mode: OutputMode,
    #[serde(default)]
    pub asset_mode: AssetMode,
    #[serde(default)]
    pub warning_policy: WarningPolicy,
    #[serde(default)]
    pub fallback_policy: FallbackPolicy,
    #[serde(default)]
    pub validation: ValidationOptions,
    #[serde(default)]
    pub reference: Option<PathBuf>,
    #[serde(default)]
    pub output_path: Option<PathBuf>,
}

impl Default for ConversionOptions {
    fn default() -> Self {
        Self {
            profile: None,
            output_mode: OutputMode::Bundle,
            asset_mode: AssetMode::Auto,
            warning_policy: WarningPolicy::Warn,
            fallback_policy: FallbackPolicy::PreserveAsAsset,
            validation: ValidationOptions::default(),
            reference: None,
            output_path: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationOptions {
    #[serde(default)]
    pub level: ValidationLevel,
    #[serde(default = "default_true")]
    pub emit_pdf: bool,
    #[serde(default = "default_true")]
    pub allow_compile_fallback: bool,
    #[serde(default)]
    pub reference_path: Option<PathBuf>,
    #[serde(default)]
    pub reference_text_similarity_threshold_percent: Option<u32>,
}

impl Default for ValidationOptions {
    fn default() -> Self {
        Self {
            level: ValidationLevel::Compile,
            emit_pdf: true,
            allow_compile_fallback: true,
            reference_path: None,
            reference_text_similarity_threshold_percent: Some(85),
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StyleMapEntry {
    #[serde(default)]
    pub heading_level: Option<u8>,
    #[serde(default)]
    pub typst_function: Option<String>,
    #[serde(default)]
    pub block: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct FontProfile {
    #[serde(default)]
    pub substitutions: IndexMap<String, String>,
    #[serde(default)]
    pub embedded_paths: IndexMap<String, PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct FallbackProfile {
    #[serde(default)]
    pub drawing: Option<FallbackPolicy>,
    #[serde(default)]
    pub equation: Option<FallbackPolicy>,
    #[serde(default)]
    pub unsupported_feature: Option<FallbackPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ValidationProfile {
    #[serde(default)]
    pub level: Option<ValidationLevel>,
    #[serde(default)]
    pub reference_diff_threshold: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConversionProfile {
    pub version: u32,
    #[serde(default)]
    pub paragraph_styles: IndexMap<String, StyleMapEntry>,
    #[serde(default)]
    pub character_styles: IndexMap<String, StyleMapEntry>,
    #[serde(default)]
    pub fonts: FontProfile,
    #[serde(default)]
    pub theme_overrides: IndexMap<String, String>,
    #[serde(default)]
    pub fallback: FallbackProfile,
    #[serde(default)]
    pub warning_threshold: Option<Severity>,
    #[serde(default)]
    pub validation: ValidationProfile,
    #[serde(default)]
    pub deny_unsupported_features: Vec<String>,
}

impl Default for ConversionProfile {
    fn default() -> Self {
        Self {
            version: 1,
            paragraph_styles: IndexMap::new(),
            character_styles: IndexMap::new(),
            fonts: FontProfile::default(),
            theme_overrides: IndexMap::new(),
            fallback: FallbackProfile::default(),
            warning_threshold: Some(Severity::Warning),
            validation: ValidationProfile::default(),
            deny_unsupported_features: Vec::new(),
        }
    }
}

impl ConversionProfile {
    pub fn from_toml(input: &str) -> Result<Self> {
        Ok(toml::from_str(input)?)
    }

    pub fn to_toml(&self) -> Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Document {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub sections: Vec<Section>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
    #[serde(default)]
    pub assets: Vec<AssetReference>,
    #[serde(default)]
    pub notes: IndexMap<String, Note>,
    #[serde(default)]
    pub fonts_seen: Vec<String>,
    #[serde(default)]
    pub font_usage_locations: IndexMap<String, SourceLocation>,
    #[serde(default)]
    pub embedded_fonts: Vec<EmbeddedFont>,
    #[serde(default)]
    pub fallbacks: Vec<FallbackRecord>,
    #[serde(default)]
    pub theme_colors: IndexMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Section {
    pub id: String,
    #[serde(default)]
    pub blocks: Vec<Block>,
    #[serde(default)]
    pub page_setup: Option<PageSetup>,
    #[serde(default)]
    pub header: Option<HeaderFooter>,
    #[serde(default)]
    pub footer: Option<HeaderFooter>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeaderFooter {
    pub kind: HeaderFooterKind,
    #[serde(default)]
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HeaderFooterKind {
    Header,
    Footer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PageSetup {
    #[serde(default)]
    pub width_twips: Option<u32>,
    #[serde(default)]
    pub height_twips: Option<u32>,
    #[serde(default)]
    pub margin_top_twips: Option<u32>,
    #[serde(default)]
    pub margin_right_twips: Option<u32>,
    #[serde(default)]
    pub margin_bottom_twips: Option<u32>,
    #[serde(default)]
    pub margin_left_twips: Option<u32>,
    #[serde(default)]
    pub header_twips: Option<u32>,
    #[serde(default)]
    pub footer_twips: Option<u32>,
    #[serde(default)]
    pub orientation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ResolvedStyle {
    #[serde(default)]
    pub style_id: Option<String>,
    #[serde(default)]
    pub style_name: Option<String>,
    #[serde(default)]
    pub font_family: Option<String>,
    #[serde(default)]
    pub font_size_half_points: Option<u32>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub background_color: Option<String>,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub underline: bool,
    #[serde(default)]
    pub strike: bool,
    #[serde(default)]
    pub subscript: bool,
    #[serde(default)]
    pub superscript: bool,
    #[serde(default)]
    pub alignment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    Paragraph(Paragraph),
    Heading(Heading),
    Table(Table),
    Image(ImageBlock),
    List(ListBlock),
    Fallback(FallbackBlock),
    Unsupported(UnsupportedBlock),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Paragraph {
    pub id: String,
    #[serde(default)]
    pub style: ResolvedStyle,
    #[serde(default)]
    pub inlines: Vec<Inline>,
    #[serde(default)]
    pub list: Option<ListMetadata>,
    #[serde(default)]
    pub spacing_before_twips: Option<u32>,
    #[serde(default)]
    pub spacing_after_twips: Option<u32>,
    #[serde(default)]
    pub line_spacing_twips: Option<u32>,
    #[serde(default)]
    pub page_break_before: bool,
    #[serde(default)]
    pub keep_next: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Heading {
    pub id: String,
    pub level: u8,
    #[serde(default)]
    pub style: ResolvedStyle,
    #[serde(default)]
    pub inlines: Vec<Inline>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ListMetadata {
    #[serde(default)]
    pub list_id: Option<String>,
    pub kind: ListKind,
    pub level: u8,
    #[serde(default)]
    pub numbering: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ListKind {
    #[default]
    Bullet,
    Numbered,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ListBlock {
    pub id: String,
    pub kind: ListKind,
    #[serde(default)]
    pub numbering: Option<String>,
    #[serde(default)]
    pub items: Vec<ListItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ListItem {
    #[serde(default)]
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct EdgeInsetsTwips {
    #[serde(default)]
    pub top: Option<u32>,
    #[serde(default)]
    pub right: Option<u32>,
    #[serde(default)]
    pub bottom: Option<u32>,
    #[serde(default)]
    pub left: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TableBorderLine {
    #[serde(default)]
    pub width_eighth_points: Option<u32>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub style: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TableCellBorders {
    #[serde(default)]
    pub top: Option<TableBorderLine>,
    #[serde(default)]
    pub right: Option<TableBorderLine>,
    #[serde(default)]
    pub bottom: Option<TableBorderLine>,
    #[serde(default)]
    pub left: Option<TableBorderLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TableBorders {
    #[serde(default)]
    pub top: Option<TableBorderLine>,
    #[serde(default)]
    pub right: Option<TableBorderLine>,
    #[serde(default)]
    pub bottom: Option<TableBorderLine>,
    #[serde(default)]
    pub left: Option<TableBorderLine>,
    #[serde(default)]
    pub inside_horizontal: Option<TableBorderLine>,
    #[serde(default)]
    pub inside_vertical: Option<TableBorderLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Table {
    pub id: String,
    #[serde(default)]
    pub width_twips: Option<u32>,
    #[serde(default)]
    pub width_pct_fiftieths: Option<u32>,
    #[serde(default)]
    pub indent_twips: Option<i32>,
    #[serde(default)]
    pub alignment: Option<String>,
    #[serde(default)]
    pub layout_type: Option<String>,
    #[serde(default)]
    pub style_id: Option<String>,
    #[serde(default)]
    pub cell_spacing_twips: Option<u32>,
    #[serde(default)]
    pub cell_margins: Option<EdgeInsetsTwips>,
    #[serde(default)]
    pub direct_borders: TableBorders,
    #[serde(default)]
    pub borders: TableBorders,
    #[serde(default)]
    pub columns: Vec<TableColumn>,
    #[serde(default)]
    pub rows: Vec<TableRow>,
    #[serde(default)]
    pub style: ResolvedStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TableColumn {
    #[serde(default)]
    pub width_twips: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TableRow {
    #[serde(default)]
    pub height_twips: Option<u32>,
    #[serde(default)]
    pub height_rule: Option<String>,
    #[serde(default)]
    pub is_header: bool,
    #[serde(default)]
    pub cells: Vec<TableCell>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TableCell {
    #[serde(default)]
    pub blocks: Vec<Block>,
    #[serde(default)]
    pub column_index: usize,
    #[serde(default)]
    pub width_twips: Option<u32>,
    #[serde(default)]
    pub width_pct_fiftieths: Option<u32>,
    #[serde(default)]
    pub margins_twips: Option<EdgeInsetsTwips>,
    #[serde(default)]
    pub background_color: Option<String>,
    #[serde(default)]
    pub direct_borders: TableCellBorders,
    #[serde(default)]
    pub borders: TableCellBorders,
    #[serde(default)]
    pub vertical_align: Option<String>,
    #[serde(default)]
    pub col_span: usize,
    #[serde(default)]
    pub row_span: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ImageBlock {
    pub id: String,
    pub asset_id: String,
    #[serde(default)]
    pub alt_text: Option<String>,
    #[serde(default)]
    pub width_emu: Option<u64>,
    #[serde(default)]
    pub height_emu: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FallbackKind {
    #[default]
    Drawing,
    Equation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct FallbackRecord {
    pub id: String,
    pub kind: FallbackKind,
    pub feature: String,
    pub asset_id: String,
    #[serde(default)]
    pub alt_text: Option<String>,
    #[serde(default)]
    pub width_emu: Option<u64>,
    #[serde(default)]
    pub height_emu: Option<u64>,
    #[serde(default)]
    pub raw_xml: Option<String>,
    #[serde(default)]
    pub location: Option<SourceLocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct FallbackBlock {
    pub record_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct FallbackInline {
    pub record_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct UnsupportedBlock {
    pub id: String,
    pub feature: String,
    #[serde(default)]
    pub raw_xml: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Inline {
    Text(TextRun),
    PageNumber(ResolvedStyle),
    LineBreak,
    PageBreak,
    Hyperlink(Hyperlink),
    NoteReference(NoteReference),
    Bookmark(Bookmark),
    Image(ImageInline),
    Fallback(FallbackInline),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TextRun {
    pub text: String,
    #[serde(default)]
    pub style: ResolvedStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Hyperlink {
    pub target: String,
    #[serde(default)]
    pub inlines: Vec<Inline>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct NoteReference {
    pub note_id: String,
    pub kind: NoteKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum NoteKind {
    #[default]
    Footnote,
    Endnote,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Bookmark {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ImageInline {
    pub asset_id: String,
    #[serde(default)]
    pub alt_text: Option<String>,
    #[serde(default)]
    pub width_emu: Option<u64>,
    #[serde(default)]
    pub height_emu: Option<u64>,
    #[serde(default)]
    pub offset_x_emu: Option<i64>,
    #[serde(default)]
    pub offset_y_emu: Option<i64>,
    #[serde(default)]
    pub floating: bool,
    #[serde(default)]
    pub behind_doc: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Note {
    pub id: String,
    pub kind: NoteKind,
    #[serde(default)]
    pub blocks: Vec<Block>,
}

impl ConversionResult {
    pub fn report_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

impl ValidationReport {
    pub fn report_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

impl InspectionReport {
    pub fn report_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}
