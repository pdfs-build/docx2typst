use docx2typst_model::{
    AssetReference, Block, Bookmark, ConversionProfile, Diagnostic, Document, EdgeInsetsTwips,
    EmbeddedFont, EmbeddedFontLicense, FallbackInline, FallbackKind, FallbackRecord,
    FontEmbeddingRights, HeaderFooter, HeaderFooterKind, Heading, Hyperlink, ImageInline, Inline,
    InspectionReport, ListBlock, ListItem, ListKind, ListMetadata, Note, NoteKind, NoteReference,
    PageSetup, Paragraph, ResolvedStyle, Section, Severity, SourceLocation, StyleMapEntry, Table,
    TableBorderLine, TableBorders, TableCell, TableCellBorders, TableColumn, TableRow, TextRun,
};
use docx2typst_opc::{OpcPackage, Relationships};
use indexmap::IndexMap;
use roxmltree::Node;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub type Result<T> = std::result::Result<T, WmlError>;

#[derive(Debug, thiserror::Error)]
pub enum WmlError {
    #[error(transparent)]
    Opc(#[from] docx2typst_opc::OpcError),
}

#[derive(Debug, Clone, Default)]
struct PartialStyle {
    style_id: Option<String>,
    style_name: Option<String>,
    font_family: Option<String>,
    font_size_half_points: Option<u32>,
    color: Option<String>,
    background_color: Option<String>,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    strike: Option<bool>,
    subscript: Option<bool>,
    superscript: Option<bool>,
    alignment: Option<String>,
    spacing_before_twips: Option<u32>,
    spacing_after_twips: Option<u32>,
    line_spacing_twips: Option<u32>,
    page_break_before: Option<bool>,
    keep_next: Option<bool>,
}

#[derive(Debug, Clone, Default)]
struct StyleDef {
    style_id: String,
    style_name: Option<String>,
    based_on: Option<String>,
    partial: PartialStyle,
}

#[derive(Debug, Clone, Default)]
struct StyleCatalog {
    document_defaults: PartialStyle,
    paragraph_styles: BTreeMap<String, StyleDef>,
    character_styles: BTreeMap<String, StyleDef>,
    table_styles: BTreeMap<String, TableStyleDef>,
}

#[derive(Debug, Clone, Default)]
struct TableStyleDef {
    based_on: Option<String>,
    borders: TableBorders,
    cell_margins: Option<EdgeInsetsTwips>,
    text_style: PartialStyle,
    row_band_size: Option<usize>,
    column_band_size: Option<usize>,
    conditionals: BTreeMap<TableStyleRegion, TableStyleConditional>,
}

#[derive(Debug, Clone, Default)]
struct TableStyleConditional {
    borders: TableBorders,
    cell_borders: TableCellBorders,
    text_style: PartialStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum TableStyleRegion {
    FirstRow,
    LastRow,
    FirstCol,
    LastCol,
    Band1Horz,
    Band2Horz,
    Band1Vert,
    Band2Vert,
}

#[derive(Debug, Clone, Default)]
struct NumberingCatalog {
    nums: BTreeMap<String, String>,
    levels: BTreeMap<(String, u8), (ListKind, String)>,
}

#[derive(Debug, Clone)]
struct ActiveRowSpan {
    start_row: usize,
    start_cell: usize,
    col_span: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct TableConditionalFlags {
    first_row: bool,
    last_row: bool,
    first_col: bool,
    last_col: bool,
    odd_hband: bool,
    even_hband: bool,
    odd_vband: bool,
    even_vband: bool,
}

#[derive(Debug, Clone, Copy)]
struct TableLookFlags {
    first_row: bool,
    last_row: bool,
    first_column: bool,
    last_column: bool,
    no_hband: bool,
    no_vband: bool,
}

impl Default for TableLookFlags {
    fn default() -> Self {
        Self {
            first_row: true,
            last_row: false,
            first_column: false,
            last_column: false,
            no_hband: false,
            no_vband: true,
        }
    }
}

#[derive(Debug)]
struct ParseContext<'a> {
    package: &'a OpcPackage,
    profile: Option<&'a ConversionProfile>,
    styles: StyleCatalog,
    numbering: NumberingCatalog,
    theme_colors: IndexMap<String, String>,
    theme_fonts: IndexMap<String, String>,
    fonts_seen: BTreeSet<String>,
    font_usage_locations: IndexMap<String, SourceLocation>,
    assets: IndexMap<String, AssetReference>,
    fallbacks: Vec<FallbackRecord>,
    diagnostics: Vec<Diagnostic>,
    counters: Counters,
    document_relationships: Relationships,
    note_map: IndexMap<String, Note>,
}

#[derive(Debug, Default)]
struct Counters {
    section: usize,
    paragraph: usize,
    table: usize,
    fallback: usize,
}

#[derive(Debug, Default)]
struct FontCatalog {
    names: Vec<String>,
    embedded_fonts: Vec<EmbeddedFont>,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldInstruction {
    PageNumber,
}

#[derive(Debug, Clone, Default)]
enum FieldState {
    #[default]
    None,
    Collecting(String),
    SuppressingResult,
}

pub fn inspect_package(package: &OpcPackage) -> Result<InspectionReport> {
    let theme_colors = parse_theme_colors(package)?;
    let theme_fonts = parse_theme_fonts(package)?;
    let styles = parse_styles(package, &theme_colors, &theme_fonts)?;
    let font_catalog = parse_font_catalog(package)?;
    let mut relationships = IndexMap::new();
    for part in package.part_names() {
        let rels = package.relationships_for(Some(&part))?;
        if !rels.0.is_empty() {
            relationships.insert(
                part.clone(),
                rels.0
                    .iter()
                    .map(|rel| format!("{} -> {}", rel.id, rel.target))
                    .collect(),
            );
        }
    }
    Ok(InspectionReport {
        package_parts: package.part_names(),
        relationships,
        styles: styles
            .paragraph_styles
            .values()
            .chain(styles.character_styles.values())
            .map(|style| {
                style
                    .style_name
                    .clone()
                    .unwrap_or_else(|| style.style_id.clone())
            })
            .collect(),
        fonts: font_catalog.names.clone(),
        embedded_fonts: font_catalog
            .embedded_fonts
            .iter()
            .map(|font| format!("{} ({})", font.family, font.variant))
            .collect(),
        theme_colors,
        theme_fonts,
        diagnostics: font_catalog.diagnostics,
    })
}

pub fn parse_document(
    package: &OpcPackage,
    profile: Option<&ConversionProfile>,
) -> Result<Document> {
    let theme_colors = parse_theme_colors(package)?;
    let theme_fonts = parse_theme_fonts(package)?;
    let styles = parse_styles(package, &theme_colors, &theme_fonts)?;
    let numbering = parse_numbering(package)?;
    let font_catalog = parse_font_catalog(package)?;
    let note_map = parse_notes(package, &styles, &theme_colors, &theme_fonts, &numbering)?;
    let document_relationships = package.relationships_for(Some("word/document.xml"))?;
    let mut ctx = ParseContext {
        package,
        profile,
        styles,
        numbering,
        theme_colors: theme_colors.clone(),
        theme_fonts: theme_fonts.clone(),
        fonts_seen: font_catalog.names.iter().cloned().collect(),
        font_usage_locations: IndexMap::new(),
        assets: IndexMap::new(),
        fallbacks: Vec::new(),
        diagnostics: font_catalog.diagnostics.clone(),
        counters: Counters::default(),
        document_relationships,
        note_map,
    };

    let xml = package.xml("word/document.xml")?;
    let body = xml
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "body");

    let mut sections = vec![new_section(&mut ctx)];
    let document_relationships = ctx.document_relationships.clone();
    if let Some(body) = body {
        for child in body.children().filter(|node| node.is_element()) {
            match child.tag_name().name() {
                "p" => {
                    let paragraph = parse_paragraph(
                        &mut ctx,
                        child,
                        "word/document.xml",
                        &document_relationships,
                        None,
                    );
                    let has_break = find_child(child, "pPr")
                        .and_then(|props| find_child(props, "sectPr"))
                        .is_some();

                    if let Some(block) = paragraph {
                        sections.last_mut().unwrap().blocks.push(block);
                    }

                    if let Some(sect_pr) =
                        find_child(child, "pPr").and_then(|props| find_child(props, "sectPr"))
                    {
                        apply_section_properties(&mut ctx, sections.last_mut().unwrap(), sect_pr);
                    }

                    if has_break {
                        sections.push(new_section(&mut ctx));
                    }
                }
                "tbl" => {
                    let table = parse_table(
                        &mut ctx,
                        child,
                        "word/document.xml",
                        &document_relationships,
                    );
                    sections
                        .last_mut()
                        .unwrap()
                        .blocks
                        .push(Block::Table(table));
                }
                "sectPr" => {
                    apply_section_properties(&mut ctx, sections.last_mut().unwrap(), child);
                }
                other => {
                    ctx.diagnostics.push(diagnostic(
                        "WML_UNSUPPORTED_BODY_CHILD",
                        Severity::Warning,
                        format!("unsupported body child `{other}`"),
                        Some(location("word/document.xml", child)),
                    ));
                }
            }
        }
    }

