use docx2typst_model::{
    AssetReference, Block, ConversionOptions, ConversionResult, ConversionStats, Diagnostic,
    Document, EdgeInsetsTwips, EmbeddedFont, FallbackKind, FallbackRecord, FontDecision,
    HeaderFooter, HeaderFooterKind, Hyperlink, Inline, ListBlock, ListItem, NoteKind, OutputMode,
    PageSetup, Paragraph, ResolvedStyle, Section, Severity, SourceLocation, Table, TableBorderLine,
    TableCell, TableCellBorders, TableRow, TextRun,
};
use indexmap::IndexMap;
use std::cell::RefCell;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, std::io::Error>;

pub const RUNTIME_FILE_NAME: &str = "docx2typst-runtime.typ";

#[derive(Clone, Copy)]
enum BreakBehavior {
    Page,
    Column,
    Ignore,
}

struct RenderContext<'a> {
    options: &'a ConversionOptions,
    font_decisions: RefCell<Vec<FontDecision>>,
    diagnostics: RefCell<Vec<Diagnostic>>,
    font_usage_locations: IndexMap<String, SourceLocation>,
    embedded_fonts: Vec<EmbeddedFont>,
}

impl<'a> RenderContext<'a> {
    fn new(options: &'a ConversionOptions, document: &Document) -> Self {
        Self {
            options,
            font_decisions: RefCell::new(Vec::new()),
            diagnostics: RefCell::new(Vec::new()),
            font_usage_locations: document.font_usage_locations.clone(),
            embedded_fonts: document.embedded_fonts.clone(),
        }
    }

    fn into_font_decisions(self) -> Vec<FontDecision> {
        self.font_decisions.into_inner()
    }

    fn take_diagnostics(&self) -> Vec<Diagnostic> {
        self.diagnostics.borrow().clone()
    }
}

pub fn emit(document: &Document, options: &ConversionOptions) -> ConversionResult {
    let assets = prepare_assets(document.assets.clone(), options.output_mode);
    let embedded_fonts = prepare_embedded_fonts(document.embedded_fonts.clone());
    let mut render_document = document.clone();
    render_document.assets = assets.clone();
    render_document.embedded_fonts = embedded_fonts.clone();

    let render_context = RenderContext::new(options, &render_document);
    let default_font = render_document
        .fonts_seen
        .first()
        .cloned()
        .unwrap_or_else(|| "Libertinus Serif".to_string());
    let resolved_default_font = resolve_font(&render_context, &default_font);

    let mut typst = String::new();
    typst.push_str(&format!("#import \"{RUNTIME_FILE_NAME}\": *\n"));
    typst.push_str(&format!(
        "#set text(font: \"{}\")\n",
        escape_typst_string(&resolved_default_font)
    ));

    if let Some(first_section) = render_document.sections.first() {
        emit_page_setup(&mut typst, first_section, &render_document, &render_context);
    }
    typst.push('\n');

    for section in &render_document.sections {
        emit_section(&mut typst, section, &render_document, &render_context);
    }

    let fallback_inventory = render_document
        .fallbacks
        .iter()
        .map(|fallback| {
            format!(
                "{}:{}",
                match fallback.kind {
                    FallbackKind::Drawing => "drawing",
                    FallbackKind::Equation => "equation",
                },
                fallback.feature
            )
        })
        .collect();
    let mut diagnostics = render_document.diagnostics.clone();
    diagnostics.extend(render_context.take_diagnostics());

    ConversionResult {
        mode: options.output_mode,
        typst,
        bundle_root: options.output_path.clone(),
        assets,
        embedded_fonts,
        fallbacks: render_document.fallbacks.clone(),
        diagnostics,
        font_decisions: render_context.into_font_decisions(),
        font_search_paths: collect_font_search_paths(options, &render_document.embedded_fonts),
        fallback_inventory,
        stats: ConversionStats {
            parts: 0,
            assets: render_document.assets.len(),
            paragraphs: count_paragraphs(&render_document),
            tables: count_tables(&render_document),
            sections: render_document.sections.len(),
        },
    }
}

pub fn runtime_source() -> &'static str {
    r#"#let docx-header(content) = context content
#let docx-footer(content) = context content

#let docx-fallback(kind, source, format: auto, alt: none, width: auto, height: auto) = {
  image(source, format: format, alt: alt, width: width, height: height)
}

#let docx-inline-image(source, format: auto, alt: none, width: auto, height: auto) = {
  box(image(source, format: format, alt: alt, width: width, height: height))
}
"#
}

pub fn write_bundle(result: &ConversionResult, out_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(out_dir)?;
    std::fs::create_dir_all(out_dir.join("assets"))?;
    std::fs::write(out_dir.join(RUNTIME_FILE_NAME), runtime_source())?;
    std::fs::write(out_dir.join("main.typ"), &result.typst)?;
    for asset in &result.assets {
        if let Some(path) = &asset.emitted_path {
            let target = out_dir.join(path);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(target, &asset.bytes)?;
        }
    }
    for font in &result.embedded_fonts {
        if let Some(path) = &font.emitted_path {
            let target = out_dir.join(path);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(target, &font.bytes)?;
        }
    }
    std::fs::write(
        out_dir.join("report.json"),
        result.report_json().unwrap_or_default(),
    )?;
    Ok(())
}

fn emit_page_setup(
    out: &mut String,
    section: &Section,
    document: &Document,
    context: &RenderContext<'_>,
) {
    if let Some(page_setup) = &section.page_setup {
        let width = page_setup.width_twips.map(twips_to_typst);
        let height = page_setup.height_twips.map(twips_to_typst);
        let safe_top = compute_safe_top_margin(section, page_setup);
        let safe_bottom = compute_safe_bottom_margin(section, page_setup);
        let top = Some(inches_to_typst(safe_top));
        let right = page_setup.margin_right_twips.map(twips_to_typst);
        let bottom = Some(inches_to_typst(safe_bottom));
        let left = page_setup.margin_left_twips.map(twips_to_typst);

        let _ = write!(out, "#set page(");
        if let Some(width) = width {
            let _ = write!(out, "width: {width}, ");
        }
        if let Some(height) = height {
            let _ = write!(out, "height: {height}, ");
        }
        if top.is_some() || right.is_some() || bottom.is_some() || left.is_some() {
            let _ = write!(
                out,
                "margin: (top: {}, right: {}, bottom: {}, left: {}), ",
                top.unwrap_or_else(|| "1in".to_string()),
                right.unwrap_or_else(|| "1in".to_string()),
                bottom.unwrap_or_else(|| "1in".to_string()),
                left.unwrap_or_else(|| "1in".to_string())
            );
        }
        if let Some(background) = render_page_layer(section, document, context, true) {
            let _ = write!(out, "background: [{background}], ");
        }
        if let Some(foreground) = render_page_layer(section, document, context, false) {
            let _ = write!(out, "foreground: [{foreground}], ");
        }
        if let Some(header) = &section.header {
            let rendered = render_header_footer(header, document, context);
            if !rendered.is_empty() {
                let _ = write!(out, "header: [{}], ", rendered);
            }
        }
        if let Some(footer) = &section.footer {
            let rendered = render_header_footer(footer, document, context);
            if !rendered.is_empty() {
                let _ = write!(out, "footer: [{}], ", rendered);
            }
        }
        if let Some(header_distance) = page_setup.header_twips {
            let _ = write!(out, "header-ascent: {}, ", twips_to_typst(header_distance));
        }
        if let Some(footer_distance) = page_setup.footer_twips {
            let _ = write!(out, "footer-descent: {}, ", twips_to_typst(footer_distance));
        }
        out.push_str(")\n");
    }
}

