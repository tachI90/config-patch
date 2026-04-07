use clap::Parser;
use std::path::PathBuf;
use thiserror::Error;
use tracing_subscriber;

mod format;
mod merge;

#[derive(Parser, Debug)]
#[command(name = "config-patch")]
#[command(about = "Deep-merge configuration files (JSON, YAML, TOML) across multiple sources")]
#[command(
    long_about = r#"config-patch merges configuration files in priority order (first -> last).

Each source file is deep-merged into the previous result, with later sources
taking precedence. Arrays of objects are merged by a configurable key field
(default: "name"), preserving unmatched items from earlier sources.

Examples:
  config-patch base.json new.json local.json -o output.json
  config-patch base.yaml new.yaml local.yaml -o output.yaml --array-key id
  config-patch base.toml new.toml local.toml -o output.json --format json
  config-patch a.json b.json c.json d.json -o merged.json --debug"#
)]
struct Cli {
    /// Configuration files to merge (in priority order, first = lowest priority)
    files: Vec<PathBuf>,

    /// Output file path (format auto-detected from extension)
    #[arg(short, long)]
    output: PathBuf,

    /// Key field for smart array merging (default: "name")
    #[arg(long, default_value = "name")]
    array_key: String,

    /// Force output format (overrides file extension detection)
    #[arg(long)]
    format: Option<FormatArg>,

    /// Enable debug logging
    #[arg(long, default_value_t = false)]
    debug: bool,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum FormatArg {
    Json,
    Yaml,
    Toml,
}

#[derive(Error, Debug)]
enum ConfigPatchError {
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Failed to parse {path}: {source}")]
    ParseError {
        path: PathBuf,
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("Unsupported file format: {0}")]
    UnsupportedFormat(String),

    #[error("Failed to write output: {0}")]
    WriteError(#[from] std::io::Error),
}

fn main() {
    let cli = Cli::parse();

    if cli.debug {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .init();
    }

    if let Err(e) = run(&cli) {
        tracing::error!("{e}");
        std::process::exit(1);
    }
}

fn run(cli: &Cli) -> Result<(), ConfigPatchError> {
    if cli.files.is_empty() {
        tracing::error!("At least one input file is required");
        std::process::exit(1);
    }

    if cli.files.len() < 2 {
        tracing::error!("At least two input files are required for merging");
        std::process::exit(1);
    }

    tracing::info!("Merging {} files", cli.files.len());

    let mut values = Vec::new();

    for (i, path) in cli.files.iter().enumerate() {
        tracing::debug!(
            file = %path.display(),
            step = i + 1,
            total = cli.files.len(),
            "Reading file"
        );
        let content = std::fs::read_to_string(path)
            .map_err(|_| ConfigPatchError::FileNotFound(path.clone()))?;
        tracing::debug!(
            file = %path.display(),
            step = i + 1,
            total = cli.files.len(),
            "Parsing file"
        );
        let value = format::parse(&content, path)?;
        values.push(value);
    }

    tracing::debug!(
        count = values.len(),
        array_key = %cli.array_key,
        "Deep-merging values"
    );
    let merged = merge::merge_all(&values, &cli.array_key);

    let output_format = match cli.format {
        Some(f) => match f {
            FormatArg::Json => format::Format::Json,
            FormatArg::Yaml => format::Format::Yaml,
            FormatArg::Toml => format::Format::Toml,
        },
        None => format::detect(&cli.output)?,
    };

    tracing::debug!(format = ?output_format, "Serializing output");
    let output_content = format::serialize(&merged, output_format)?;
    tracing::debug!(output = %cli.output.display(), "Writing output");
    std::fs::write(&cli.output, output_content)?;

    tracing::info!(
        input_count = cli.files.len(),
        output = %cli.output.display(),
        "Merge complete"
    );

    Ok(())
}