    let mut document = Document {
        title: None,
        sections,
        diagnostics: ctx.diagnostics,
        assets: ctx.assets.into_values().collect(),
        notes: ctx.note_map,
        fonts_seen: ctx.fonts_seen.into_iter().collect(),
        font_usage_locations: ctx.font_usage_locations,
        embedded_fonts: font_catalog.embedded_fonts,
        fallbacks: ctx.fallbacks,
        theme_colors,
    };
    normalize_document_lists(&mut document);
    Ok(document)
}

fn parse_notes(
    package: &OpcPackage,
    styles: &StyleCatalog,
    theme_colors: &IndexMap<String, String>,
    theme_fonts: &IndexMap<String, String>,
    numbering: &NumberingCatalog,
) -> Result<IndexMap<String, Note>> {
    let mut notes = IndexMap::new();
    for (part, kind) in [
        ("word/footnotes.xml", NoteKind::Footnote),
        ("word/endnotes.xml", NoteKind::Endnote),
    ] {
        let Ok(xml) = package.xml(part) else {
            continue;
        };
        let rels = package.relationships_for(Some(part))?;
        let mut ctx = ParseContext {
            package,
            profile: None,
            styles: styles.clone(),
            numbering: numbering.clone(),
            theme_colors: theme_colors.clone(),
            theme_fonts: theme_fonts.clone(),
            fonts_seen: BTreeSet::new(),
            font_usage_locations: IndexMap::new(),
            assets: IndexMap::new(),
            fallbacks: Vec::new(),
            diagnostics: Vec::new(),
            counters: Counters::default(),
            document_relationships: rels.clone(),
            note_map: IndexMap::new(),
        };

        for note in xml.descendants().filter(|node| {
            node.is_element() && matches!(node.tag_name().name(), "footnote" | "endnote")
        }) {
            let id = note.attribute(w_name("id")).unwrap_or_default().to_string();
            if id == "-1" || id == "0" {
                continue;
            }
            let mut blocks = Vec::new();
            for child in note.children().filter(|node| node.is_element()) {
                match child.tag_name().name() {
                    "p" => {
                        if let Some(block) = parse_paragraph(&mut ctx, child, part, &rels, None) {
                            blocks.push(block);
                        }
                    }
                    "tbl" => blocks.push(Block::Table(parse_table(&mut ctx, child, part, &rels))),
                    _ => {}
                }
            }
            notes.insert(
                id.clone(),
                Note {
                    id,
                    kind: kind.clone(),
                    blocks,
                },
            );
        }
    }
    Ok(notes)
}

fn apply_section_properties(
    ctx: &mut ParseContext<'_>,
    section: &mut Section,
    sect_pr: Node<'_, '_>,
) {
    section.page_setup = Some(parse_page_setup(sect_pr));
    for kind in ["headerReference", "footerReference"] {
        for child in sect_pr
            .children()
            .filter(|node| node.is_element() && node.tag_name().name() == kind)
        {
            let rel_id = child.attribute(r_name("id")).unwrap_or_default();
            if let Some(rel) = ctx.document_relationships.find(rel_id) {
                let target = ctx
                    .package
                    .resolve_relationship_target("word/document.xml", rel);
                if let Ok(content) = parse_header_footer(ctx, &target, kind) {
                    match kind {
                        "headerReference" => section.header = Some(content),
                        "footerReference" => section.footer = Some(content),
                        _ => {}
                    }
                }
            }
        }
    }
}

fn parse_header_footer(ctx: &mut ParseContext<'_>, part: &str, kind: &str) -> Result<HeaderFooter> {
    let xml = ctx.package.xml(part)?;
    let rels = ctx.package.relationships_for(Some(part))?;
    let mut blocks = Vec::new();
    for child in xml
        .root_element()
        .children()
        .filter(|node| node.is_element())
    {
        match child.tag_name().name() {
            "p" => {
                if let Some(block) = parse_paragraph(ctx, child, part, &rels, None) {
                    blocks.push(block);
                }
            }
            "tbl" => blocks.push(Block::Table(parse_table(ctx, child, part, &rels))),
            _ => {}
        }
    }
    Ok(HeaderFooter {
        kind: if kind == "headerReference" {
            HeaderFooterKind::Header
        } else {
            HeaderFooterKind::Footer
        },
        blocks,
    })
}

fn parse_table(
    ctx: &mut ParseContext<'_>,
    node: Node<'_, '_>,
    part: &str,
    rels: &Relationships,
) -> Table {
    ctx.counters.table += 1;
    let table_props = find_child(node, "tblPr");
    let style_id = table_props
        .and_then(|props| find_child(props, "tblStyle"))
        .and_then(|style| style.attribute(w_name("val")))
        .map(str::to_string);
    let resolved_table_style = style_id
        .as_deref()
        .map(|style_id| resolve_table_style(&ctx.styles, style_id))
        .unwrap_or_default();
    let table_look = parse_table_look(table_props.and_then(|props| find_child(props, "tblLook")));
    let direct_borders = table_props
        .and_then(|props| find_child(props, "tblBorders"))
        .map(|borders| parse_table_borders(borders, &ctx.theme_colors))
        .unwrap_or_default();
    let borders = merged_table_borders(resolved_table_style.borders.clone(), &direct_borders);
    let direct_cell_margins = table_props
        .and_then(|props| find_child(props, "tblCellMar"))
        .map(parse_edge_insets_twips)
        .filter(edge_insets_present);
    let cell_margins = direct_cell_margins
        .clone()
        .or_else(|| resolved_table_style.cell_margins.clone());
    let columns = node
        .children()
        .find(|child| child.is_element() && child.tag_name().name() == "tblGrid")
        .map(|grid| {
            grid.children()
                .filter(|child| child.is_element() && child.tag_name().name() == "gridCol")
                .map(|column| TableColumn {
                    width_twips: column
                        .attribute(w_name("w"))
                        .and_then(|value| value.parse::<u32>().ok()),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let row_nodes = node
        .children()
        .filter(|child| child.is_element() && child.tag_name().name() == "tr")
        .collect::<Vec<_>>();
    let total_rows = row_nodes.len();
    let total_columns = infer_table_column_count(&columns, &row_nodes);
    let first_row_header = table_props
        .and_then(|props| find_child(props, "tblLook"))
        .and_then(|look| look.attribute(w_name("firstRow")))
        .is_some_and(|value| value == "1");
    let mut rows: Vec<TableRow> = Vec::new();
    let mut active_rowspans: Vec<(usize, ActiveRowSpan)> = Vec::new();
    for (row_index, row) in row_nodes.iter().copied().enumerate() {
        let row_props = find_child(row, "trPr");
        let row_conditional = merge_table_conditional_flags(
            row_position_flags(
                table_look,
                row_index,
                total_rows,
                resolved_table_style.row_band_size.unwrap_or(1),
            ),
            parse_table_conditional_flags(
                row_props.and_then(|props| find_child(props, "cnfStyle")),
            ),
        );
        let mut cells = Vec::new();
        let mut next_active_rowspans = Vec::new();
        let mut column_index = 0usize;
        for cell in row
            .children()
            .filter(|child| child.is_element() && child.tag_name().name() == "tc")
        {
            let tc_props = find_child(cell, "tcPr");
            let col_span = tc_props
                .and_then(|props| find_child(props, "gridSpan"))
                .and_then(|grid| grid.attribute(w_name("val")))
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(1);
            let v_merge = tc_props.and_then(|props| find_child(props, "vMerge"));
            let is_vmerge_continue = v_merge.is_some_and(|merge| {
                merge.attribute(w_name("val")).unwrap_or("continue") != "restart"
            });

            if is_vmerge_continue {
                if let Some(active_index) =
                    active_rowspans.iter().position(|(start_col, active)| {
                        *start_col <= column_index && column_index < *start_col + active.col_span
                    })
                {
                    let (start_col, active) = active_rowspans[active_index].clone();
                    if let Some(previous_row) = rows.get_mut(active.start_row) {
                        if let Some(previous_cell) = previous_row.cells.get_mut(active.start_cell) {
                            previous_cell.row_span += 1;
                        }
                    }
                    next_active_rowspans.push((start_col, active.clone()));
                    column_index += active.col_span.max(col_span);
                    continue;
                }
            }

            active_rowspans.retain(|(start_col, active)| {
                let covers =
                    *start_col <= column_index && column_index < *start_col + active.col_span;
                !covers
            });

            let cell_flags = merge_table_conditional_flags(
                row_conditional,
                merge_table_conditional_flags(
                    column_position_flags(
                        table_look,
                        column_index,
                        col_span,
                        total_columns,
                        resolved_table_style.column_band_size.unwrap_or(1),
                    ),
                    parse_table_conditional_flags(
                        tc_props.and_then(|props| find_child(props, "cnfStyle")),
                    ),
                ),
            );

            let mut blocks = Vec::new();
            for child in cell.children().filter(|child| child.is_element()) {
                match child.tag_name().name() {
                    "p" => {
                        if let Some(block) = parse_paragraph(
                            ctx,
                            child,
                            part,
                            rels,
                            Some((&resolved_table_style, cell_flags)),
                        ) {
                            blocks.push(block);
                        }
                    }
                    "tbl" => blocks.push(Block::Table(parse_table(ctx, child, part, rels))),
                    _ => {}
                }
            }
            cells.push(TableCell {
                blocks,
                column_index,
                width_twips: tc_props
                    .and_then(|props| find_child(props, "tcW"))
                    .and_then(parse_width_twips),
                width_pct_fiftieths: tc_props
                    .and_then(|props| find_child(props, "tcW"))
                    .and_then(parse_width_pct_fiftieths),
                margins_twips: tc_props
                    .and_then(|props| find_child(props, "tcMar"))
                    .map(parse_edge_insets_twips)
                    .filter(edge_insets_present),
                background_color: tc_props
                    .and_then(|props| find_child(props, "shd"))
                    .and_then(|node| node.attribute(w_name("fill")))
                    .filter(|fill| *fill != "auto")
                    .map(normalize_hex_color),
                direct_borders: tc_props
                    .and_then(|props| find_child(props, "tcBorders"))
                    .map(|borders| parse_table_cell_borders(borders, &ctx.theme_colors))
                    .unwrap_or_default(),
                borders: resolve_effective_table_cell_borders(
                    &borders,
                    &resolved_table_style,
                    cell_flags,
                    row_index,
                    total_rows,
                    column_index,
                    col_span,
                    total_columns,
                    tc_props
                        .and_then(|props| find_child(props, "tcBorders"))
                        .map(|borders| parse_table_cell_borders(borders, &ctx.theme_colors))
                        .unwrap_or_default(),
                ),
                vertical_align: tc_props
                    .and_then(|props| find_child(props, "vAlign"))
                    .and_then(|node| node.attribute(w_name("val")))
                    .map(str::to_string),
                col_span,
                row_span: 1,
            });

            if v_merge.is_some_and(|merge| merge.attribute(w_name("val")) == Some("restart")) {
                next_active_rowspans.push((
                    column_index,
                    ActiveRowSpan {
                        start_row: row_index,
                        start_cell: cells.len() - 1,
                        col_span,
                    },
                ));
            }
            column_index += col_span;
        }
        rows.push(TableRow {
            height_twips: row_props
                .and_then(|props| find_child(props, "trHeight"))
                .and_then(|height| height.attribute(w_name("val")))
                .and_then(|value| value.parse::<u32>().ok()),
            height_rule: row_props
                .and_then(|props| find_child(props, "trHeight"))
                .and_then(|height| height.attribute(w_name("hRule")))
                .map(str::to_string),
            is_header: row_props
                .and_then(|props| find_child(props, "cnfStyle"))
                .and_then(|style| style.attribute(w_name("firstRow")))
                .is_some_and(|value| value == "1")
                || (first_row_header && row_index == 0),
            cells,
        });
        active_rowspans = next_active_rowspans;
    }
    Table {
        id: format!("table-{}", ctx.counters.table),
        width_twips: table_props
            .and_then(|props| find_child(props, "tblW"))
            .and_then(parse_width_twips),
        width_pct_fiftieths: table_props
            .and_then(|props| find_child(props, "tblW"))
            .and_then(parse_width_pct_fiftieths),
        indent_twips: table_props
            .and_then(|props| find_child(props, "tblInd"))
            .and_then(|indent| indent.attribute(w_name("w")))
            .and_then(|value| value.parse::<i32>().ok()),
        alignment: table_props
            .and_then(|props| find_child(props, "jc"))
            .and_then(|node| node.attribute(w_name("val")))
            .map(str::to_string),
        layout_type: table_props
            .and_then(|props| find_child(props, "tblLayout"))
            .and_then(|node| node.attribute(w_name("type")))
            .map(str::to_string),
        style_id,
        cell_spacing_twips: table_props
            .and_then(|props| find_child(props, "tblCellSpacing"))
            .and_then(parse_width_twips),
        cell_margins,
        direct_borders,
        borders,
        columns,
        rows,
        style: ResolvedStyle::default(),
    }
}

fn infer_table_column_count<'a>(columns: &[TableColumn], rows: &[Node<'a, 'a>]) -> usize {
    if !columns.is_empty() {
        return columns.len();
    }

    rows.iter()
        .map(|row| {
            row.children()
                .filter(|child| child.is_element() && child.tag_name().name() == "tc")
                .map(|cell| {
                    find_child(cell, "tcPr")
                        .and_then(|props| find_child(props, "gridSpan"))
                        .and_then(|grid| grid.attribute(w_name("val")))
                        .and_then(|value| value.parse::<usize>().ok())
                        .unwrap_or(1)
                })
                .sum::<usize>()
        })
        .max()
        .unwrap_or(1)
        .max(1)
}

fn parse_table_look(node: Option<Node<'_, '_>>) -> TableLookFlags {
    let Some(node) = node else {
        return TableLookFlags::default();
    };
    TableLookFlags {
        first_row: node
            .attribute(w_name("firstRow"))
            .map_or(true, |value| parse_table_flag(value, true)),
        last_row: node
            .attribute(w_name("lastRow"))
            .is_some_and(|value| parse_table_flag(value, false)),
        first_column: node
            .attribute(w_name("firstColumn"))
            .is_some_and(|value| parse_table_flag(value, false)),
        last_column: node
            .attribute(w_name("lastColumn"))
            .is_some_and(|value| parse_table_flag(value, false)),
        no_hband: node
            .attribute(w_name("noHBand"))
            .is_some_and(|value| parse_table_flag(value, false)),
        no_vband: node
            .attribute(w_name("noVBand"))
            .map_or(true, |value| parse_table_flag(value, true)),
    }
}

fn parse_table_flag(value: &str, default_when_empty: bool) -> bool {
    if value.is_empty() {
        default_when_empty
    } else {
        parse_table_on_off_value(value)
    }
}

fn parse_table_on_off_value(value: &str) -> bool {
    !matches!(value, "0" | "false" | "off" | "none")
}

fn parse_table_conditional_flags(node: Option<Node<'_, '_>>) -> TableConditionalFlags {
    let Some(node) = node else {
        return TableConditionalFlags::default();
    };
    TableConditionalFlags {
        first_row: node
            .attribute(w_name("firstRow"))
            .is_some_and(parse_table_on_off_value),
        last_row: node
            .attribute(w_name("lastRow"))
            .is_some_and(parse_table_on_off_value),
        first_col: node
            .attribute(w_name("firstColumn"))
            .is_some_and(parse_table_on_off_value),
        last_col: node
            .attribute(w_name("lastColumn"))
            .is_some_and(parse_table_on_off_value),
        odd_hband: node
            .attribute(w_name("oddHBand"))
            .is_some_and(parse_table_on_off_value),
        even_hband: node
            .attribute(w_name("evenHBand"))
            .is_some_and(parse_table_on_off_value),
        odd_vband: node
            .attribute(w_name("oddVBand"))
            .is_some_and(parse_table_on_off_value),
        even_vband: node
            .attribute(w_name("evenVBand"))
            .is_some_and(parse_table_on_off_value),
    }
}

fn row_position_flags(
    look: TableLookFlags,
    row_index: usize,
    total_rows: usize,
    band_size: usize,
) -> TableConditionalFlags {
    let mut flags = TableConditionalFlags::default();
    if total_rows == 0 {
        return flags;
    }

    flags.first_row = look.first_row && row_index == 0;
    flags.last_row = look.last_row && row_index + 1 == total_rows;

    if !look.no_hband {
        let band_index = (row_index / band_size.max(1)) % 2;
        flags.odd_hband = band_index == 0;
        flags.even_hband = band_index == 1;
    }
    flags
}

fn column_position_flags(
    look: TableLookFlags,
    column_index: usize,
    col_span: usize,
    total_columns: usize,
    band_size: usize,
) -> TableConditionalFlags {
    let mut flags = TableConditionalFlags::default();
    if total_columns == 0 {
        return flags;
    }

    flags.first_col = look.first_column && column_index == 0;
    flags.last_col = look.last_column && column_index + col_span >= total_columns;

    if !look.no_vband {
        let band_index = (column_index / band_size.max(1)) % 2;
        flags.odd_vband = band_index == 0;
        flags.even_vband = band_index == 1;
    }
    flags
}

fn merge_table_conditional_flags(
    base: TableConditionalFlags,
    top: TableConditionalFlags,
) -> TableConditionalFlags {
    TableConditionalFlags {
        first_row: base.first_row || top.first_row,
        last_row: base.last_row || top.last_row,
        first_col: base.first_col || top.first_col,
        last_col: base.last_col || top.last_col,
        odd_hband: base.odd_hband || top.odd_hband,
        even_hband: base.even_hband || top.even_hband,
        odd_vband: base.odd_vband || top.odd_vband,
        even_vband: base.even_vband || top.even_vband,
    }
}

fn resolve_table_style(styles: &StyleCatalog, style_id: &str) -> TableStyleDef {
    let mut style = TableStyleDef::default();
    apply_table_style_chain(styles, style_id, &mut style);
    style
}

fn apply_table_style_chain(styles: &StyleCatalog, style_id: &str, out: &mut TableStyleDef) {
    if let Some(style) = styles.table_styles.get(style_id) {
        if let Some(parent) = style.based_on.as_deref() {
            apply_table_style_chain(styles, parent, out);
        }
        merge_table_style(out, style);
    }
}

fn merge_table_style(base: &mut TableStyleDef, top: &TableStyleDef) {
    merge_table_borders_in_place(&mut base.borders, &top.borders);
    if let Some(cell_margins) = &top.cell_margins {
        base.cell_margins = Some(merge_edge_insets(
            base.cell_margins.as_ref(),
            Some(cell_margins),
        ));
    }
    merge_partial(&mut base.text_style, top.text_style.clone());
    if top.row_band_size.is_some() {
        base.row_band_size = top.row_band_size;
    }
    if top.column_band_size.is_some() {
        base.column_band_size = top.column_band_size;
    }

    for (region, conditional) in &top.conditionals {
        let entry = base.conditionals.entry(*region).or_default();
        merge_table_borders_in_place(&mut entry.borders, &conditional.borders);
        merge_table_cell_borders_in_place(&mut entry.cell_borders, &conditional.cell_borders);
        merge_partial(&mut entry.text_style, conditional.text_style.clone());
    }
}

fn merge_edge_insets(
    base: Option<&EdgeInsetsTwips>,
    top: Option<&EdgeInsetsTwips>,
) -> EdgeInsetsTwips {
    EdgeInsetsTwips {
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
    }
}

fn merged_table_borders(mut base: TableBorders, top: &TableBorders) -> TableBorders {
    merge_table_borders_in_place(&mut base, top);
    base
}

fn merge_table_borders_in_place(base: &mut TableBorders, top: &TableBorders) {
    if top.top.is_some() {
        base.top = top.top.clone();
    }
    if top.right.is_some() {
        base.right = top.right.clone();
    }
    if top.bottom.is_some() {
        base.bottom = top.bottom.clone();
    }
    if top.left.is_some() {
        base.left = top.left.clone();
    }
    if top.inside_horizontal.is_some() {
        base.inside_horizontal = top.inside_horizontal.clone();
    }
    if top.inside_vertical.is_some() {
        base.inside_vertical = top.inside_vertical.clone();
    }
}

fn merged_table_cell_borders(
    mut base: TableCellBorders,
    top: &TableCellBorders,
) -> TableCellBorders {
    merge_table_cell_borders_in_place(&mut base, top);
    base
}

fn merge_table_cell_borders_in_place(base: &mut TableCellBorders, top: &TableCellBorders) {
    if top.top.is_some() {
        base.top = top.top.clone();
    }
    if top.right.is_some() {
        base.right = top.right.clone();
    }
    if top.bottom.is_some() {
        base.bottom = top.bottom.clone();
    }
    if top.left.is_some() {
        base.left = top.left.clone();
    }
}

fn resolve_effective_table_cell_borders(
    base_table_borders: &TableBorders,
    table_style: &TableStyleDef,
    flags: TableConditionalFlags,
    row_index: usize,
    total_rows: usize,
    column_index: usize,
    col_span: usize,
    total_columns: usize,
    direct_borders: TableCellBorders,
) -> TableCellBorders {
    let mut effective = table_borders_for_cell(
        base_table_borders,
        row_index,
        total_rows,
        column_index,
        col_span,
        total_columns,
    );

    for region in active_table_style_regions(flags) {
        if let Some(conditional) = table_style.conditionals.get(&region) {
            effective = merged_table_cell_borders(
                effective,
                &table_borders_for_cell(
                    &conditional.borders,
                    row_index,
                    total_rows,
                    column_index,
                    col_span,
                    total_columns,
                ),
            );
            effective = merged_table_cell_borders(effective, &conditional.cell_borders);
        }
    }

    merged_table_cell_borders(effective, &direct_borders)
}

fn resolve_effective_table_text_style(
    table_style: &TableStyleDef,
    flags: TableConditionalFlags,
) -> PartialStyle {
    let mut effective = table_style.text_style.clone();
    for region in active_table_style_regions(flags) {
        if let Some(conditional) = table_style.conditionals.get(&region) {
            merge_partial(&mut effective, conditional.text_style.clone());
        }
    }
    effective
}

fn active_table_style_regions(flags: TableConditionalFlags) -> Vec<TableStyleRegion> {
    let mut regions = Vec::new();
    if flags.odd_hband {
        regions.push(TableStyleRegion::Band1Horz);
    }
    if flags.even_hband {
        regions.push(TableStyleRegion::Band2Horz);
    }
    if flags.odd_vband {
        regions.push(TableStyleRegion::Band1Vert);
    }
    if flags.even_vband {
        regions.push(TableStyleRegion::Band2Vert);
    }
    if flags.first_row {
        regions.push(TableStyleRegion::FirstRow);
    }
    if flags.last_row {
        regions.push(TableStyleRegion::LastRow);
    }
    if flags.first_col {
        regions.push(TableStyleRegion::FirstCol);
    }
    if flags.last_col {
        regions.push(TableStyleRegion::LastCol);
    }
    regions
}

fn table_borders_for_cell(
    borders: &TableBorders,
    row_index: usize,
    total_rows: usize,
    column_index: usize,
    col_span: usize,
    total_columns: usize,
) -> TableCellBorders {
    TableCellBorders {
        top: if row_index == 0 {
            borders.top.clone()
        } else {
            borders.inside_horizontal.clone()
        },
        right: if column_index + col_span >= total_columns.max(1) {
            borders.right.clone()
        } else {
            borders.inside_vertical.clone()
        },
        bottom: if row_index + 1 >= total_rows {
            borders.bottom.clone()
        } else {
            borders.inside_horizontal.clone()
        },
        left: if column_index == 0 {
            borders.left.clone()
        } else {
            borders.inside_vertical.clone()
        },
    }
}

fn parse_width_twips(node: Node<'_, '_>) -> Option<u32> {
    match node.attribute(w_name("type")) {
        Some("auto" | "nil" | "pct") => return None,
        _ => {}
    }
    node.attribute(w_name("w"))
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value > 0)
}

fn parse_width_pct_fiftieths(node: Node<'_, '_>) -> Option<u32> {
    matches!(node.attribute(w_name("type")), Some("pct"))
        .then(|| {
            node.attribute(w_name("w"))
                .and_then(|value| value.parse::<u32>().ok())
                .filter(|value| *value > 0)
        })
        .flatten()
}

fn parse_edge_insets_twips(node: Node<'_, '_>) -> EdgeInsetsTwips {
    EdgeInsetsTwips {
        top: find_child(node, "top")
            .and_then(|side| side.attribute(w_name("w")))
            .and_then(|value| value.parse::<u32>().ok()),
        right: find_child(node, "right")
            .or_else(|| find_child(node, "end"))
            .and_then(|side| side.attribute(w_name("w")))
            .and_then(|value| value.parse::<u32>().ok()),
        bottom: find_child(node, "bottom")
            .and_then(|side| side.attribute(w_name("w")))
            .and_then(|value| value.parse::<u32>().ok()),
        left: find_child(node, "left")
            .or_else(|| find_child(node, "start"))
            .and_then(|side| side.attribute(w_name("w")))
            .and_then(|value| value.parse::<u32>().ok()),
    }
}

fn edge_insets_present(insets: &EdgeInsetsTwips) -> bool {
    insets.top.is_some()
        || insets.right.is_some()
        || insets.bottom.is_some()
        || insets.left.is_some()
}

fn parse_table_borders(
    node: Node<'_, '_>,
    theme_colors: &IndexMap<String, String>,
) -> TableBorders {
    TableBorders {
        top: find_child(node, "top")
            .and_then(|border| parse_table_border_line(border, theme_colors)),
        right: find_child(node, "right")
            .or_else(|| find_child(node, "end"))
            .and_then(|border| parse_table_border_line(border, theme_colors)),
        bottom: find_child(node, "bottom")
            .and_then(|border| parse_table_border_line(border, theme_colors)),
        left: find_child(node, "left")
            .or_else(|| find_child(node, "start"))
            .and_then(|border| parse_table_border_line(border, theme_colors)),
        inside_horizontal: find_child(node, "insideH")
            .and_then(|border| parse_table_border_line(border, theme_colors)),
        inside_vertical: find_child(node, "insideV")
            .and_then(|border| parse_table_border_line(border, theme_colors)),
    }
}

fn parse_table_cell_borders(
    node: Node<'_, '_>,
    theme_colors: &IndexMap<String, String>,
) -> TableCellBorders {
    TableCellBorders {
        top: find_child(node, "top")
            .and_then(|border| parse_table_border_line(border, theme_colors)),
        right: find_child(node, "right")
            .or_else(|| find_child(node, "end"))
            .and_then(|border| parse_table_border_line(border, theme_colors)),
        bottom: find_child(node, "bottom")
            .and_then(|border| parse_table_border_line(border, theme_colors)),
        left: find_child(node, "left")
            .or_else(|| find_child(node, "start"))
            .and_then(|border| parse_table_border_line(border, theme_colors)),
    }
}

fn parse_table_border_line(
    node: Node<'_, '_>,
    theme_colors: &IndexMap<String, String>,
) -> Option<TableBorderLine> {
    let style = node.attribute(w_name("val")).map(str::to_string);
    if style.is_none()
        && node.attribute(w_name("sz")).is_none()
        && node.attribute(w_name("color")).is_none()
        && node.attribute(w_name("themeColor")).is_none()
    {
        return None;
    }

    Some(TableBorderLine {
        width_eighth_points: node
            .attribute(w_name("sz"))
            .and_then(|value| value.parse::<u32>().ok()),
        color: parse_table_border_color(node, theme_colors),
        style,
    })
}

fn parse_table_border_color(
    node: Node<'_, '_>,
    theme_colors: &IndexMap<String, String>,
) -> Option<String> {
    let literal = node
        .attribute(w_name("color"))
        .filter(|value| !matches!(*value, "auto" | "AUTO"))
        .map(normalize_hex_color);

    let Some(theme_name) = node.attribute(w_name("themeColor")) else {
        return literal;
    };
    let Some(base) = resolve_theme_color(theme_colors, theme_name) else {
        return literal;
    };
    let tint = node.attribute(w_name("themeTint")).and_then(parse_hex_u8);
    let shade = node.attribute(w_name("themeShade")).and_then(parse_hex_u8);
    apply_theme_tint_shade(base, tint, shade).or_else(|| Some(base.clone()))
}

fn parse_hex_u8(value: &str) -> Option<u8> {
    u8::from_str_radix(value, 16).ok()
}

fn parse_paragraph(
    ctx: &mut ParseContext<'_>,
    node: Node<'_, '_>,
    part: &str,
    rels: &Relationships,
    table_text_style: Option<(&TableStyleDef, TableConditionalFlags)>,
) -> Option<Block> {
    ctx.counters.paragraph += 1;
    let props = find_child(node, "pPr");
    let mut partial = resolve_paragraph_partial(ctx, props);
    if let Some((table_style, flags)) = table_text_style {
        let paragraph_flags = merge_table_conditional_flags(
            flags,
            parse_table_conditional_flags(props.and_then(|props| find_child(props, "cnfStyle"))),
        );
        merge_partial(
            &mut partial,
            resolve_effective_table_text_style(table_style, paragraph_flags),
        );
    }
    let style = partial_to_resolved(partial.clone());
    if let Some(font) = style.font_family.clone() {
        record_font_usage(ctx, &font, part, node);
    }
    let list = props.and_then(|props| parse_list_metadata(ctx, props));
    let style_id = style.style_id.clone();
    let style_name = style.style_name.clone();

    let mut inlines = Vec::new();
    let mut field_state = FieldState::None;
    parse_inline_children(
        ctx,
        node,
        part,
        rels,
        &style,
        &mut inlines,
        &mut field_state,
    );

    let has_visible_inlines = inlines
        .iter()
        .any(|inline| !matches!(inline, Inline::Bookmark(_)));
    let has_layout = partial.spacing_before_twips.is_some()
        || partial.spacing_after_twips.is_some()
        || partial.line_spacing_twips.is_some()
        || partial.page_break_before.unwrap_or(false)
        || partial.keep_next.unwrap_or(false);
    if !has_visible_inlines && list.is_none() && !has_layout {
        return None;
    }

    if let Some(heading_level) =
        infer_heading_level(ctx.profile, style_id.as_deref(), style_name.as_deref())
    {
        return Some(Block::Heading(Heading {
            id: format!("heading-{}", ctx.counters.paragraph),
            level: heading_level,
            style,
            inlines,
        }));
    }

    Some(Block::Paragraph(Paragraph {
        id: format!("paragraph-{}", ctx.counters.paragraph),
        style,
        inlines,
        list,
        spacing_before_twips: partial.spacing_before_twips,
        spacing_after_twips: partial.spacing_after_twips,
        line_spacing_twips: partial.line_spacing_twips,
        page_break_before: partial.page_break_before.unwrap_or(false),
        keep_next: partial.keep_next.unwrap_or(false),
    }))
}

fn parse_hyperlink(
    ctx: &mut ParseContext<'_>,
    node: Node<'_, '_>,
    part: &str,
    rels: &Relationships,
    paragraph_style: &ResolvedStyle,
) -> Hyperlink {
    let target = if let Some(rel_id) = node.attribute(r_name("id")) {
        rels.find(rel_id)
            .map(|rel| ctx.package.resolve_relationship_target(part, rel))
            .unwrap_or_else(|| rel_id.to_string())
    } else if let Some(anchor) = node.attribute(w_name("anchor")) {
        format!("#{anchor}")
    } else {
        String::new()
    };

    let mut inlines = Vec::new();
    let mut field_state = FieldState::None;
    parse_inline_children(
        ctx,
        node,
        part,
        rels,
        paragraph_style,
        &mut inlines,
        &mut field_state,
    );
    Hyperlink { target, inlines }
}

fn parse_inline_children(
    ctx: &mut ParseContext<'_>,
    node: Node<'_, '_>,
    part: &str,
    rels: &Relationships,
    paragraph_style: &ResolvedStyle,
    output: &mut Vec<Inline>,
    field_state: &mut FieldState,
) {
    for child in node.children().filter(|child| child.is_element()) {
        match child.tag_name().name() {
            "r" => parse_run(ctx, child, part, rels, paragraph_style, output, field_state),
            "hyperlink" => {
                let hyperlink = parse_hyperlink(ctx, child, part, rels, paragraph_style);
                if !hyperlink.target.is_empty() || !hyperlink.inlines.is_empty() {
                    output.push(Inline::Hyperlink(hyperlink));
                }
            }
            "bookmarkStart" => {
                if let Some(name) = child.attribute(w_name("name")) {
                    if !name.starts_with('_') {
                        output.push(Inline::Bookmark(Bookmark {
                            name: name.to_string(),
                        }));
                    }
                }
            }
            "fldSimple" => {
                if let Some(instr) = child.attribute(w_name("instr")) {
                    if let Some(FieldInstruction::PageNumber) = parse_field_instruction(instr) {
                        output.push(Inline::PageNumber(paragraph_style.clone()));
                        continue;
                    }
                }
                parse_inline_children(ctx, child, part, rels, paragraph_style, output, field_state);
            }
            "sdt" => {
                if let Some(content) = find_child(child, "sdtContent") {
                    parse_inline_children(
                        ctx,
                        content,
                        part,
                        rels,
                        paragraph_style,
                        output,
                        field_state,
                    );
                }
            }
            "smartTag" | "customXml" => {
                parse_inline_children(ctx, child, part, rels, paragraph_style, output, field_state);
            }
            "oMath" | "oMathPara" => {
                if let Some(fallback) = register_equation_fallback(ctx, child, part) {
                    output.push(Inline::Fallback(FallbackInline {
                        record_id: fallback.id,
                    }));
                }
            }
            _ => {}
        }
    }
}

fn parse_run(
    ctx: &mut ParseContext<'_>,
    node: Node<'_, '_>,
    part: &str,
    rels: &Relationships,
    paragraph_style: &ResolvedStyle,
    output: &mut Vec<Inline>,
    field_state: &mut FieldState,
) {
    let run_properties = find_child(node, "rPr");
    let mut style = paragraph_style.clone();
    if let Some(style_id) = run_properties
        .and_then(|props| find_child(props, "rStyle"))
        .and_then(|style| style.attribute(w_name("val")))
    {
        style = apply_partial_style(style, resolve_character_partial(ctx, style_id));
    }
    style = apply_partial_style(
        style,
        parse_run_properties(run_properties, &ctx.theme_colors, &ctx.theme_fonts),
    );
    if let Some(font) = style.font_family.clone() {
        record_font_usage(ctx, &font, part, node);
    }

    for child in node.children().filter(|child| child.is_element()) {
        match child.tag_name().name() {
            "t" => {
                if !matches!(
                    field_state,
                    FieldState::SuppressingResult | FieldState::Collecting(_)
                ) {
                    push_text_inline(output, child.text().unwrap_or_default(), style.clone());
                }
            }
            "instrText" => {
                if let FieldState::Collecting(instruction) = field_state {
                    instruction.push_str(child.text().unwrap_or_default());
                }
            }
            "tab" => {
                if !matches!(
                    field_state,
                    FieldState::SuppressingResult | FieldState::Collecting(_)
                ) {
                    push_text_inline(output, "\t", style.clone());
                }
            }
            "br" => {
                if !matches!(
                    field_state,
                    FieldState::SuppressingResult | FieldState::Collecting(_)
                ) {
                    if child.attribute(w_name("type")) == Some("page") {
                        output.push(Inline::PageBreak);
                    } else {
                        output.push(Inline::LineBreak);
                    }
                }
            }
            "cr" => {
                if !matches!(
                    field_state,
                    FieldState::SuppressingResult | FieldState::Collecting(_)
                ) {
                    output.push(Inline::LineBreak);
                }
            }
            "fldChar" => match child.attribute(w_name("fldCharType")) {
                Some("begin") => *field_state = FieldState::Collecting(String::new()),
                Some("separate") => {
                    let previous = std::mem::take(field_state);
                    if let FieldState::Collecting(instruction) = previous {
                        if matches!(
                            parse_field_instruction(&instruction),
                            Some(FieldInstruction::PageNumber)
                        ) {
                            output.push(Inline::PageNumber(style.clone()));
                            *field_state = FieldState::SuppressingResult;
                        }
                    }
                }
                Some("end") => *field_state = FieldState::None,
                _ => {}
            },
            "lastRenderedPageBreak" => {}
            "drawing" => {
                if matches!(
                    field_state,
                    FieldState::SuppressingResult | FieldState::Collecting(_)
                ) {
                    continue;
                }
                if let Some(image) = parse_image(ctx, child, part, rels) {
                    output.push(Inline::Image(image));
                } else if let Some(fallback) = register_drawing_fallback(ctx, child, part) {
                    output.push(Inline::Fallback(FallbackInline {
                        record_id: fallback.id,
                    }));
                }
            }
            "pict" => {
                if matches!(
                    field_state,
                    FieldState::SuppressingResult | FieldState::Collecting(_)
                ) {
                    continue;
                }
                if let Some(image) = parse_vml_image(ctx, child, part, rels) {
                    output.push(Inline::Image(image));
                } else if let Some(fallback) = register_vml_fallback(ctx, child, part) {
                    output.push(Inline::Fallback(FallbackInline {
                        record_id: fallback.id,
                    }));
                }
            }
            "oMath" | "oMathPara" => {
                if matches!(
                    field_state,
                    FieldState::SuppressingResult | FieldState::Collecting(_)
                ) {
                    continue;
                }
                if let Some(fallback) = register_equation_fallback(ctx, child, part) {
                    output.push(Inline::Fallback(FallbackInline {
                        record_id: fallback.id,
                    }));
                }
            }
            "footnoteReference" => {
                if matches!(
                    field_state,
                    FieldState::SuppressingResult | FieldState::Collecting(_)
                ) {
                    continue;
                }
                if let Some(id) = child.attribute(w_name("id")) {
                    output.push(Inline::NoteReference(NoteReference {
                        note_id: id.to_string(),
                        kind: NoteKind::Footnote,
                    }));
                }
            }
            "endnoteReference" => {
                if matches!(
                    field_state,
                    FieldState::SuppressingResult | FieldState::Collecting(_)
                ) {
                    continue;
                }
                if let Some(id) = child.attribute(w_name("id")) {
                    output.push(Inline::NoteReference(NoteReference {
                        note_id: id.to_string(),
                        kind: NoteKind::Endnote,
                    }));
                }
            }
            _ => {}
        }
    }
}

fn parse_field_instruction(instruction: &str) -> Option<FieldInstruction> {
    let normalized = instruction.trim().to_ascii_uppercase();
    if normalized == "PAGE" || normalized.starts_with("PAGE ") {
        Some(FieldInstruction::PageNumber)
    } else {
        None
    }
}

fn parse_image(
    ctx: &mut ParseContext<'_>,
    node: Node<'_, '_>,
    part: &str,
    rels: &Relationships,
) -> Option<ImageInline> {
    let blip = node
        .descendants()
        .find(|child| child.is_element() && child.tag_name().name() == "blip")?;
    let rel_id = blip
        .attribute(r_name("embed"))
        .or_else(|| blip.attribute(r_name("link")))?;
    let rel = rels.find(rel_id)?;
    let target = ctx.package.resolve_relationship_target(part, rel);
    let asset = ensure_asset(ctx, &target, extract_alt_text(node));
    let anchor = node
        .descendants()
        .find(|child| child.is_element() && matches!(child.tag_name().name(), "anchor" | "inline"));
    let extent = anchor
        .and_then(|anchor| find_child(anchor, "extent"))
        .or_else(|| {
            node.descendants()
                .find(|child| child.is_element() && child.tag_name().name() == "extent")
        });
    let offset_x_emu = anchor
        .and_then(|anchor| find_child(anchor, "positionH"))
        .and_then(|position| find_child(position, "posOffset"))
        .and_then(|value| value.text())
        .and_then(|value| value.parse::<i64>().ok());
    let offset_y_emu = anchor
        .and_then(|anchor| find_child(anchor, "positionV"))
        .and_then(|position| find_child(position, "posOffset"))
        .and_then(|value| value.text())
        .and_then(|value| value.parse::<i64>().ok());
    let floating = anchor.is_some_and(|anchor| anchor.tag_name().name() == "anchor");
    let behind_doc = anchor
        .and_then(|anchor| anchor.attribute("behindDoc"))
        .is_some_and(|value| value == "1");
    Some(ImageInline {
        asset_id: asset.id.clone(),
        alt_text: asset.alt_text.clone(),
        width_emu: extent
            .and_then(|value| value.attribute("cx"))
            .and_then(|value| value.parse::<u64>().ok()),
        height_emu: extent
            .and_then(|value| value.attribute("cy"))
            .and_then(|value| value.parse::<u64>().ok()),
        offset_x_emu,
        offset_y_emu,
        floating,
        behind_doc,
    })
}

fn parse_vml_image(
    ctx: &mut ParseContext<'_>,
    node: Node<'_, '_>,
    part: &str,
    rels: &Relationships,
) -> Option<ImageInline> {
    let image_data = node
        .descendants()
        .find(|child| child.is_element() && child.tag_name().name() == "imagedata")?;
    let rel_id = image_data.attribute(r_name("id"))?;
    let rel = rels.find(rel_id)?;
    let target = ctx.package.resolve_relationship_target(part, rel);
    let asset = ensure_asset(ctx, &target, extract_alt_text(node));
    Some(ImageInline {
        asset_id: asset.id.clone(),
        alt_text: asset.alt_text.clone(),
        width_emu: None,
        height_emu: None,
        offset_x_emu: None,
        offset_y_emu: None,
        floating: false,
        behind_doc: false,
    })
}

fn register_drawing_fallback(
    ctx: &mut ParseContext<'_>,
    node: Node<'_, '_>,
    part: &str,
) -> Option<FallbackRecord> {
    let feature = classify_drawing_feature(node)?;
    register_fallback(
        ctx,
        node,
        part,
        FallbackKind::Drawing,
        feature,
        extract_alt_text(node),
        parse_drawing_extent(node),
    )
}

fn register_vml_fallback(
    ctx: &mut ParseContext<'_>,
    node: Node<'_, '_>,
    part: &str,
) -> Option<FallbackRecord> {
    let feature = if node.descendants().any(|child| {
        child.is_element() && matches!(child.tag_name().name(), "textbox" | "txbxContent")
    }) {
        "text_box"
    } else {
        "vml_drawing"
    };
    register_fallback(
        ctx,
        node,
        part,
        FallbackKind::Drawing,
        feature.to_string(),
        extract_alt_text(node),
        None,
    )
}

fn register_equation_fallback(
    ctx: &mut ParseContext<'_>,
    node: Node<'_, '_>,
    part: &str,
) -> Option<FallbackRecord> {
    register_fallback(
        ctx,
        node,
        part,
        FallbackKind::Equation,
        "omml_equation".to_string(),
        None,
        None,
    )
}

fn register_fallback(
    ctx: &mut ParseContext<'_>,
    node: Node<'_, '_>,
    part: &str,
    kind: FallbackKind,
    feature: String,
    alt_text: Option<String>,
    extent: Option<(u64, u64)>,
) -> Option<FallbackRecord> {
    ctx.counters.fallback += 1;
    let id = format!("fallback-{}", ctx.counters.fallback);
    let asset_path = format!("generated/{id}.svg");
    let summary = fallback_summary_text(node, kind, &feature, alt_text.as_deref());
    let (width_emu, height_emu) = extent.unwrap_or_else(|| default_fallback_extent(kind));
    let asset = AssetReference {
        id: format!("asset-fallback-{}", ctx.counters.fallback),
        original_path: asset_path.clone(),
        media_type: "image/svg+xml".to_string(),
        emitted_path: None,
        alt_text: alt_text.clone().or_else(|| Some(summary.clone())),
        bytes: render_fallback_svg(&feature, &summary, width_emu, height_emu),
    };
    ctx.assets.insert(asset_path, asset.clone());
    let fallback = FallbackRecord {
        id: id.clone(),
        kind,
        feature: feature.clone(),
        asset_id: asset.id.clone(),
        alt_text: alt_text.clone().or_else(|| Some(summary.clone())),
        width_emu: Some(width_emu),
        height_emu: Some(height_emu),
        raw_xml: None,
        location: Some(location(part, node)),
    };
    ctx.diagnostics.push(diagnostic(
        "DOCX_FALLBACK_RENDERED_ASSET",
        Severity::Warning,
        format!(
            "rendered fallback asset for unsupported {} `{}`",
            match kind {
                FallbackKind::Drawing => "drawing",
                FallbackKind::Equation => "equation",
            },
            feature
        ),
        fallback.location.clone(),
    ));
    ctx.fallbacks.push(fallback.clone());
    Some(fallback)
}

fn classify_drawing_feature(node: Node<'_, '_>) -> Option<String> {
    let graphic_uri = node
        .descendants()
        .find(|child| child.is_element() && child.tag_name().name() == "graphicData")
        .and_then(|child| child.attribute("uri"));
    if matches!(
        graphic_uri,
        Some("http://schemas.openxmlformats.org/drawingml/2006/chart")
    ) || node.descendants().any(|child| {
        child.is_element() && matches!(child.tag_name().name(), "chart" | "chartSpace")
    }) {
        return Some("chart".to_string());
    }
    if graphic_uri.is_some_and(|uri| uri.contains("diagram"))
        || node.descendants().any(|child| {
            child.is_element() && matches!(child.tag_name().name(), "relIds" | "dataModelExt")
        })
    {
        return Some("smart_art".to_string());
    }
    if node.descendants().any(|child| {
        child.is_element() && matches!(child.tag_name().name(), "txbxContent" | "textbox")
    }) {
        return Some("text_box".to_string());
    }
    if graphic_uri.is_some_and(|uri| uri.contains("picture")) {
        return Some("drawing_picture".to_string());
    }
    Some("drawing".to_string())
}

fn parse_drawing_extent(node: Node<'_, '_>) -> Option<(u64, u64)> {
    let extent = node
        .descendants()
        .find(|child| child.is_element() && child.tag_name().name() == "extent")?;
    let width = extent.attribute("cx")?.parse::<u64>().ok()?;
    let height = extent.attribute("cy")?.parse::<u64>().ok()?;
    Some((width, height))
}

fn default_fallback_extent(kind: FallbackKind) -> (u64, u64) {
    match kind {
        FallbackKind::Drawing => (3_657_600, 1_828_800),
        FallbackKind::Equation => (2_286_000, 685_800),
    }
}

fn fallback_summary_text(
    node: Node<'_, '_>,
    kind: FallbackKind,
    feature: &str,
    alt_text: Option<&str>,
) -> String {
    if let Some(alt_text) = alt_text.filter(|text| !text.trim().is_empty()) {
        return alt_text.trim().to_string();
    }
    let extracted = collect_visible_text(node);
    if !extracted.is_empty() {
        return extracted;
    }
    match kind {
        FallbackKind::Drawing => format!("Unsupported {feature}"),
        FallbackKind::Equation => "Unsupported equation".to_string(),
    }
}

fn collect_visible_text(node: Node<'_, '_>) -> String {
    let mut text = String::new();
    for child in node.descendants().filter(|child| child.is_element()) {
        if matches!(child.tag_name().name(), "t" | "instrText")
            && let Some(value) = child.text().map(str::trim)
            && !value.is_empty()
        {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(value);
        }
    }
    text.chars().take(160).collect::<String>()
}

fn render_fallback_svg(feature: &str, summary: &str, width_emu: u64, height_emu: u64) -> Vec<u8> {
    let width_px = ((width_emu as f64 / 9_525.0).round() as u32).clamp(160, 960);
    let height_px = ((height_emu as f64 / 9_525.0).round() as u32).clamp(48, 480);
    let label = feature.replace('_', " ").to_ascii_uppercase();
    let summary = truncate_svg_text(summary, 96);
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width_px}" height="{height_px}" viewBox="0 0 {width_px} {height_px}">
<rect width="{width_px}" height="{height_px}" rx="8" ry="8" fill="#F7FAFC" stroke="#CBD5E1" stroke-width="2" stroke-dasharray="6 4"/>
<text x="16" y="24" font-family="Arial, sans-serif" font-size="13" font-weight="700" fill="#334155">{}</text>
<text x="16" y="46" font-family="Arial, sans-serif" font-size="12" fill="#475569">{}</text>
</svg>"##,
        escape_xml_text(&label),
        escape_xml_text(&summary),
    );
    svg.into_bytes()
}

fn truncate_svg_text(value: &str, limit: usize) -> String {
    let text = value.trim();
    if text.chars().count() <= limit {
        text.to_string()
    } else {
        let prefix = text
            .chars()
            .take(limit.saturating_sub(1))
            .collect::<String>();
        format!("{prefix}…")
    }
}

fn escape_xml_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn ensure_asset(
    ctx: &mut ParseContext<'_>,
    target: &str,
    alt_text: Option<String>,
) -> AssetReference {
    if let Some(existing) = ctx.assets.get(target) {
        return existing.clone();
    }
    let bytes = ctx.package.part_bytes(target).unwrap_or(&[]).to_vec();
    let media_type = ctx
        .package
        .content_type(target)
        .unwrap_or("application/octet-stream")
        .to_string();
    let asset = AssetReference {
        id: format!("asset-{}", ctx.assets.len() + 1),
        original_path: target.to_string(),
        media_type,
        emitted_path: None,
        alt_text,
        bytes,
    };
    ctx.assets.insert(target.to_string(), asset.clone());
    asset
}

fn parse_page_setup(node: Node<'_, '_>) -> PageSetup {
    let pg_sz = find_child(node, "pgSz");
    let pg_mar = find_child(node, "pgMar");
    PageSetup {
        width_twips: pg_sz
            .and_then(|value| value.attribute(w_name("w")))
            .and_then(|value| value.parse().ok()),
        height_twips: pg_sz
            .and_then(|value| value.attribute(w_name("h")))
            .and_then(|value| value.parse().ok()),
        margin_top_twips: pg_mar
            .and_then(|value| value.attribute(w_name("top")))
            .and_then(|value| value.parse().ok()),
        margin_right_twips: pg_mar
            .and_then(|value| value.attribute(w_name("right")))
            .and_then(|value| value.parse().ok()),
        margin_bottom_twips: pg_mar
            .and_then(|value| value.attribute(w_name("bottom")))
            .and_then(|value| value.parse().ok()),
        margin_left_twips: pg_mar
            .and_then(|value| value.attribute(w_name("left")))
            .and_then(|value| value.parse().ok()),
        header_twips: pg_mar
            .and_then(|value| value.attribute(w_name("header")))
            .and_then(|value| value.parse().ok()),
        footer_twips: pg_mar
            .and_then(|value| value.attribute(w_name("footer")))
            .and_then(|value| value.parse().ok()),
        orientation: pg_sz
            .and_then(|value| value.attribute(w_name("orient")))
            .map(str::to_string),
    }
}

fn parse_list_metadata(ctx: &ParseContext<'_>, props: Node<'_, '_>) -> Option<ListMetadata> {
    let num_pr = find_child(props, "numPr")?;
    let num_id = find_child(num_pr, "numId")?
        .attribute(w_name("val"))?
        .to_string();
    let level = find_child(num_pr, "ilvl")
        .and_then(|node| node.attribute(w_name("val")))
        .and_then(|value| value.parse::<u8>().ok())
        .unwrap_or(0);
    let abstract_num_id = ctx.numbering.nums.get(&num_id)?;
    let (kind, numbering) = ctx
        .numbering
        .levels
        .get(&(abstract_num_id.clone(), level))
        .cloned()
        .unwrap_or((ListKind::Bullet, "bullet".to_string()));
    Some(ListMetadata {
        list_id: Some(num_id),
        kind,
        level,
        numbering: Some(numbering),
    })
}

fn normalize_document_lists(document: &mut Document) {
    let mut next_list_id = 1usize;
    for section in &mut document.sections {
        normalize_block_sequence(&mut section.blocks, &mut next_list_id);
        if let Some(header) = &mut section.header {
            normalize_block_sequence(&mut header.blocks, &mut next_list_id);
        }
        if let Some(footer) = &mut section.footer {
            normalize_block_sequence(&mut footer.blocks, &mut next_list_id);
        }
    }
    for note in document.notes.values_mut() {
        normalize_block_sequence(&mut note.blocks, &mut next_list_id);
    }
}

fn normalize_block_sequence(blocks: &mut Vec<Block>, next_list_id: &mut usize) {
    let original = std::mem::take(blocks);
    let mut normalized = Vec::new();
    let mut index = 0usize;
    while index < original.len() {
        if list_meta(&original[index]).is_some() {
            normalized.push(Block::List(consume_list_block(
                &original,
                &mut index,
                next_list_id,
            )));
            continue;
        }

        let mut block = original[index].clone();
        normalize_nested_block_lists(&mut block, next_list_id);
        normalized.push(block);
        index += 1;
    }
    *blocks = normalized;
}

fn normalize_nested_block_lists(block: &mut Block, next_list_id: &mut usize) {
    match block {
        Block::Table(table) => {
            for row in &mut table.rows {
                for cell in &mut row.cells {
                    normalize_block_sequence(&mut cell.blocks, next_list_id);
                }
            }
        }
        Block::List(list) => {
            for item in &mut list.items {
                normalize_block_sequence(&mut item.blocks, next_list_id);
            }
        }
        Block::Paragraph(_)
        | Block::Heading(_)
        | Block::Image(_)
        | Block::Fallback(_)
        | Block::Unsupported(_) => {}
    }
}

fn consume_list_block(blocks: &[Block], index: &mut usize, next_list_id: &mut usize) -> ListBlock {
    let meta = list_meta(&blocks[*index]).expect("list block must start with a list paragraph");
    let key = list_key(meta);
    let mut list = ListBlock {
        id: format!("list-{}", *next_list_id),
        kind: meta.kind.clone(),
        numbering: meta.numbering.clone(),
        items: Vec::new(),
    };
    *next_list_id += 1;

    while *index < blocks.len() {
        let Some(paragraph) = list_paragraph(&blocks[*index]) else {
            break;
        };
        let Some(paragraph_meta) = paragraph.list.as_ref() else {
            break;
        };
        if paragraph_meta.level != meta.level || list_key(paragraph_meta) != key {
            break;
        }

        let mut item_blocks = vec![Block::Paragraph(Paragraph {
            list: None,
            ..paragraph.clone()
        })];
        *index += 1;

        while *index < blocks.len() {
            if let Some(next_meta) = list_meta(&blocks[*index]) {
                if next_meta.level > meta.level {
                    item_blocks.push(Block::List(consume_list_block(blocks, index, next_list_id)));
                    continue;
                }
                break;
            }

            match &blocks[*index] {
                Block::Table(_) | Block::Image(_) | Block::Fallback(_) | Block::Unsupported(_) => {
                    let mut nested = blocks[*index].clone();
                    normalize_nested_block_lists(&mut nested, next_list_id);
                    item_blocks.push(nested);
                    *index += 1;
                }
                Block::Paragraph(_) | Block::Heading(_) | Block::List(_) => break,
            }
        }

        list.items.push(ListItem {
            blocks: item_blocks,
        });
    }

    list
}

fn list_paragraph(block: &Block) -> Option<&Paragraph> {
    match block {
        Block::Paragraph(paragraph) => Some(paragraph),
        _ => None,
    }
}

fn list_meta(block: &Block) -> Option<&ListMetadata> {
    list_paragraph(block).and_then(|paragraph| paragraph.list.as_ref())
}

fn list_key(meta: &ListMetadata) -> (Option<&str>, &ListKind, Option<&str>) {
    (
        meta.list_id.as_deref(),
        &meta.kind,
        meta.numbering.as_deref(),
    )
}

fn parse_styles(
    package: &OpcPackage,
    theme_colors: &IndexMap<String, String>,
    theme_fonts: &IndexMap<String, String>,
) -> Result<StyleCatalog> {
    let Ok(xml) = package.xml("word/styles.xml") else {
        return Ok(StyleCatalog::default());
    };
    let mut catalog = StyleCatalog::default();

    if let Some(doc_defaults) = xml
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "docDefaults")
    {
        let ppr = doc_defaults
            .descendants()
            .find(|node| node.is_element() && node.tag_name().name() == "pPrDefault")
            .and_then(|node| find_child(node, "pPr"));
        let rpr = doc_defaults
            .descendants()
            .find(|node| node.is_element() && node.tag_name().name() == "rPrDefault")
            .and_then(|node| find_child(node, "rPr"));
        let mut defaults = PartialStyle::default();
        merge_partial(
            &mut defaults,
            parse_paragraph_properties(ppr, theme_colors, theme_fonts),
        );
        merge_partial(
            &mut defaults,
            parse_run_properties(rpr, theme_colors, theme_fonts),
        );
        catalog.document_defaults = defaults;
    }