fn emit_section(
    out: &mut String,
    section: &Section,
    document: &Document,
    context: &RenderContext<'_>,
) {
    let rendered = render_block_sequence(&section.blocks, document, context, BreakBehavior::Page);
    if !rendered.is_empty() {
        out.push_str(&rendered);
        out.push('\n');
    }
}

fn render_block(
    block: &Block,
    document: &Document,
    context: &RenderContext<'_>,
    break_behavior: BreakBehavior,
) -> String {
    match block {
        Block::Paragraph(paragraph) => {
            render_paragraph(paragraph, document, context, break_behavior)
        }
        Block::Heading(heading) => render_heading(heading, document, context, break_behavior),
        Block::Table(table) => render_table(table, document, context),
        Block::Image(image) => render_asset_ref(
            document
                .assets
                .iter()
                .find(|asset| asset.id == image.asset_id)
                .expect("image asset must exist"),
            context.options.output_mode,
            true,
        ),
        Block::List(list) => render_list_block(list, document, context, break_behavior),
        Block::Fallback(fallback) => {
            render_fallback_block(fallback.record_id.as_str(), document, context)
        }
        Block::Unsupported(block) => {
            format!(
                "#docx-fallback(\"{}\", \"unsupported://{}\")",
                escape_typst_string(&block.feature),
                escape_typst_string(&block.id)
            )
        }
    }
}

fn render_block_sequence(
    blocks: &[Block],
    document: &Document,
    context: &RenderContext<'_>,
    break_behavior: BreakBehavior,
) -> String {
    let mut rendered = Vec::new();
    let mut index = 0usize;
    while index < blocks.len() {
        let chunk = match &blocks[index] {
            Block::Paragraph(paragraph) if paragraph.list.is_some() => {
                let (content, consumed) =
                    render_list_run(&blocks[index..], document, context, break_behavior);
                index += consumed;
                content
            }
            block => {
                index += 1;
                render_block(block, document, context, break_behavior)
            }
        };
        let chunk = chunk.trim_end().to_string();
        if !chunk.is_empty() {
            rendered.push(chunk);
        }
    }
    rendered.join("\n\n")
}

fn render_paragraph(
    paragraph: &Paragraph,
    document: &Document,
    context: &RenderContext<'_>,
    break_behavior: BreakBehavior,
) -> String {
    let mut parts = Vec::new();
    if paragraph.page_break_before
        && let Some(command) = render_break_command(break_behavior)
    {
        parts.push(command.to_string());
    }
    if let Some(before) = paragraph.spacing_before_twips.filter(|value| *value > 0) {
        parts.push(format!("#v({})", twips_to_typst(before)));
    }

    let mut segments = Vec::new();
    let mut current = Vec::new();
    for inline in &paragraph.inlines {
        if matches!(inline, Inline::PageBreak) {
            segments.push(current);
            current = Vec::new();
        } else {
            current.push(inline.clone());
        }
    }
    segments.push(current);

    for (index, segment) in segments.iter().enumerate() {
        if index > 0
            && let Some(command) = render_break_command(break_behavior)
        {
            parts.push(command.to_string());
        }
        let body = render_inlines(segment, document, context);
        if body.trim().is_empty() {
            parts.push(render_empty_paragraph(paragraph));
        } else {
            parts.push(wrap_alignment(&paragraph.style, body));
        }
    }

    if let Some(after) = paragraph.spacing_after_twips.filter(|value| *value > 0) {
        parts.push(format!("#v({})", twips_to_typst(after)));
    }
    parts.join("\n")
}

fn render_heading(
    heading: &docx2typst_model::Heading,
    document: &Document,
    context: &RenderContext<'_>,
    break_behavior: BreakBehavior,
) -> String {
    let mut parts = Vec::new();
    let mut current = Vec::new();
    for inline in &heading.inlines {
        if matches!(inline, Inline::PageBreak) {
            let rendered = render_heading_segment(heading.level, &current, document, context);
            if !rendered.is_empty() {
                parts.push(rendered);
            }
            if let Some(command) = render_break_command(break_behavior) {
                parts.push(command.to_string());
            }
            current.clear();
        } else {
            current.push(inline.clone());
        }
    }

    let rendered = render_heading_segment(heading.level, &current, document, context);
    if !rendered.is_empty() {
        parts.push(rendered);
    }

    parts.join("\n")
}

fn render_heading_segment(
    level: u8,
    inlines: &[Inline],
    document: &Document,
    context: &RenderContext<'_>,
) -> String {
    let content = render_inlines(inlines, document, context)
        .trim()
        .to_string();
    if content.is_empty() {
        String::new()
    } else {
        format!("{} {}", "=".repeat(level as usize), content)
    }
}

fn render_empty_paragraph(paragraph: &Paragraph) -> String {
    let line_height = paragraph
        .line_spacing_twips
        .or(paragraph
            .style
            .font_size_half_points
            .map(|value| value * 10))
        .unwrap_or(240);
    format!("#v({})", twips_to_typst(line_height))
}

fn render_table(
    table: &docx2typst_model::Table,
    document: &Document,
    context: &RenderContext<'_>,
) -> String {
    let default_insets = table
        .cell_margins
        .as_ref()
        .filter(|insets| should_emit_table_insets(insets));
    let mut out = String::new();
    let _ = writeln!(&mut out, "#table(");
    out.push_str("  stroke: none,\n");
    if let Some(gutter) = table
        .cell_spacing_twips
        .filter(|value| *value > 0)
        .map(twips_to_typst)
    {
        let _ = writeln!(&mut out, "  gutter: {gutter},");
    }
    if let Some(inset) = default_insets.and_then(render_edge_insets_twips) {
        let _ = writeln!(&mut out, "  inset: {inset},");
    }
    let _ = writeln!(&mut out, "  columns: {},", render_table_columns(table));
    if let Some(rows) = render_table_rows(table) {
        let _ = writeln!(&mut out, "  rows: {rows},");
    }

    let leading_header_rows = table.rows.iter().take_while(|row| row.is_header).count();
    let header_rows = table
        .rows
        .iter()
        .take(leading_header_rows)
        .enumerate()
        .fold(leading_header_rows, |extent, (row_index, row)| {
            let row_extent = row
                .cells
                .iter()
                .map(|cell| row_index + cell.row_span)
                .max()
                .unwrap_or(row_index + 1);
            extent.max(row_extent)
        })
        .min(table.rows.len());
    if header_rows > 0 {
        out.push_str("  table.header(\n");
        for row in table.rows.iter().take(header_rows) {
            for cell in &row.cells {
                let _ = writeln!(
                    &mut out,
                    "    {},",
                    render_table_cell(cell, default_insets, document, context,)
                );
            }
        }
        out.push_str("  ),\n");
    }

    for row in table.rows.iter().skip(header_rows) {
        for cell in &row.cells {
            let _ = writeln!(
                &mut out,
                "  {},",
                render_table_cell(cell, default_insets, document, context,)
            );
        }
    }
    out.push(')');
    wrap_table_layout(table, out)
}

