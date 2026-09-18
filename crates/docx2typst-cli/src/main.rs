use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use docx2typst::{
    ConversionOptions, ConversionProfile, OutputMode, ValidationLevel, ValidationOptions,
    ValidationReport, convert_path, explain_diagnostic, inspect_path, validate_path,
    validate_result,
};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "docx2typst")]
#[command(about = "Convert DOCX files into Typst bundles or single-file output")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Convert(ConvertArgs),
    Validate(ValidateArgs),
    Inspect(InspectArgs),
    Explain(ExplainArgs),
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ModeArg {
    Bundle,
    SingleFile,
}

impl From<ModeArg> for OutputMode {
    fn from(value: ModeArg) -> Self {
        match value {
            ModeArg::Bundle => OutputMode::Bundle,
            ModeArg::SingleFile => OutputMode::SingleFile,
        }
    }
}

#[derive(Debug, Args)]
struct ConvertArgs {
    input: PathBuf,
    #[arg(long)]
    profile: Option<PathBuf>,
    #[arg(long, value_enum, default_value = "bundle")]
    mode: ModeArg,
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long)]
    pdf_out: Option<PathBuf>,
    #[arg(long)]
    reference: Option<PathBuf>,
    #[arg(long)]
    reference_threshold: Option<u32>,
    #[arg(long)]
    no_validate: bool,
}

#[derive(Debug, Args)]
struct ValidateArgs {
    output: PathBuf,
    #[arg(long)]
    reference: Option<PathBuf>,
    #[arg(long)]
    reference_threshold: Option<u32>,
    #[arg(long)]
    syntax_only: bool,
}

#[derive(Debug, Args)]
struct InspectArgs {
    input: PathBuf,
}

#[derive(Debug, Args)]
struct ExplainArgs {
    diagnostic_code: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Convert(args) => convert(args),
        Command::Validate(args) => validate(args),
        Command::Inspect(args) => inspect(args),
        Command::Explain(args) => explain(args),
    }
}

fn convert(args: ConvertArgs) -> Result<()> {
    if args.no_validate && args.pdf_out.is_some() {
        bail!("--pdf-out requires validation; remove --no-validate to emit a PDF");
    }

    let profile = load_profile(args.profile.as_ref())?;
    let profile_reference_threshold = profile
        .as_ref()
        .and_then(|profile| profile.validation.reference_diff_threshold);
    let output_mode: OutputMode = args.mode.into();
    let output_path = args.out.or_else(|| match output_mode {
        OutputMode::Bundle => Some(PathBuf::from("docx2typst-out")),
        OutputMode::SingleFile => None,
    });

    let options = ConversionOptions {
        profile,
        output_mode,
        reference: args.reference.clone(),
        output_path: output_path.clone(),
        ..ConversionOptions::default()
    };

    let result = convert_path(&args.input, &options)?;

    if args.no_validate {
        if output_path.is_none() {
            println!("{}", result.typst);
        }
        return Ok(());
    }

    let report = validate_result(
        &result,
        &ValidationOptions {
            reference_path: args.reference,
            reference_text_similarity_threshold_percent: args
                .reference_threshold
                .or(profile_reference_threshold)
                .or(ValidationOptions::default().reference_text_similarity_threshold_percent),
            ..ValidationOptions::default()
        },
    )?;
    if let Some(pdf_out) = &args.pdf_out {
        write_pdf(&report, pdf_out)?;
    }

    if output_path.is_none() {
        println!("{}", result.typst);
        eprintln!("{}", report.report_json()?);
    } else if let Some(path) = output_path {
        let report_path = match output_mode {
            OutputMode::Bundle => path.join("validation.json"),
            OutputMode::SingleFile => path.with_extension("validation.json"),
        };
        std::fs::write(&report_path, report.report_json()?).with_context(|| {
            format!(
                "failed to write validation report to {}",
                report_path.display()
            )
        })?;
        println!("{}", path.display());
    }
    Ok(())
}

fn validate(args: ValidateArgs) -> Result<()> {
    let options = ValidationOptions {
        level: if args.syntax_only {
            ValidationLevel::Syntax
        } else {
            ValidationLevel::Compile
        },
        reference_path: args.reference,
        reference_text_similarity_threshold_percent: args
            .reference_threshold
            .or(ValidationOptions::default().reference_text_similarity_threshold_percent),
        ..ValidationOptions::default()
    };
    let report = validate_path(&args.output, &options)?;
    print_report(report)
}

fn inspect(args: InspectArgs) -> Result<()> {
    let report = inspect_path(&args.input)?;
    println!("{}", report.report_json()?);
    Ok(())
}

fn explain(args: ExplainArgs) -> Result<()> {
    println!("{}", explain_diagnostic(&args.diagnostic_code));
    Ok(())
}

fn print_report(report: ValidationReport) -> Result<()> {
    println!("{}", report.report_json()?);
    Ok(())
}

fn write_pdf(report: &ValidationReport, path: &PathBuf) -> Result<()> {
    if !report.compile_ok || report.pdf_bytes.is_empty() {
        bail!("validation did not produce a PDF; check diagnostics and generated Typst output");
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!("failed to create PDF parent directory {}", parent.display())
        })?;
    }
    std::fs::write(path, &report.pdf_bytes)
        .with_context(|| format!("failed to write PDF to {}", path.display()))?;
    Ok(())
}

fn load_profile(path: Option<&PathBuf>) -> Result<Option<ConversionProfile>> {
    let Some(path) = path else {
        return Ok(None);
    };
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read profile {}", path.display()))?;
    Ok(Some(ConversionProfile::from_toml(&raw)?))
}