    for style in xml
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "style")
    {
        let style_id = style
            .attribute(w_name("styleId"))
            .unwrap_or_default()
            .to_string();
        if style_id.is_empty() {
            continue;
        }
        let style_name = find_child(style, "name")
            .and_then(|node| node.attribute(w_name("val")))
            .map(str::to_string);
        let based_on = find_child(style, "basedOn")
            .and_then(|node| node.attribute(w_name("val")))
            .map(str::to_string);

        let mut partial = PartialStyle {
            style_id: Some(style_id.clone()),
            style_name: style_name.clone(),
            ..Default::default()
        };
        merge_partial(
            &mut partial,
            parse_paragraph_properties(find_child(style, "pPr"), theme_colors, theme_fonts),
        );
        merge_partial(
            &mut partial,
            parse_run_properties(find_child(style, "rPr"), theme_colors, theme_fonts),
        );

        let style_def = StyleDef {
            style_id: style_id.clone(),
            style_name,
            based_on,
            partial,
        };

        match style.attribute(w_name("type")) {
            Some("character") => {
                catalog.character_styles.insert(style_id.clone(), style_def);
            }
            Some("table") => {
                catalog.table_styles.insert(
                    style_id.clone(),
                    parse_table_style(style, theme_colors, theme_fonts),
                );
            }
            Some("paragraph") | None => {
                catalog.paragraph_styles.insert(style_id.clone(), style_def);
            }
            _ => {}
        }
    }
    Ok(catalog)
}