fn render_table_columns(table: &docx2typst_model::Table) -> String {
    if table.columns.is_empty() {
        if let Some(specs) = infer_table_columns_from_cells(table) {
            let specs = specs
                .into_iter()
                .map(|width| {
                    width
                        .map(|value| format!("{value}fr"))
                        .unwrap_or_else(|| "auto".to_string())
                })
                .collect::<Vec<_>>()
                .join(", ");
            return format!("({specs})");
        }

        let columns = table
            .rows
            .iter()
            .map(|row| row.cells.iter().map(|cell| cell.col_span).sum::<usize>())
            .max()
            .unwrap_or(1)
            .max(1);
        return columns.to_string();
    }

    let specs = table
        .columns
        .iter()
        .map(|column| {
            column
                .width_twips
                .map(|value| format!("{value}fr"))
                .unwrap_or_else(|| "auto".to_string())
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("({specs})")
}

fn infer_table_columns_from_cells(table: &Table) -> Option<Vec<Option<u32>>> {
    let column_count = table
        .rows
        .iter()
        .map(|row| row.cells.iter().map(|cell| cell.col_span).sum::<usize>())
        .max()
        .unwrap_or(0);
    if column_count == 0 {
        return None;
    }

    let mut widths = vec![None; column_count];
    let mut saw_width = false;
    for row in &table.rows {
        for cell in &row.cells {
            let width = cell.width_twips.or_else(|| {
                cell.width_pct_fiftieths
                    .map(pct_fiftieths_to_twips_estimate)
            });
            let Some(width) = width else {
                continue;
            };
            saw_width = true;
            let span = cell.col_span.max(1);
            let base = width / span as u32;
            let remainder = width % span as u32;
            for offset in 0..span {
                let share = base + u32::from(offset == span - 1) * remainder;
                let slot = &mut widths[cell.column_index + offset];
                *slot = Some(slot.unwrap_or(share).max(share));
            }
        }
    }

    saw_width.then_some(widths)
}

fn render_table_rows(table: &Table) -> Option<String> {
    let column_widths = table_column_widths_twips(table);
    let specs = table
        .rows
        .iter()
        .map(|row| render_table_row_height(row, column_widths.as_deref()))
        .collect::<Vec<_>>();
    if !specs.iter().any(|spec| spec != "auto") {
        return None;
    }
    Some(format!("({})", specs.join(", ")))
}

fn wrap_table_layout(table: &Table, mut body: String) -> String {
    if let Some(width) = render_table_block_width(table) {
        body = format!("#block(width: {width}, breakable: true)[{body}]");
    }

    if let Some(indent) = table.indent_twips.filter(|value| *value > 0) {
        body = format!("#pad(left: {})[{body}]", twips_to_typst(indent as u32));
    }

    match table.alignment.as_deref() {
        Some("center") => format!("#align(center)[{body}]"),
        Some("right") => format!("#align(right)[{body}]"),
        _ => body,
    }
}

fn render_table_block_width(table: &Table) -> Option<String> {
    table
        .width_twips
        .filter(|value| *value > 0)
        .map(twips_to_typst)
        .or_else(|| {
            table
                .width_pct_fiftieths
                .map(|value| format!("{}%", trim_float(value as f32 / 50.0)))
        })
}

fn table_column_widths_twips(table: &Table) -> Option<Vec<u32>> {
    if !table.columns.is_empty()
        && table
            .columns
            .iter()
            .all(|column| column.width_twips.is_some())
    {
        return Some(
            table
                .columns
                .iter()
                .map(|column| column.width_twips.unwrap_or_default())
                .collect(),
        );
    }

    infer_table_columns_from_cells(table)
        .and_then(|widths| widths.into_iter().collect::<Option<Vec<_>>>())
}

fn render_table_row_height(row: &TableRow, column_widths: Option<&[u32]>) -> String {
    if !matches!(row.height_rule.as_deref(), Some("exact")) {
        return "auto".to_string();
    }
    let Some(height) = row.height_twips.filter(|value| *value > 0) else {
        return "auto".to_string();
    };
    if row_supports_fixed_height(row, column_widths) {
        twips_to_typst(height)
    } else {
        "auto".to_string()
    }
}

fn row_supports_fixed_height(row: &TableRow, column_widths: Option<&[u32]>) -> bool {
    let Some(row_height_twips) = row.height_twips else {
        return false;
    };
    row.cells
        .iter()
        .all(|cell| cell_supports_fixed_height(cell, column_widths, row_height_twips))
}

fn cell_supports_fixed_height(
    cell: &TableCell,
    column_widths: Option<&[u32]>,
    row_height_twips: u32,
) -> bool {
    if row_height_twips == 0 || cell.blocks.is_empty() {
        return true;
    }
    let Some(width_twips) = cell_effective_width_twips(cell, column_widths) else {
        return false;
    };
    let estimated = estimate_table_cell_content_height_twips(cell, width_twips);
    estimated
        .map(|value| value <= row_height_twips.saturating_mul(cell.row_span.max(1) as u32))
        .unwrap_or(false)
}

fn cell_effective_width_twips(cell: &TableCell, column_widths: Option<&[u32]>) -> Option<u32> {
    cell.width_twips
        .or_else(|| {
            column_widths.map(|widths| {
                widths
                    .iter()
                    .skip(cell.column_index)
                    .take(cell.col_span.max(1))
                    .copied()
                    .sum()
            })
        })
        .or_else(|| {
            cell.width_pct_fiftieths
                .map(pct_fiftieths_to_twips_estimate)
        })
}

fn estimate_table_cell_content_height_twips(cell: &TableCell, width_twips: u32) -> Option<u32> {
    let mut total = 0u32;
    for block in &cell.blocks {
        total = total.checked_add(estimate_block_height_twips(block, width_twips)?)?;
    }

    let vertical_padding = cell
        .margins_twips
        .as_ref()
        .map(|insets| insets.top.unwrap_or(0) + insets.bottom.unwrap_or(0))
        .unwrap_or(0);
    total.checked_add(vertical_padding)
}

fn estimate_block_height_twips(block: &Block, width_twips: u32) -> Option<u32> {
    match block {
        Block::Paragraph(paragraph) => estimate_paragraph_height_twips(paragraph, width_twips),
        Block::Heading(heading) => {
            let font_half_points = heading.style.font_size_half_points.unwrap_or(24);
            let line_height = font_half_points * 10;
            let chars_per_line = estimate_chars_per_line(width_twips, font_half_points)?;
            let text = inline_plain_text(&heading.inlines)?;
            let lines = estimate_text_lines(&text, chars_per_line);
            Some(lines.saturating_mul(line_height).max(line_height))
        }
        _ => None,
    }
}

fn estimate_paragraph_height_twips(paragraph: &Paragraph, width_twips: u32) -> Option<u32> {
    if paragraph.list.is_some() {
        return None;
    }
    let font_half_points = paragraph.style.font_size_half_points.unwrap_or(20);
    let line_height = paragraph
        .line_spacing_twips
        .unwrap_or(font_half_points * 10);
    let chars_per_line = estimate_chars_per_line(width_twips, font_half_points)?;
    let text = inline_plain_text(&paragraph.inlines)?;
    let lines = estimate_text_lines(&text, chars_per_line);
    Some(
        paragraph.spacing_before_twips.unwrap_or(0)
            + paragraph.spacing_after_twips.unwrap_or(0)
            + lines.saturating_mul(line_height).max(line_height),
    )
}

fn inline_plain_text(inlines: &[Inline]) -> Option<String> {
    let mut text = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(run) => text.push_str(&run.text),
            Inline::Hyperlink(link) => text.push_str(&inline_plain_text(&link.inlines)?),
            Inline::LineBreak => text.push('\n'),
            Inline::Bookmark(_) => {}
            Inline::PageBreak
            | Inline::NoteReference(_)
            | Inline::Image(_)
            | Inline::Fallback(_)
            | Inline::PageNumber(_) => return None,
        }
    }
    Some(text)
}

