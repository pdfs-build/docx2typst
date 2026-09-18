export type OutputMode = 'bundle' | 'single_file'
export type AssetMode = 'auto' | 'extract' | 'inline'
export type WarningPolicy = 'ignore' | 'warn' | 'error'
export type FallbackPolicy = 'omit' | 'preserve_as_asset' | 'error'
export type Severity = 'info' | 'warning' | 'error'
export type ValidationLevel = 'syntax' | 'compile'
export type FontEmbeddingRights =
  | 'unknown'
  | 'installable'
  | 'restricted'
  | 'preview_print'
  | 'editable'
export type FallbackKind = 'drawing' | 'equation'

export interface SourceLocation {
  part: string
  path: string
  range?: [number, number] | null
}

export interface Diagnostic {
  code: string
  severity: Severity
  message: string
  location?: SourceLocation | null
  context?: Record<string, string>
}

export interface FontDecision {
  requested: string
  resolved: string
  reason: string
  location?: SourceLocation | null
}

export interface EmbeddedFontLicense {
  rights?: FontEmbeddingRights
  no_subsetting?: boolean
  bitmap_only?: boolean
}

export interface EmbeddedFont {
  family: string
  variant: string
  original_path: string
  media_type: string
  emitted_path?: string | null
  subsetted?: boolean
  obfuscated?: boolean
  usable?: boolean
  license?: EmbeddedFontLicense
  location?: SourceLocation | null
  bytes?: number[]
}

export interface AssetReference {
  id: string
  original_path: string
  media_type: string
  emitted_path?: string | null
  alt_text?: string | null
  bytes?: number[]
}

export interface FallbackRecord {
  id: string
  kind: FallbackKind
  feature: string
  asset_id: string
  alt_text?: string | null
  width_emu?: number | null
  height_emu?: number | null
  raw_xml?: string | null
  location?: SourceLocation | null
}

export interface ConversionStats {
  parts: number
  assets: number
  paragraphs: number
  tables: number
  sections: number
}

export interface ReferenceValidation {
  reference_path: string
  reference_page_count?: number | null
  compared_page_count?: number | null
  page_count_delta?: number | null
  page_count_match?: boolean
  text_similarity_percent?: number | null
  threshold_percent?: number | null
  within_threshold?: boolean | null
}

export interface ValidationReport {
  level: ValidationLevel
  syntax_ok: boolean
  compile_ok: boolean
  page_count?: number | null
  diagnostics?: Diagnostic[]
  reference?: ReferenceValidation | null
}

export interface InspectionReport {
  package_parts: string[]
  relationships: Record<string, string[]>
  styles: string[]
  fonts: string[]
  embedded_fonts?: string[]
  theme_colors: Record<string, string>
  theme_fonts?: Record<string, string>
  diagnostics?: Diagnostic[]
}

export interface ConversionResult {
  mode: OutputMode
  typst: string
  bundle_root?: string | null
  assets?: AssetReference[]
  embedded_fonts?: EmbeddedFont[]
  fallbacks?: FallbackRecord[]
  diagnostics?: Diagnostic[]
  font_decisions?: FontDecision[]
  font_search_paths?: string[]
  fallback_inventory?: string[]
  stats: ConversionStats
}

export interface StyleMapEntry {
  heading_level?: number | null
  typst_function?: string | null
  block?: boolean
}

export interface FontProfile {
  substitutions?: Record<string, string>
  embedded_paths?: Record<string, string>
}

export interface FallbackProfile {
  drawing?: FallbackPolicy | null
  equation?: FallbackPolicy | null
  unsupported_feature?: FallbackPolicy | null
}

export interface ValidationProfile {
  level?: ValidationLevel | null
  reference_diff_threshold?: number | null
}

export interface ConversionProfile {
  version: number
  paragraph_styles?: Record<string, StyleMapEntry>
  character_styles?: Record<string, StyleMapEntry>
  fonts?: FontProfile
  theme_overrides?: Record<string, string>
  fallback?: FallbackProfile
  warning_threshold?: Severity | null
  validation?: ValidationProfile
  deny_unsupported_features?: string[]
}

export interface ValidationOptions {
  level?: ValidationLevel
  emit_pdf?: boolean
  allow_compile_fallback?: boolean
  reference_path?: string | null
  reference_text_similarity_threshold_percent?: number | null
}

export interface ConversionOptions {
  profile?: ConversionProfile | null
  output_mode?: OutputMode
  asset_mode?: AssetMode
  warning_policy?: WarningPolicy
  fallback_policy?: FallbackPolicy
  validation?: ValidationOptions
  reference?: string | null
  output_path?: string | null
}

export interface ConversionWithValidation {
  conversion: ConversionResult
  validation: ValidationReport
}

export function convertFile(
  path: string,
  options?: ConversionOptions,
): Promise<ConversionResult | ConversionWithValidation>

export function convertBuffer(
  buffer: Buffer,
  options?: ConversionOptions,
): Promise<ConversionResult | ConversionWithValidation>

export function validateOutput(
  path: string,
  options?: ValidationOptions,
): Promise<ValidationReport>

export function inspectFile(path: string): Promise<InspectionReport>
export function inspectBuffer(buffer: Buffer): Promise<InspectionReport>
export function explain(code: string): string
export function runtimeSource(): string