fn parse_table_style(
    style: Node<'_, '_>,
    theme_colors: &IndexMap<String, String>,
    theme_fonts: &IndexMap<String, String>,
) -> TableStyleDef {
    let based_on = find_child(style, "basedOn")
        .and_then(|node| node.attribute(w_name("val")))
        .map(str::to_string);
    let cell_margins = find_child(style, "tblPr")
        .and_then(|props| find_child(props, "tblCellMar"))
        .map(parse_edge_insets_twips)
        .filter(edge_insets_present);
    let row_band_size = find_child(style, "tblPr")
        .and_then(|props| find_child(props, "tblStyleRowBandSize"))
        .and_then(|node| node.attribute(w_name("val")))
        .and_then(|value| value.parse::<usize>().ok());
    let column_band_size = find_child(style, "tblPr")
        .and_then(|props| find_child(props, "tblStyleColBandSize"))
        .and_then(|node| node.attribute(w_name("val")))
        .and_then(|value| value.parse::<usize>().ok());
    let borders = find_child(style, "tblPr")
        .and_then(|props| find_child(props, "tblBorders"))
        .map(|node| parse_table_borders(node, theme_colors))
        .unwrap_or_default();
    let mut text_style = PartialStyle::default();
    merge_partial(
        &mut text_style,
        parse_paragraph_properties(find_child(style, "pPr"), theme_colors, theme_fonts),
    );
    merge_partial(
        &mut text_style,
        parse_run_properties(find_child(style, "rPr"), theme_colors, theme_fonts),
    );
    let mut conditionals = BTreeMap::new();
    for conditional in style
        .children()
        .filter(|node| node.is_element() && node.tag_name().name() == "tblStylePr")
    {
        let Some(region) = conditional
            .attribute(w_name("type"))
            .and_then(parse_table_style_region)
        else {
            continue;
        };
        let conditional_style = TableStyleConditional {
            borders: find_child(conditional, "tblPr")
                .and_then(|props| find_child(props, "tblBorders"))
                .map(|node| parse_table_borders(node, theme_colors))
                .unwrap_or_default(),
            cell_borders: find_child(conditional, "tcPr")
                .and_then(|props| find_child(props, "tcBorders"))
                .map(|node| parse_table_cell_borders(node, theme_colors))
                .unwrap_or_default(),
            text_style: {
                let mut partial = PartialStyle::default();
                merge_partial(
                    &mut partial,
                    parse_paragraph_properties(
                        find_child(conditional, "pPr"),
                        theme_colors,
                        theme_fonts,
                    ),
                );
                merge_partial(
                    &mut partial,
                    parse_run_properties(find_child(conditional, "rPr"), theme_colors, theme_fonts),
                );
                partial
            },
        };
        conditionals.insert(region, conditional_style);
    }

    TableStyleDef {
        based_on,
        borders,
        cell_margins,
        text_style,
        row_band_size,
        column_band_size,
        conditionals,
    }
}