fn estimate_chars_per_line(width_twips: u32, font_half_points: u32) -> Option<usize> {
    if width_twips == 0 || font_half_points == 0 {
        return None;
    }
    let font_points = font_half_points as f32 / 2.0;
    let width_points = width_twips as f32 / 20.0;
    let average_char_width = (font_points * 0.55).max(1.0);
    Some((width_points / average_char_width).floor().max(1.0) as usize)
}

fn estimate_text_lines(text: &str, chars_per_line: usize) -> u32 {
    let mut lines = 0u32;
    for raw_line in text.split('\n') {
        let len = raw_line.trim().chars().count().max(1);
        lines += len.div_ceil(chars_per_line) as u32;
    }
    lines.max(1)
}

fn pct_fiftieths_to_twips_estimate(value: u32) -> u32 {
    value.saturating_mul(2)
}

fn render_table_cell(
    cell: &docx2typst_model::TableCell,
    default_insets: Option<&EdgeInsetsTwips>,
    document: &Document,
    context: &RenderContext<'_>,
) -> String {
    let mut arguments = Vec::new();
    if cell.col_span > 1 {
        arguments.push(format!("colspan: {}", cell.col_span));
    }
    if cell.row_span > 1 {
        arguments.push(format!("rowspan: {}", cell.row_span));
    }
    if let Some(fill) = &cell.background_color {
        arguments.push(format!("fill: {}", render_color(fill)));
    }
    if let Some(stroke) = render_table_cell_stroke(&cell.borders) {
        arguments.push(format!("stroke: {stroke}"));
    }
    if cell.margins_twips.is_some() {
        let merged_insets = merge_edge_insets(default_insets, cell.margins_twips.as_ref());
        if let Some(inset) = merged_insets.as_ref().and_then(render_edge_insets_twips) {
            arguments.push(format!("inset: {inset}"));
        }
    }
    if let Some(align) = render_table_cell_alignment(cell.vertical_align.as_deref()) {
        arguments.push(format!("align: {align}"));
    }

    let content = render_block_sequence(&cell.blocks, document, context, BreakBehavior::Ignore);
    let content = if content.trim().is_empty() {
        " ".to_string()
    } else {
        content
    };

    if arguments.is_empty() {
        format!("[{content}]")
    } else {
        format!("table.cell({})[{content}]", arguments.join(", "))
    }
}

fn merge_edge_insets(
    base: Option<&EdgeInsetsTwips>,
    top: Option<&EdgeInsetsTwips>,
) -> Option<EdgeInsetsTwips> {
    if base.is_none() && top.is_none() {
        return None;
    }
    Some(EdgeInsetsTwips {
        top: top
            .and_then(|insets| insets.top)
            .or_else(|| base.and_then(|insets| insets.top)),
        right: top
            .and_then(|insets| insets.right)
            .or_else(|| base.and_then(|insets| insets.right)),
        bottom: top
            .and_then(|insets| insets.bottom)
            .or_else(|| base.and_then(|insets| insets.bottom)),
        left: top
            .and_then(|insets| insets.left)
            .or_else(|| base.and_then(|insets| insets.left)),
    })
}

fn render_edge_insets_twips(insets: &EdgeInsetsTwips) -> Option<String> {
    if insets.top.is_none()
        && insets.right.is_none()
        && insets.bottom.is_none()
        && insets.left.is_none()
    {
        return None;
    }

    Some(format!(
        "(top: {}, right: {}, bottom: {}, left: {})",
        insets
            .top
            .map(twips_to_typst)
            .unwrap_or_else(|| "0pt".to_string()),
        insets
            .right
            .map(twips_to_typst)
            .unwrap_or_else(|| "0pt".to_string()),
        insets
            .bottom
            .map(twips_to_typst)
            .unwrap_or_else(|| "0pt".to_string()),
        insets
            .left
            .map(twips_to_typst)
            .unwrap_or_else(|| "0pt".to_string()),
    ))
}

fn should_emit_table_insets(insets: &EdgeInsetsTwips) -> bool {
    !is_word_default_table_insets(insets)
}

fn is_word_default_table_insets(insets: &EdgeInsetsTwips) -> bool {
    matches!(insets.top, Some(0))
        && matches!(insets.bottom, Some(0))
        && is_near_twips(insets.left, 108, 12)
        && is_near_twips(insets.right, 108, 12)
}

fn is_near_twips(value: Option<u32>, target: u32, tolerance: u32) -> bool {
    value
        .map(|value| value.abs_diff(target) <= tolerance)
        .unwrap_or(false)
}

fn render_table_cell_stroke(borders: &TableCellBorders) -> Option<String> {
    let mut sides = Vec::new();
    if let Some(border) = borders.top.as_ref() {
        sides.push(format!("top: {}", render_table_border_line(border)));
    }
    if let Some(border) = borders.right.as_ref() {
        sides.push(format!("right: {}", render_table_border_line(border)));
    }
    if let Some(border) = borders.bottom.as_ref() {
        sides.push(format!("bottom: {}", render_table_border_line(border)));
    }
    if let Some(border) = borders.left.as_ref() {
        sides.push(format!("left: {}", render_table_border_line(border)));
    }

    if sides.is_empty() {
        None
    } else {
        Some(format!("({})", sides.join(", ")))
    }
}

fn render_table_border_line(border: &TableBorderLine) -> String {
    if matches!(border.style.as_deref(), Some("none" | "nil")) {
        return "none".to_string();
    }
    let width = border
        .width_eighth_points
        .map(|value| format!("{}pt", trim_float(value as f32 / 8.0)));
    let color = border.color.as_deref().map(render_color);
    match (width, color) {
        (Some(width), Some(color)) => format!("{width} + {color}"),
        (Some(width), None) => width,
        (None, Some(color)) => format!("1pt + {color}"),
        (None, None) => "1pt".to_string(),
    }
}

fn render_table_cell_alignment(vertical_align: Option<&str>) -> Option<&'static str> {
    match vertical_align {
        Some("center") => Some("horizon"),
        Some("bottom") => Some("bottom"),
        Some("top") => Some("top"),
        _ => None,
    }
}