fn parse_table_style_region(value: &str) -> Option<TableStyleRegion> {
    match value {
        "firstRow" => Some(TableStyleRegion::FirstRow),
        "lastRow" => Some(TableStyleRegion::LastRow),
        "firstCol" => Some(TableStyleRegion::FirstCol),
        "lastCol" => Some(TableStyleRegion::LastCol),
        "band1Horz" => Some(TableStyleRegion::Band1Horz),
        "band2Horz" => Some(TableStyleRegion::Band2Horz),
        "band1Vert" => Some(TableStyleRegion::Band1Vert),
        "band2Vert" => Some(TableStyleRegion::Band2Vert),
        _ => None,
    }
}

fn parse_theme_colors(package: &OpcPackage) -> Result<IndexMap<String, String>> {
    let path = if package.part("word/theme/theme1.xml").is_some() {
        "word/theme/theme1.xml"
    } else {
        return Ok(IndexMap::new());
    };
    let xml = package.xml(path)?;
    let mut colors = IndexMap::new();
    if let Some(clr_scheme) = xml
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "clrScheme")
    {
        for child in clr_scheme.children().filter(|node| node.is_element()) {
            let key = child.tag_name().name().to_string();
            let value = child
                .children()
                .find(|node| {
                    node.is_element() && matches!(node.tag_name().name(), "srgbClr" | "sysClr")
                })
                .and_then(|node| node.attribute("val").or_else(|| node.attribute("lastClr")))
                .map(str::to_string);
            if let Some(value) = value {
                colors.insert(key, normalize_hex_color(&value));
            }
        }
    }
    Ok(colors)
}

fn parse_theme_fonts(package: &OpcPackage) -> Result<IndexMap<String, String>> {
    let path = if package.part("word/theme/theme1.xml").is_some() {
        "word/theme/theme1.xml"
    } else {
        return Ok(IndexMap::new());
    };
    let xml = package.xml(path)?;
    let mut fonts = IndexMap::new();
    if let Some(font_scheme) = xml
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "fontScheme")
    {
        collect_theme_font_family(&mut fonts, find_child(font_scheme, "majorFont"), "major");
        collect_theme_font_family(&mut fonts, find_child(font_scheme, "minorFont"), "minor");
    }
    Ok(fonts)
}

fn collect_theme_font_family(
    out: &mut IndexMap<String, String>,
    family: Option<Node<'_, '_>>,
    prefix: &str,
) {
    let Some(family) = family else {
        return;
    };
    let latin = find_child(family, "latin")
        .and_then(|node| node.attribute("typeface"))
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let east_asia = find_child(family, "ea")
        .and_then(|node| node.attribute("typeface"))
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| latin.clone());
    let complex_script = find_child(family, "cs")
        .and_then(|node| node.attribute("typeface"))
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| latin.clone());

    if let Some(latin) = latin {
        out.insert(format!("{prefix}Ascii"), latin.clone());
        out.insert(format!("{prefix}HAnsi"), latin);
    }
    if let Some(east_asia) = east_asia {
        out.insert(format!("{prefix}EastAsia"), east_asia);
    }
    if let Some(complex_script) = complex_script {
        out.insert(format!("{prefix}Bidi"), complex_script.clone());
        out.insert(format!("{prefix}Cs"), complex_script);
    }
}