fn render_list_run(
    blocks: &[Block],
    document: &Document,
    context: &RenderContext<'_>,
    break_behavior: BreakBehavior,
) -> (String, usize) {
    let Some(Block::Paragraph(first)) = blocks.first() else {
        return (String::new(), 0);
    };
    let target_kind = first.list.as_ref().map(|meta| meta.kind.clone());
    let target_level = first.list.as_ref().map(|meta| meta.level);
    let target_list_id = first
        .list
        .as_ref()
        .and_then(|meta| meta.list_id.as_deref())
        .map(str::to_string);
    let target_numbering = first
        .list
        .as_ref()
        .and_then(|meta| meta.numbering.as_deref())
        .map(str::to_string);

    let mut items = Vec::new();
    let mut consumed = 0usize;
    for block in blocks {
        let Block::Paragraph(paragraph) = block else {
            break;
        };
        let Some(meta) = &paragraph.list else {
            break;
        };
        if Some(meta.kind.clone()) != target_kind
            || Some(meta.level) != target_level
            || meta.list_id.as_deref() != target_list_id.as_deref()
            || meta.numbering.as_deref() != target_numbering.as_deref()
        {
            break;
        }
        items.push(ListItem {
            blocks: vec![Block::Paragraph(Paragraph {
                list: None,
                ..paragraph.clone()
            })],
        });
        consumed += 1;
    }

    let list = ListBlock {
        id: "legacy-list".to_string(),
        kind: target_kind.unwrap_or(docx2typst_model::ListKind::Bullet),
        numbering: target_numbering,
        items,
    };
    let item_break_behavior = nested_break_behavior(break_behavior);
    (
        render_list_block_with_behavior(&list, document, context, item_break_behavior),
        consumed,
    )
}

fn render_list_item(
    item: &ListItem,
    document: &Document,
    context: &RenderContext<'_>,
    break_behavior: BreakBehavior,
) -> String {
    render_block_sequence(&item.blocks, document, context, break_behavior)
}

fn render_list_block(
    list: &ListBlock,
    document: &Document,
    context: &RenderContext<'_>,
    break_behavior: BreakBehavior,
) -> String {
    render_list_block_with_behavior(
        list,
        document,
        context,
        nested_break_behavior(break_behavior),
    )
}

fn render_list_block_with_behavior(
    list: &ListBlock,
    document: &Document,
    context: &RenderContext<'_>,
    break_behavior: BreakBehavior,
) -> String {
    let mut out = String::new();
    let _ = writeln!(&mut out, "#{}(", render_list_function_name(&list.kind));
    if matches!(list.kind, docx2typst_model::ListKind::Numbered)
        && let Some(pattern) = render_enum_numbering(list.numbering.as_deref())
    {
        let _ = writeln!(&mut out, "  numbering: \"{pattern}\",");
    }
    for item in &list.items {
        let _ = writeln!(
            &mut out,
            "  [{}],",
            render_list_item(item, document, context, break_behavior)
        );
    }
    out.push(')');
    out
}

fn render_list_function_name(kind: &docx2typst_model::ListKind) -> &'static str {
    match kind {
        docx2typst_model::ListKind::Bullet => "list",
        docx2typst_model::ListKind::Numbered => "enum",
    }
}

fn render_enum_numbering(numbering: Option<&str>) -> Option<&'static str> {
    match numbering.unwrap_or("decimal") {
        "decimal" => Some("1."),
        "decimalZero" => Some("01."),
        "lowerLetter" => Some("a."),
        "upperLetter" => Some("A."),
        "lowerRoman" => Some("i."),
        "upperRoman" => Some("I."),
        _ => None,
    }
}

fn render_inline(inline: &Inline, document: &Document, context: &RenderContext<'_>) -> String {
    match inline {
        Inline::Text(run) => render_text_run(run, context),
        Inline::PageNumber(style) => render_page_number(style, context),
        Inline::LineBreak => "\\\n".to_string(),
        Inline::PageBreak => "#pagebreak()".to_string(),
        Inline::Hyperlink(link) => render_hyperlink(link, document, context),
        Inline::NoteReference(note) => {
            if let Some(content) = document.notes.get(&note.note_id) {
                let body = render_block_sequence(
                    &content.blocks,
                    document,
                    context,
                    BreakBehavior::Ignore,
                );
                let function = if matches!(note.kind, NoteKind::Endnote) {
                    "footnote"
                } else {
                    "footnote"
                };
                format!("#{function}[{}]", body)
            } else {
                String::new()
            }
        }
        Inline::Bookmark(bookmark) => format!("#metadata(none) <{}>", bookmark.name),
        Inline::Image(image) => document
            .assets
            .iter()
            .find(|asset| asset.id == image.asset_id)
            .map(|asset| render_inline_image(image, asset, context.options.output_mode))
            .unwrap_or_default(),
        Inline::Fallback(fallback) => {
            render_fallback_inline(fallback.record_id.as_str(), document, context)
        }
    }
}

fn render_inlines(inlines: &[Inline], document: &Document, context: &RenderContext<'_>) -> String {
    let mut output = String::new();
    for inline in inlines {
        output.push_str(&render_inline(inline, document, context));
    }
    output
}

fn render_text_run(run: &TextRun, context: &RenderContext<'_>) -> String {
    let body = escape_typst_text(&run.text);
    if body.is_empty() {
        return body;
    }
    render_styled_inline_body(body, &run.style, context)
}

fn render_page_number(style: &ResolvedStyle, context: &RenderContext<'_>) -> String {
    render_styled_inline_body(
        "#context counter(page).display()".to_string(),
        style,
        context,
    )
}

fn render_styled_inline_body(
    mut body: String,
    style: &ResolvedStyle,
    context: &RenderContext<'_>,
) -> String {
    let mut wrappers = Vec::new();
    if let Some(color) = &style.color {
        wrappers.push(format!("text(fill: {})", render_color(color)));
    }
    if let Some(font) = &style.font_family {
        let resolved = resolve_font(context, font);
        wrappers.push(format!(
            "text(font: \"{}\")",
            escape_typst_string(&resolved)
        ));
    }
    if let Some(size) = style.font_size_half_points {
        let points = size as f32 / 2.0;
        wrappers.push(format!("text(size: {}pt)", trim_float(points)));
    }
    if style.bold {
        wrappers.push("strong".to_string());
    }
    if style.italic {
        wrappers.push("emph".to_string());
    }
    if style.underline {
        wrappers.push("underline".to_string());
    }
    if style.strike {
        wrappers.push("strike".to_string());
    }
    if style.superscript {
        wrappers.push("super".to_string());
    }
    if style.subscript {
        wrappers.push("sub".to_string());
    }

    for wrapper in wrappers.into_iter().rev() {
        body = format!("#{wrapper}[{body}]");
    }
    body
}

fn render_fallback_block(
    record_id: &str,
    document: &Document,
    context: &RenderContext<'_>,
) -> String {
    let Some(record) = find_fallback_record(document, record_id) else {
        return String::new();
    };
    let Some(asset) = document
        .assets
        .iter()
        .find(|asset| asset.id == record.asset_id)
    else {
        return String::new();
    };
    render_fallback_asset(record, asset, context.options.output_mode, true)
}

fn render_fallback_inline(
    record_id: &str,
    document: &Document,
    context: &RenderContext<'_>,
) -> String {
    let Some(record) = find_fallback_record(document, record_id) else {
        return String::new();
    };
    let Some(asset) = document
        .assets
        .iter()
        .find(|asset| asset.id == record.asset_id)
    else {
        return String::new();
    };
    render_fallback_asset(record, asset, context.options.output_mode, false)
}