fn parse_font_catalog(package: &OpcPackage) -> Result<FontCatalog> {
    let Ok(xml) = package.xml("word/fontTable.xml") else {
        return Ok(FontCatalog::default());
    };
    let rels = package.relationships_for(Some("word/fontTable.xml"))?;
    let mut fonts = BTreeSet::new();
    let mut embedded_fonts = Vec::new();
    let mut diagnostics = Vec::new();
    for font in xml
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "font")
    {
        if let Some(name) = font.attribute(w_name("name")) {
            fonts.insert(name.to_string());
            for (element_name, variant) in [
                ("embedRegular", "regular"),
                ("embedBold", "bold"),
                ("embedItalic", "italic"),
                ("embedBoldItalic", "bold_italic"),
            ] {
                let Some(embed) = find_child(font, element_name) else {
                    continue;
                };
                let Some(rel_id) = embed.attribute(r_name("id")) else {
                    diagnostics.push(diagnostic(
                        "DOCX_EMBEDDED_FONT_MISSING_RELATIONSHIP",
                        Severity::Warning,
                        format!("embedded font `{name}` ({variant}) is missing a relationship id"),
                        Some(location("word/fontTable.xml", embed)),
                    ));
                    continue;
                };
                let Some(rel) = rels.find(rel_id) else {
                    diagnostics.push(diagnostic(
                        "DOCX_EMBEDDED_FONT_MISSING_RELATIONSHIP",
                        Severity::Warning,
                        format!(
                            "embedded font `{name}` ({variant}) references unknown relationship `{rel_id}`"
                        ),
                        Some(location("word/fontTable.xml", embed)),
                    ));
                    continue;
                };
                let target = package.resolve_relationship_target("word/fontTable.xml", rel);
                let Some(part) = package.part(&target) else {
                    diagnostics.push(diagnostic(
                        "DOCX_EMBEDDED_FONT_MISSING_PART",
                        Severity::Warning,
                        format!(
                            "embedded font `{name}` ({variant}) references missing part `{target}`"
                        ),
                        Some(location("word/fontTable.xml", embed)),
                    ));
                    continue;
                };

                let raw_bytes = part.bytes.clone();
                let font_key = embed.attribute(w_name("fontKey"));
                let obfuscated =
                    is_obfuscated_font_part(&target, part.content_type.as_deref(), font_key);
                let bytes = if obfuscated {
                    match deobfuscate_embedded_font(&raw_bytes, font_key) {
                        Some(bytes) => bytes,
                        None => {
                            diagnostics.push(diagnostic(
                                "DOCX_EMBEDDED_FONT_DEOBFUSCATION_FAILED",
                                Severity::Warning,
                                format!(
                                    "embedded font `{name}` ({variant}) could not be deobfuscated"
                                ),
                                Some(location("word/fontTable.xml", embed)),
                            ));
                            Vec::new()
                        }
                    }
                } else {
                    raw_bytes
                };

                let license = parse_embedded_font_license(&bytes);
                let usable =
                    !matches!(license.rights, FontEmbeddingRights::Restricted) && !bytes.is_empty();
                if matches!(license.rights, FontEmbeddingRights::Restricted) {
                    diagnostics.push(font_license_diagnostic(
                        name,
                        variant,
                        &license,
                        Some(location("word/fontTable.xml", embed)),
                    ));
                }

                embedded_fonts.push(EmbeddedFont {
                    family: name.to_string(),
                    variant: variant.to_string(),
                    original_path: target,
                    media_type: part
                        .content_type
                        .clone()
                        .unwrap_or_else(|| "application/octet-stream".to_string()),
                    emitted_path: None,
                    subsetted: embed
                        .attribute(w_name("subsetted"))
                        .is_some_and(parse_table_on_off_value),
                    obfuscated,
                    usable,
                    license,
                    location: Some(location("word/fontTable.xml", embed)),
                    bytes: if usable { bytes } else { Vec::new() },
                });
            }
        }
    }
    Ok(FontCatalog {
        names: fonts.into_iter().collect(),
        embedded_fonts,
        diagnostics,
    })
}

fn record_font_usage(ctx: &mut ParseContext<'_>, font: &str, part: &str, node: Node<'_, '_>) {
    ctx.fonts_seen.insert(font.to_string());
    if ctx
        .font_usage_locations
        .keys()
        .any(|existing| existing.eq_ignore_ascii_case(font))
    {
        return;
    }
    ctx.font_usage_locations
        .insert(font.to_string(), location(part, node));
}

fn is_obfuscated_font_part(
    target: &str,
    content_type: Option<&str>,
    font_key: Option<&str>,
) -> bool {
    font_key.is_some()
        || content_type.is_some_and(|kind| kind.contains("obfuscated"))
        || Path::new(target)
            .extension()
            .map(|ext| ext.to_string_lossy().into_owned())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("odttf"))
}

fn deobfuscate_embedded_font(bytes: &[u8], font_key: Option<&str>) -> Option<Vec<u8>> {
    let mut output = bytes.to_vec();
    let key = parse_font_key(font_key?)?;
    for (index, byte) in output.iter_mut().take(32).enumerate() {
        *byte ^= key[index % key.len()];
    }
    Some(output)
}

fn parse_font_key(value: &str) -> Option<Vec<u8>> {
    let normalized = value
        .chars()
        .filter(|ch| ch.is_ascii_hexdigit())
        .collect::<String>();
    if normalized.len() != 32 {
        return None;
    }

    let mut bytes = (0..normalized.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&normalized[index..index + 2], 16).ok())
        .collect::<Option<Vec<_>>>()?;
    bytes.reverse();
    Some(bytes)
}

fn parse_embedded_font_license(bytes: &[u8]) -> EmbeddedFontLicense {
    let Some(offset) = find_sfnt_table_offset(bytes, b"OS/2") else {
        return EmbeddedFontLicense::default();
    };
    let Some(fs_type) = read_be_u16(bytes, offset + 8) else {
        return EmbeddedFontLicense::default();
    };

    let rights = if fs_type & 0x0002 != 0 {
        FontEmbeddingRights::Restricted
    } else if fs_type & 0x0008 != 0 {
        FontEmbeddingRights::Editable
    } else if fs_type & 0x0004 != 0 {
        FontEmbeddingRights::PreviewPrint
    } else if fs_type == 0 {
        FontEmbeddingRights::Installable
    } else {
        FontEmbeddingRights::Unknown
    };

    EmbeddedFontLicense {
        rights,
        no_subsetting: fs_type & 0x0100 != 0,
        bitmap_only: fs_type & 0x0200 != 0,
    }
}

fn find_sfnt_table_offset(bytes: &[u8], tag: &[u8; 4]) -> Option<usize> {
    let num_tables = read_be_u16(bytes, 4)? as usize;
    let mut record_offset = 12usize;
    for _ in 0..num_tables {
        let record = bytes.get(record_offset..record_offset + 16)?;
        if record.get(0..4)? == tag {
            let offset = read_be_u32(bytes, record_offset + 8)? as usize;
            let length = read_be_u32(bytes, record_offset + 12)? as usize;
            if offset.checked_add(length)? <= bytes.len() {
                return Some(offset);
            }
            return None;
        }
        record_offset += 16;
    }
    None
}

fn read_be_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let slice = bytes.get(offset..offset + 2)?;
    Some(u16::from_be_bytes([slice[0], slice[1]]))
}

fn read_be_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset + 4)?;
    Some(u32::from_be_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn font_license_diagnostic(
    family: &str,
    variant: &str,
    license: &EmbeddedFontLicense,
    location: Option<SourceLocation>,
) -> Diagnostic {
    let mut context = IndexMap::new();
    context.insert("family".to_string(), family.to_string());
    context.insert("variant".to_string(), variant.to_string());
    context.insert(
        "rights".to_string(),
        match license.rights {
            FontEmbeddingRights::Unknown => "unknown",
            FontEmbeddingRights::Installable => "installable",
            FontEmbeddingRights::Restricted => "restricted",
            FontEmbeddingRights::PreviewPrint => "preview_print",
            FontEmbeddingRights::Editable => "editable",
        }
        .to_string(),
    );
    context.insert(
        "no_subsetting".to_string(),
        license.no_subsetting.to_string(),
    );
    context.insert("bitmap_only".to_string(), license.bitmap_only.to_string());
    Diagnostic {
        code: "DOCX_EMBEDDED_FONT_LICENSE".to_string(),
        severity: Severity::Warning,
        message: format!(
            "embedded font `{family}` ({variant}) is not legally usable for extraction because of its embedding rights"
        ),
        location,
        context,
    }
}

fn parse_numbering(package: &OpcPackage) -> Result<NumberingCatalog> {
    let Ok(xml) = package.xml("word/numbering.xml") else {
        return Ok(NumberingCatalog::default());
    };
    let mut catalog = NumberingCatalog::default();

    for num in xml
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "num")
    {
        let Some(num_id) = num.attribute(w_name("numId")) else {
            continue;
        };
        let abstract_num_id = find_child(num, "abstractNumId")
            .and_then(|node| node.attribute(w_name("val")))
            .unwrap_or_default()
            .to_string();
        catalog.nums.insert(num_id.to_string(), abstract_num_id);
    }

    for abstract_num in xml
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "abstractNum")
    {
        let Some(abstract_num_id) = abstract_num.attribute(w_name("abstractNumId")) else {
            continue;
        };
        for lvl in abstract_num
            .children()
            .filter(|node| node.is_element() && node.tag_name().name() == "lvl")
        {
            let level = lvl
                .attribute(w_name("ilvl"))
                .and_then(|value| value.parse::<u8>().ok())
                .unwrap_or(0);
            let num_fmt = find_child(lvl, "numFmt")
                .and_then(|node| node.attribute(w_name("val")))
                .unwrap_or("bullet");
            let kind = if num_fmt == "bullet" {
                ListKind::Bullet
            } else {
                ListKind::Numbered
            };
            catalog.levels.insert(
                (abstract_num_id.to_string(), level),
                (kind, num_fmt.to_string()),
            );
        }
    }
    Ok(catalog)
}

fn resolve_paragraph_partial(
    ctx: &mut ParseContext<'_>,
    props: Option<Node<'_, '_>>,
) -> PartialStyle {
    let mut partial = ctx.styles.document_defaults.clone();
    let style_id = props
        .and_then(|p| find_child(p, "pStyle"))
        .and_then(|node| node.attribute(w_name("val")))
        .map(str::to_string);
    if let Some(style_id) = style_id.as_deref() {
        apply_style_chain(&ctx.styles.paragraph_styles, style_id, &mut partial);
    }
    merge_partial(
        &mut partial,
        parse_paragraph_properties(props, &ctx.theme_colors, &ctx.theme_fonts),
    );
    let style = partial_to_resolved(partial.clone());
    if let Some(font) = style.font_family {
        ctx.fonts_seen.insert(font);
    }
    partial
}

fn resolve_character_partial(ctx: &ParseContext<'_>, style_id: &str) -> PartialStyle {
    let mut partial = PartialStyle::default();
    apply_style_chain(&ctx.styles.character_styles, style_id, &mut partial);
    partial
}

fn apply_style_chain(styles: &BTreeMap<String, StyleDef>, style_id: &str, out: &mut PartialStyle) {
    if let Some(style) = styles.get(style_id) {
        if let Some(parent) = style.based_on.as_deref() {
            apply_style_chain(styles, parent, out);
        }
        merge_partial(out, style.partial.clone());
    }
}

fn parse_paragraph_properties(
    props: Option<Node<'_, '_>>,
    theme_colors: &IndexMap<String, String>,
    theme_fonts: &IndexMap<String, String>,
) -> PartialStyle {
    let mut style = PartialStyle::default();
    let Some(props) = props else {
        return style;
    };
    style.alignment = find_child(props, "jc")
        .and_then(|node| node.attribute(w_name("val")))
        .map(str::to_string);
    if let Some(spacing) = find_child(props, "spacing") {
        style.spacing_before_twips = spacing
            .attribute(w_name("before"))
            .and_then(|value| value.parse::<u32>().ok());
        style.spacing_after_twips = spacing
            .attribute(w_name("after"))
            .and_then(|value| value.parse::<u32>().ok());
        style.line_spacing_twips = spacing
            .attribute(w_name("line"))
            .and_then(|value| value.parse::<u32>().ok());
    }
    style.page_break_before = find_child(props, "pageBreakBefore").map(parse_on_off_value);
    style.keep_next = find_child(props, "keepNext").map(parse_on_off_value);
    if let Some(shd) = find_child(props, "shd")
        .and_then(|node| node.attribute(w_name("fill")))
        .filter(|fill| *fill != "auto")
    {
        style.background_color = Some(normalize_hex_color(shd));
    }
    merge_partial(
        &mut style,
        parse_run_properties(find_child(props, "rPr"), theme_colors, theme_fonts),
    );
    style
}

fn parse_run_properties(
    props: Option<Node<'_, '_>>,
    theme_colors: &IndexMap<String, String>,
    theme_fonts: &IndexMap<String, String>,
) -> PartialStyle {
    let mut style = PartialStyle::default();
    let Some(props) = props else {
        return style;
    };
    style.bold = find_child(props, "b").map(parse_on_off_value);
    style.italic = find_child(props, "i").map(parse_on_off_value);
    style.strike = find_child(props, "strike").map(parse_on_off_value);
    style.underline = find_child(props, "u").map(|node| {
        !matches!(
            node.attribute(w_name("val")),
            Some("none" | "0" | "false" | "off")
        )
    });
    style.font_family =
        find_child(props, "rFonts").and_then(|node| resolve_run_font_family(node, theme_fonts));
    style.font_size_half_points = find_child(props, "sz")
        .and_then(|node| node.attribute(w_name("val")))
        .and_then(|value| value.parse::<u32>().ok());
    style.color = find_child(props, "color").and_then(|node| {
        node.attribute(w_name("themeColor"))
            .and_then(|theme| resolve_theme_color(theme_colors, theme))
            .cloned()
            .or_else(|| node.attribute(w_name("val")).map(normalize_hex_color))
    });
    style.background_color = find_child(props, "highlight")
        .and_then(|node| node.attribute(w_name("val")))
        .and_then(named_highlight_to_hex);
    match find_child(props, "vertAlign").and_then(|node| node.attribute(w_name("val"))) {
        Some("superscript") => style.superscript = Some(true),
        Some("subscript") => style.subscript = Some(true),
        _ => {}
    }
    style
}

fn resolve_run_font_family(
    fonts: Node<'_, '_>,
    theme_fonts: &IndexMap<String, String>,
) -> Option<String> {
    fonts
        .attribute(w_name("ascii"))
        .or_else(|| fonts.attribute(w_name("hAnsi")))
        .or_else(|| fonts.attribute(w_name("eastAsia")))
        .or_else(|| fonts.attribute(w_name("cs")))
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            fonts
                .attribute(w_name("asciiTheme"))
                .or_else(|| fonts.attribute(w_name("hAnsiTheme")))
                .or_else(|| fonts.attribute(w_name("eastAsiaTheme")))
                .or_else(|| fonts.attribute(w_name("csTheme")))
                .or_else(|| fonts.attribute(w_name("cstheme")))
                .and_then(|theme| resolve_theme_font(theme_fonts, theme))
                .cloned()
        })
}

fn partial_to_resolved(partial: PartialStyle) -> ResolvedStyle {
    ResolvedStyle {
        style_id: partial.style_id,
        style_name: partial.style_name,
        font_family: partial.font_family,
        font_size_half_points: partial.font_size_half_points,
        color: partial.color,
        background_color: partial.background_color,
        bold: partial.bold.unwrap_or(false),
        italic: partial.italic.unwrap_or(false),
        underline: partial.underline.unwrap_or(false),
        strike: partial.strike.unwrap_or(false),
        subscript: partial.subscript.unwrap_or(false),
        superscript: partial.superscript.unwrap_or(false),
        alignment: partial.alignment,
    }
}

fn apply_partial_style(mut base: ResolvedStyle, top: PartialStyle) -> ResolvedStyle {
    if top.style_id.is_some() {
        base.style_id = top.style_id;
    }
    if top.style_name.is_some() {
        base.style_name = top.style_name;
    }
    if top.font_family.is_some() {
        base.font_family = top.font_family;
    }
    if top.font_size_half_points.is_some() {
        base.font_size_half_points = top.font_size_half_points;
    }
    if top.color.is_some() {
        base.color = top.color;
    }
    if top.background_color.is_some() {
        base.background_color = top.background_color;
    }
    if top.alignment.is_some() {
        base.alignment = top.alignment;
    }
    if let Some(value) = top.bold {
        base.bold = value;
    }
    if let Some(value) = top.italic {
        base.italic = value;
    }
    if let Some(value) = top.underline {
        base.underline = value;
    }
    if let Some(value) = top.strike {
        base.strike = value;
    }
    if let Some(value) = top.subscript {
        base.subscript = value;
    }
    if let Some(value) = top.superscript {
        base.superscript = value;
    }
    base
}

fn merge_partial(base: &mut PartialStyle, top: PartialStyle) {
    if top.style_id.is_some() {
        base.style_id = top.style_id;
    }
    if top.style_name.is_some() {
        base.style_name = top.style_name;
    }
    if top.font_family.is_some() {
        base.font_family = top.font_family;
    }
    if top.font_size_half_points.is_some() {
        base.font_size_half_points = top.font_size_half_points;
    }
    if top.color.is_some() {
        base.color = top.color;
    }
    if top.background_color.is_some() {
        base.background_color = top.background_color;
    }
    if top.bold.is_some() {
        base.bold = top.bold;
    }
    if top.italic.is_some() {
        base.italic = top.italic;
    }
    if top.underline.is_some() {
        base.underline = top.underline;
    }
    if top.strike.is_some() {
        base.strike = top.strike;
    }
    if top.subscript.is_some() {
        base.subscript = top.subscript;
    }
    if top.superscript.is_some() {
        base.superscript = top.superscript;
    }
    if top.alignment.is_some() {
        base.alignment = top.alignment;
    }
    if top.spacing_before_twips.is_some() {
        base.spacing_before_twips = top.spacing_before_twips;
    }
    if top.spacing_after_twips.is_some() {
        base.spacing_after_twips = top.spacing_after_twips;
    }
    if top.line_spacing_twips.is_some() {
        base.line_spacing_twips = top.line_spacing_twips;
    }
    if top.page_break_before.is_some() {
        base.page_break_before = top.page_break_before;
    }
    if top.keep_next.is_some() {
        base.keep_next = top.keep_next;
    }
}

fn parse_on_off_value(node: Node<'_, '_>) -> bool {
    !matches!(
        node.attribute(w_name("val")),
        Some("0" | "false" | "off" | "none")
    )
}

fn push_text_inline(output: &mut Vec<Inline>, text: &str, style: ResolvedStyle) {
    if text.is_empty() {
        return;
    }
    if let Some(Inline::Text(existing)) = output.last_mut() {
        if existing.style == style {
            existing.text.push_str(text);
            return;
        }
    }
    output.push(Inline::Text(TextRun {
        text: text.to_string(),
        style,
    }));
}

fn infer_heading_level(
    profile: Option<&ConversionProfile>,
    style_id: Option<&str>,
    style_name: Option<&str>,
) -> Option<u8> {
    let profile_level = profile.and_then(|profile| {
        let entry = style_id
            .and_then(|id| profile.paragraph_styles.get(id))
            .or_else(|| style_name.and_then(|name| profile.paragraph_styles.get(name)));
        entry.and_then(|entry: &StyleMapEntry| entry.heading_level)
    });
    if profile_level.is_some() {
        return profile_level;
    }

    let candidate = style_name.or(style_id)?;
    let lower = candidate.to_ascii_lowercase();
    if let Some(suffix) = lower.strip_prefix("heading ") {
        return suffix.parse::<u8>().ok();
    }
    if let Some(suffix) = lower.strip_prefix("heading") {
        return suffix.parse::<u8>().ok();
    }
    None
}

fn find_child<'a>(node: Node<'a, 'a>, local_name: &str) -> Option<Node<'a, 'a>> {
    node.children()
        .find(|child| child.is_element() && child.tag_name().name() == local_name)
}

fn extract_alt_text(node: Node<'_, '_>) -> Option<String> {
    node.descendants()
        .find(|child| child.is_element() && child.tag_name().name() == "docPr")
        .and_then(|child| {
            child
                .attribute("descr")
                .or_else(|| child.attribute("title"))
                .map(str::to_string)
        })
}

fn new_section(ctx: &mut ParseContext<'_>) -> Section {
    ctx.counters.section += 1;
    Section {
        id: format!("section-{}", ctx.counters.section),
        ..Section::default()
    }
}

fn w_name(name: &'static str) -> (&'static str, &'static str) {
    (
        "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
        name,
    )
}

fn r_name(name: &'static str) -> (&'static str, &'static str) {
    (
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships",
        name,
    )
}

fn normalize_hex_color(input: &str) -> String {
    let trimmed = input.trim().trim_start_matches('#');
    format!("#{}", trimmed.to_ascii_uppercase())
}

fn resolve_theme_color<'a>(
    theme_colors: &'a IndexMap<String, String>,
    name: &str,
) -> Option<&'a String> {
    let key = match name {
        "background1" => "lt1",
        "text1" => "dk1",
        "background2" => "lt2",
        "text2" => "dk2",
        "hyperlink" => "hlink",
        "followedHyperlink" => "folHlink",
        _ => name,
    };
    theme_colors.get(key)
}

fn resolve_theme_font<'a>(
    theme_fonts: &'a IndexMap<String, String>,
    name: &str,
) -> Option<&'a String> {
    let key = match name {
        "majorAscii" | "majorHAnsi" | "majorEastAsia" | "majorBidi" | "majorCs" => name,
        "minorAscii" | "minorHAnsi" | "minorEastAsia" | "minorBidi" | "minorCs" => name,
        "major" => "majorHAnsi",
        "minor" => "minorHAnsi",
        _ => name,
    };
    theme_fonts.get(key)
}

fn apply_theme_tint_shade(color: &str, tint: Option<u8>, shade: Option<u8>) -> Option<String> {
    let (red, green, blue) = parse_hex_rgb(color)?;
    let (hue, saturation, mut lightness) = rgb_to_hsl(red, green, blue);

    if let Some(shade) = shade {
        lightness *= shade as f32 / 255.0;
    }
    if let Some(tint) = tint {
        let tint = tint as f32 / 255.0;
        lightness = lightness * tint + (1.0 - tint);
    }

    let (red, green, blue) = hsl_to_rgb(hue, saturation, lightness.clamp(0.0, 1.0));
    Some(format!(
        "#{:02X}{:02X}{:02X}",
        quantize_rgb(red),
        quantize_rgb(green),
        quantize_rgb(blue)
    ))
}

fn parse_hex_rgb(color: &str) -> Option<(u8, u8, u8)> {
    let normalized = color.trim().trim_start_matches('#');
    if normalized.len() != 6 {
        return None;
    }

    Some((
        u8::from_str_radix(&normalized[0..2], 16).ok()?,
        u8::from_str_radix(&normalized[2..4], 16).ok()?,
        u8::from_str_radix(&normalized[4..6], 16).ok()?,
    ))
}

fn rgb_to_hsl(red: u8, green: u8, blue: u8) -> (f32, f32, f32) {
    let red = red as f32 / 255.0;
    let green = green as f32 / 255.0;
    let blue = blue as f32 / 255.0;
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let delta = max - min;
    let lightness = (max + min) / 2.0;

    if delta == 0.0 {
        return (0.0, 0.0, lightness);
    }

    let saturation = delta / (1.0 - (2.0 * lightness - 1.0).abs());
    let hue = if max == red {
        ((green - blue) / delta).rem_euclid(6.0)
    } else if max == green {
        ((blue - red) / delta) + 2.0
    } else {
        ((red - green) / delta) + 4.0
    } / 6.0;

    (hue, saturation, lightness)
}

fn hsl_to_rgb(hue: f32, saturation: f32, lightness: f32) -> (f32, f32, f32) {
    if saturation == 0.0 {
        return (lightness, lightness, lightness);
    }

    let q = if lightness < 0.5 {
        lightness * (1.0 + saturation)
    } else {
        lightness + saturation - lightness * saturation
    };
    let p = 2.0 * lightness - q;

    (
        hue_to_rgb(p, q, hue + 1.0 / 3.0),
        hue_to_rgb(p, q, hue),
        hue_to_rgb(p, q, hue - 1.0 / 3.0),
    )
}

fn hue_to_rgb(p: f32, q: f32, mut value: f32) -> f32 {
    if value < 0.0 {
        value += 1.0;
    }
    if value > 1.0 {
        value -= 1.0;
    }
    if value < 1.0 / 6.0 {
        p + (q - p) * 6.0 * value
    } else if value < 0.5 {
        q
    } else if value < 2.0 / 3.0 {
        p + (q - p) * (2.0 / 3.0 - value) * 6.0
    } else {
        p
    }
}

fn quantize_rgb(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).floor() as u8
}

fn named_highlight_to_hex(name: &str) -> Option<String> {
    let color = match name {
        "yellow" => "#FFFF00",
        "green" => "#00FF00",
        "cyan" => "#00FFFF",
        "magenta" => "#FF00FF",
        "blue" => "#0000FF",
        "red" => "#FF0000",
        "darkBlue" => "#00008B",
        "darkCyan" => "#008B8B",
        "darkGreen" => "#006400",
        "darkMagenta" => "#8B008B",
        "darkRed" => "#8B0000",
        "darkYellow" => "#B8860B",
        "darkGray" => "#A9A9A9",
        "lightGray" => "#D3D3D3",
        "black" => "#000000",
        "none" => return None,
        _ => return None,
    };
    Some(color.to_string())
}

fn location(part: &str, node: Node<'_, '_>) -> SourceLocation {
    let mut path_segments = Vec::new();
    let mut current = Some(node);
    while let Some(value) = current {
        if value.is_element() {
            let index = value
                .prev_siblings()
                .filter(|sibling| {
                    sibling.is_element() && sibling.tag_name().name() == value.tag_name().name()
                })
                .count()
                + 1;
            path_segments.push(format!("{}[{index}]", value.tag_name().name()));
        }
        current = value.parent();
    }
    path_segments.reverse();
    SourceLocation {
        part: part.to_string(),
        path: format!("/{}", path_segments.join("/")),
        range: Some((node.range().start, node.range().end)),
    }
}

fn diagnostic(
    code: &str,
    severity: Severity,
    message: String,
    location: Option<SourceLocation>,
) -> Diagnostic {
    Diagnostic {
        code: code.to_string(),
        severity,
        message,
        location,
        context: IndexMap::new(),
    }
}