fn find_fallback_record<'a>(document: &'a Document, record_id: &str) -> Option<&'a FallbackRecord> {
    document
        .fallbacks
        .iter()
        .find(|record| record.id == record_id)
}

fn render_fallback_asset(
    record: &FallbackRecord,
    asset: &AssetReference,
    mode: OutputMode,
    block_level: bool,
) -> String {
    let source = match mode {
        OutputMode::Bundle => format!(
            "\"{}\"",
            escape_typst_string(
                asset
                    .emitted_path
                    .as_deref()
                    .unwrap_or(&asset.original_path)
            )
        ),
        OutputMode::SingleFile => emit_bytes_literal(&asset.bytes),
    };
    let format = media_type_to_typst_format(&asset.media_type);
    let alt = record
        .alt_text
        .as_deref()
        .map(|text| format!("\"{}\"", escape_typst_string(text)))
        .unwrap_or_else(|| "none".to_string());
    let kind = match record.kind {
        FallbackKind::Drawing => "drawing",
        FallbackKind::Equation => "equation",
    };
    let mut rendered = format!("#docx-fallback(\"{kind}\", {source}, format: {format}, alt: {alt}");
    if let Some(width) = record.width_emu.map(emu_to_typst) {
        let _ = write!(&mut rendered, ", width: {width}");
    }
    if let Some(height) = record.height_emu.map(emu_to_typst) {
        let _ = write!(&mut rendered, ", height: {height}");
    }
    rendered.push(')');
    if block_level {
        format!("#figure({rendered})")
    } else {
        rendered
    }
}

fn render_color(color: &str) -> String {
    let trimmed = color.trim();
    let normalized = trimmed.trim_start_matches('#');
    match normalized {
        "AUTO" => "black".to_string(),
        "WINDOWTEXT" => "black".to_string(),
        "WINDOW" => "white".to_string(),
        _ if is_hex_color(normalized) => format!("rgb(\"#{}\")", normalized),
        _ => format!("rgb(\"{}\")", escape_typst_string(trimmed)),
    }
}

fn is_hex_color(value: &str) -> bool {
    matches!(value.len(), 3 | 6 | 8) && value.chars().all(|char| char.is_ascii_hexdigit())
}

fn render_hyperlink(link: &Hyperlink, document: &Document, context: &RenderContext<'_>) -> String {
    let body = render_inlines(&link.inlines, document, context);
    format!("#link(\"{}\")[{}]", escape_typst_string(&link.target), body)
}

fn render_header_footer(
    content: &HeaderFooter,
    document: &Document,
    context: &RenderContext<'_>,
) -> String {
    let mut out = String::new();
    for block in &content.blocks {
        if let Some(rendered) = render_header_footer_block(block, document, context) {
            if out.is_empty() {
                match content.kind {
                    HeaderFooterKind::Header => out.push_str("#docx-header["),
                    HeaderFooterKind::Footer => out.push_str("#docx-footer["),
                }
            }
            out.push_str(&rendered);
        }
    }
    if !out.is_empty() {
        out.push(']');
    }
    out
}

fn render_header_footer_block(
    block: &Block,
    document: &Document,
    context: &RenderContext<'_>,
) -> Option<String> {
    match block {
        Block::Paragraph(paragraph) => {
            let body = render_header_footer_inlines(&paragraph.inlines, document, context);
            if body.trim().is_empty() {
                None
            } else {
                Some(wrap_alignment(&paragraph.style, body))
            }
        }
        _ => Some(render_block(
            block,
            document,
            context,
            BreakBehavior::Ignore,
        )),
    }
}

fn render_header_footer_inlines(
    inlines: &[Inline],
    document: &Document,
    context: &RenderContext<'_>,
) -> String {
    let mut out = String::new();
    for inline in inlines {
        match inline {
            Inline::PageBreak => {}
            Inline::Image(image) if is_page_overlay_image(image) => {}
            _ => out.push_str(&render_inline(inline, document, context)),
        }
    }
    out
}

fn render_break_command(break_behavior: BreakBehavior) -> Option<&'static str> {
    match break_behavior {
        BreakBehavior::Page => Some("#pagebreak()"),
        BreakBehavior::Column => Some("#colbreak()"),
        BreakBehavior::Ignore => None,
    }
}

fn nested_break_behavior(break_behavior: BreakBehavior) -> BreakBehavior {
    match break_behavior {
        BreakBehavior::Page | BreakBehavior::Column => BreakBehavior::Column,
        BreakBehavior::Ignore => BreakBehavior::Ignore,
    }
}

fn render_page_layer(
    section: &Section,
    document: &Document,
    context: &RenderContext<'_>,
    behind_doc: bool,
) -> Option<String> {
    let page_setup = section.page_setup.as_ref()?;
    let mut items = Vec::new();

    if let Some(header) = &section.header {
        items.extend(render_header_footer_layer_items(
            header, page_setup, document, context, behind_doc,
        ));
    }
    if let Some(footer) = &section.footer {
        items.extend(render_header_footer_layer_items(
            footer, page_setup, document, context, behind_doc,
        ));
    }

    if items.is_empty() {
        None
    } else {
        Some(items.join("\n"))
    }
}

fn compute_safe_top_margin(section: &Section, page_setup: &PageSetup) -> f64 {
    let base = page_setup
        .margin_top_twips
        .map(twips_to_inches)
        .unwrap_or(1.0);
    let overlay_bottom = section
        .header
        .as_ref()
        .map(|content| {
            overlay_images(content)
                .filter(|image| image.behind_doc)
                .map(|image| {
                    overlay_y_position_inches(image, HeaderFooterKind::Header, page_setup)
                        + image.height_emu.map(emu_to_inches_u64).unwrap_or(0.0)
                })
                .fold(0.0, f64::max)
        })
        .unwrap_or(0.0);
    base.max(overlay_bottom)
}

fn compute_safe_bottom_margin(section: &Section, page_setup: &PageSetup) -> f64 {
    let base = page_setup
        .margin_bottom_twips
        .map(twips_to_inches)
        .unwrap_or(1.0);
    let Some(page_height) = page_setup.height_twips.map(twips_to_inches) else {
        return base;
    };
    let overlay_top = section.footer.as_ref().and_then(|content| {
        overlay_images(content)
            .filter(|image| image.behind_doc)
            .map(|image| overlay_y_position_inches(image, HeaderFooterKind::Footer, page_setup))
            .min_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal))
    });
    match overlay_top {
        Some(top) => base.max((page_height - top).max(0.0)),
        None => base,
    }
}

fn overlay_images(content: &HeaderFooter) -> impl Iterator<Item = &docx2typst_model::ImageInline> {
    content
        .blocks
        .iter()
        .flat_map(|block| match block {
            Block::Paragraph(paragraph) => paragraph
                .inlines
                .iter()
                .filter_map(|inline| match inline {
                    Inline::Image(image) if is_page_overlay_image(image) => Some(image),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect::<Vec<_>>()
        .into_iter()
}

fn render_header_footer_layer_items(
    content: &HeaderFooter,
    page_setup: &PageSetup,
    document: &Document,
    context: &RenderContext<'_>,
    behind_doc: bool,
) -> Vec<String> {
    let mut items = Vec::new();
    for block in &content.blocks {
        let Block::Paragraph(paragraph) = block else {
            continue;
        };
        for inline in &paragraph.inlines {
            let Inline::Image(image) = inline else {
                continue;
            };
            if !is_page_overlay_image(image) || image.behind_doc != behind_doc {
                continue;
            }
            let Some(asset) = document
                .assets
                .iter()
                .find(|asset| asset.id == image.asset_id)
            else {
                continue;
            };
            items.push(render_page_overlay_image(
                image,
                asset,
                context.options.output_mode,
                content.kind.clone(),
                page_setup,
            ));
        }
    }
    items
}

fn wrap_alignment(style: &ResolvedStyle, body: String) -> String {
    match style.alignment.as_deref() {
        Some("center") => format!("#align(center)[{body}]"),
        Some("right") => format!("#align(right)[{body}]"),
        Some("both") => format!("#par(justify: true)[{body}]"),
        _ => body,
    }
}

fn render_asset_ref(asset: &AssetReference, mode: OutputMode, block_level: bool) -> String {
    match mode {
        OutputMode::Bundle => {
            let emitted = asset
                .emitted_path
                .as_deref()
                .unwrap_or(&asset.original_path);
            let format = media_type_to_typst_format(&asset.media_type);
            let image = format!(
                "#docx-inline-image(\"{}\", format: {})",
                escape_typst_string(emitted),
                format
            );
            if block_level {
                format!("#figure({image})")
            } else {
                image
            }
        }
        OutputMode::SingleFile => {
            let bytes = emit_bytes_literal(&asset.bytes);
            let format = media_type_to_typst_format(&asset.media_type);
            let image = format!("#docx-inline-image({bytes}, format: {format})");
            if block_level {
                format!("#figure({image})")
            } else {
                image
            }
        }
    }
}

fn render_inline_image(
    image: &docx2typst_model::ImageInline,
    asset: &AssetReference,
    mode: OutputMode,
) -> String {
    let rendered = render_inline_image_content(image, asset, mode);

    match (image.offset_x_emu, image.offset_y_emu) {
        (Some(dx), Some(dy)) if dx != 0 || dy != 0 => format!(
            "#move(dx: {}, dy: {})[{}]",
            emu_offset_to_typst(dx),
            emu_offset_to_typst(dy),
            rendered
        ),
        (Some(dx), None) if dx != 0 => format!(
            "#move(dx: {}, dy: 0in)[{}]",
            emu_offset_to_typst(dx),
            rendered
        ),
        (None, Some(dy)) if dy != 0 => format!(
            "#move(dx: 0in, dy: {})[{}]",
            emu_offset_to_typst(dy),
            rendered
        ),
        _ => rendered,
    }
}

fn render_inline_image_content(
    image: &docx2typst_model::ImageInline,
    asset: &AssetReference,
    mode: OutputMode,
) -> String {
    let source = match mode {
        OutputMode::Bundle => format!(
            "\"{}\"",
            escape_typst_string(
                asset
                    .emitted_path
                    .as_deref()
                    .unwrap_or(&asset.original_path)
            )
        ),
        OutputMode::SingleFile => emit_bytes_literal(&asset.bytes),
    };
    let format = media_type_to_typst_format(&asset.media_type);
    let alt = image
        .alt_text
        .as_deref()
        .map(|text| format!("\"{}\"", escape_typst_string(text)))
        .unwrap_or_else(|| "none".to_string());

    let mut rendered = format!("#docx-inline-image({source}, format: {format}, alt: {alt}");
    if let Some(width) = image.width_emu.map(emu_to_typst) {
        let _ = write!(&mut rendered, ", width: {width}");
    }
    if let Some(height) = image.height_emu.map(emu_to_typst) {
        let _ = write!(&mut rendered, ", height: {height}");
    }
    rendered.push(')');
    rendered
}

fn render_page_overlay_image(
    image: &docx2typst_model::ImageInline,
    asset: &AssetReference,
    mode: OutputMode,
    kind: HeaderFooterKind,
    page_setup: &PageSetup,
) -> String {
    let content = render_inline_image_content(image, asset, mode);
    format!(
        "#place(top + left, dx: {}, dy: {})[{}]",
        inches_to_typst(overlay_x_position_inches(image, page_setup)),
        inches_to_typst(overlay_y_position_inches(image, kind, page_setup)),
        content
    )
}

fn overlay_x_position_inches(image: &docx2typst_model::ImageInline, page_setup: &PageSetup) -> f64 {
    let margin_left_in = page_setup
        .margin_left_twips
        .map(twips_to_inches)
        .unwrap_or(0.0);
    let offset_in = image.offset_x_emu.map(emu_to_inches_i64).unwrap_or(0.0);
    margin_left_in + offset_in
}

fn overlay_y_position_inches(
    image: &docx2typst_model::ImageInline,
    kind: HeaderFooterKind,
    page_setup: &PageSetup,
) -> f64 {
    let offset_in = image.offset_y_emu.map(emu_to_inches_i64).unwrap_or(0.0);
    match kind {
        HeaderFooterKind::Header => {
            page_setup
                .header_twips
                .or(page_setup.margin_top_twips)
                .map(twips_to_inches)
                .unwrap_or(0.0)
                + offset_in
        }
        HeaderFooterKind::Footer => {
            page_setup.height_twips.map(twips_to_inches).unwrap_or(0.0)
                - page_setup
                    .footer_twips
                    .or(page_setup.margin_bottom_twips)
                    .map(twips_to_inches)
                    .unwrap_or(0.0)
                + offset_in
        }
    }
}

fn is_page_overlay_image(image: &docx2typst_model::ImageInline) -> bool {
    image.floating
}

fn emit_bytes_literal(bytes: &[u8]) -> String {
    let joined = bytes
        .iter()
        .map(|byte| byte.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!("bytes(({joined}))")
}

fn media_type_to_typst_format(media_type: &str) -> &'static str {
    match media_type {
        "image/png" => "\"png\"",
        "image/jpeg" => "\"jpg\"",
        "image/gif" => "\"gif\"",
        "image/svg+xml" => "\"svg\"",
        "application/pdf" => "\"pdf\"",
        "image/webp" => "\"webp\"",
        _ => "auto",
    }
}

fn prepare_assets(mut assets: Vec<AssetReference>, output_mode: OutputMode) -> Vec<AssetReference> {
    if matches!(output_mode, OutputMode::Bundle) {
        for asset in &mut assets {
            let extension = file_extension(&asset.original_path, &asset.media_type);
            asset.emitted_path = Some(format!("assets/{}.{}", asset.id, extension));
        }
    } else {
        for asset in &mut assets {
            asset.emitted_path = None;
        }
    }
    assets
}

fn prepare_embedded_fonts(mut fonts: Vec<EmbeddedFont>) -> Vec<EmbeddedFont> {
    for (index, font) in fonts.iter_mut().enumerate() {
        if !font.usable || font.bytes.is_empty() {
            font.emitted_path = None;
            continue;
        }
        let extension = font_file_extension(font);
        font.emitted_path = Some(format!(
            "fonts/font-{}-{}-{}.{}",
            index + 1,
            slugify_font_name(&font.family),
            slugify_font_name(&font.variant),
            extension
        ));
    }
    fonts
}

fn file_extension(path: &str, media_type: &str) -> String {
    PathBuf::from(path)
        .extension()
        .map(|ext| ext.to_string_lossy().into_owned())
        .filter(|ext| !ext.is_empty())
        .unwrap_or_else(|| match media_type {
            "image/png" => "png".to_string(),
            "image/jpeg" => "jpg".to_string(),
            "image/gif" => "gif".to_string(),
            "image/svg+xml" => "svg".to_string(),
            "application/pdf" => "pdf".to_string(),
            _ => "bin".to_string(),
        })
}

fn font_file_extension(font: &EmbeddedFont) -> String {
    PathBuf::from(&font.original_path)
        .extension()
        .map(|ext| ext.to_string_lossy().into_owned())
        .filter(|ext| !ext.is_empty() && !ext.eq_ignore_ascii_case("odttf"))
        .unwrap_or_else(|| match font.media_type.as_str() {
            "font/otf" | "application/vnd.ms-opentype" => "otf".to_string(),
            _ => "ttf".to_string(),
        })
}

fn slugify_font_name(value: &str) -> String {
    let slug = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    slug.trim_matches('-').to_string()
}

fn resolve_font(context: &RenderContext<'_>, requested: &str) -> String {
    let (resolved, reason) = if context
        .embedded_fonts
        .iter()
        .any(|font| font.usable && font.family.eq_ignore_ascii_case(requested))
    {
        (requested.to_string(), "document embedded font".to_string())
    } else {
        match context.options.profile.as_ref() {
            Some(profile) if profile.fonts.embedded_paths.contains_key(requested) => {
                (requested.to_string(), "profile font path".to_string())
            }
            Some(profile)
                if profile
                    .fonts
                    .embedded_paths
                    .keys()
                    .any(|candidate| candidate.eq_ignore_ascii_case(requested)) =>
            {
                (requested.to_string(), "profile font path".to_string())
            }
            Some(profile) => profile
                .fonts
                .substitutions
                .get(requested)
                .cloned()
                .or_else(|| {
                    profile
                        .fonts
                        .substitutions
                        .iter()
                        .find_map(|(candidate, resolved)| {
                            candidate
                                .eq_ignore_ascii_case(requested)
                                .then(|| resolved.clone())
                        })
                })
                .map(|resolved| (resolved, "profile substitution".to_string()))
                .unwrap_or_else(|| (requested.to_string(), "document font retained".to_string())),
            None => (requested.to_string(), "document font retained".to_string()),
        }
    };
    let decision = FontDecision {
        requested: requested.to_string(),
        resolved: resolved.clone(),
        reason,
        location: lookup_font_location(context, requested),
    };
    let mut decisions = context.font_decisions.borrow_mut();
    if !decisions.iter().any(|existing| existing == &decision) {
        maybe_push_font_substitution_diagnostic(context, &decision);
        decisions.push(decision);
    }
    resolved
}

fn lookup_font_location(context: &RenderContext<'_>, requested: &str) -> Option<SourceLocation> {
    context
        .font_usage_locations
        .get(requested)
        .cloned()
        .or_else(|| {
            context
                .font_usage_locations
                .iter()
                .find_map(|(font, location)| {
                    font.eq_ignore_ascii_case(requested)
                        .then(|| location.clone())
                })
        })
}

fn maybe_push_font_substitution_diagnostic(context: &RenderContext<'_>, decision: &FontDecision) {
    if decision.requested == decision.resolved {
        return;
    }

    let mut diagnostics = context.diagnostics.borrow_mut();
    if diagnostics.iter().any(|existing| {
        existing.code == "FONT_SUBSTITUTION"
            && existing.location == decision.location
            && existing
                .context
                .get("requested")
                .is_some_and(|value| value == &decision.requested)
            && existing
                .context
                .get("resolved")
                .is_some_and(|value| value == &decision.resolved)
    }) {
        return;
    }

    let mut context_map = IndexMap::new();
    context_map.insert("requested".to_string(), decision.requested.clone());
    context_map.insert("resolved".to_string(), decision.resolved.clone());
    context_map.insert("reason".to_string(), decision.reason.clone());
    diagnostics.push(Diagnostic {
        code: "FONT_SUBSTITUTION".to_string(),
        severity: Severity::Warning,
        message: format!(
            "substituted DOCX font `{}` with `{}`",
            decision.requested, decision.resolved
        ),
        location: decision.location.clone(),
        context: context_map,
    });
}

fn collect_font_search_paths(
    options: &ConversionOptions,
    embedded_fonts: &[EmbeddedFont],
) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(profile) = &options.profile {
        for path in profile.fonts.embedded_paths.values() {
            let candidate = if path.is_dir() {
                path.clone()
            } else {
                path.parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| path.clone())
            };
            if !paths.iter().any(|existing| existing == &candidate) {
                paths.push(candidate);
            }
        }
    }
    if embedded_fonts
        .iter()
        .any(|font| font.usable && !font.bytes.is_empty())
        && let Some(output_path) = &options.output_path
    {
        let candidate = match options.output_mode {
            OutputMode::Bundle => output_path.join("fonts"),
            OutputMode::SingleFile => single_file_font_dir(output_path),
        };
        if !paths.iter().any(|existing| existing == &candidate) {
            paths.push(candidate);
        }
    }
    paths
}

pub fn single_file_font_dir(output_path: &Path) -> PathBuf {
    let stem = output_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("docx2typst");
    let sibling = format!("{stem}.fonts");
    output_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(sibling)
}

fn count_paragraphs(document: &Document) -> usize {
    document
        .sections
        .iter()
        .map(|section| {
            section
                .blocks
                .iter()
                .filter(|block| matches!(block, Block::Paragraph(_) | Block::Heading(_)))
                .count()
        })
        .sum()
}

fn count_tables(document: &Document) -> usize {
    document
        .sections
        .iter()
        .map(|section| {
            section
                .blocks
                .iter()
                .filter(|block| matches!(block, Block::Table(_)))
                .count()
        })
        .sum()
}

fn twips_to_typst(value: u32) -> String {
    inches_to_typst(twips_to_inches(value))
}

fn emu_to_typst(value: u64) -> String {
    inches_to_typst(emu_to_inches_u64(value))
}

fn emu_offset_to_typst(value: i64) -> String {
    inches_to_typst(emu_to_inches_i64(value))
}

fn twips_to_inches(value: u32) -> f64 {
    value as f64 / 1_440.0
}

fn emu_to_inches_u64(value: u64) -> f64 {
    value as f64 / 914_400.0
}

fn emu_to_inches_i64(value: i64) -> f64 {
    value as f64 / 914_400.0
}

fn inches_to_typst(value: f64) -> String {
    format!("{}in", trim_float(value as f32))
}

fn trim_float(value: f32) -> String {
    let mut text = format!("{value:.4}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

fn escape_typst_text(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace("//", "\\/\\/")
        .replace('@', "\\@")
        .replace('$', "\\$")
        .replace('#', "\\#")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('<', "\\<")
        .replace('>', "\\>")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

fn escape_typst_string(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}
