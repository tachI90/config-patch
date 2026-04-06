use clap::Parser;
use std::path::PathBuf;
use thiserror::Error;

mod format;
mod merge;

#[derive(Parser, Debug)]
#[command(name = "config-patch")]
#[command(about = "Deep-merge configuration files (JSON, YAML, TOML) across multiple sources")]
#[command(long_about = r#"config-patch merges configuration files in priority order: Base -> New -> Local.

Each source file is deep-merged into the previous result, with later sources
taking precedence. Arrays of objects are merged by a configurable key field
(default: "name"), preserving unmatched items from earlier sources.

Examples:
  config-patch base.json new.json local.json -o output.json
  config-patch base.yaml new.yaml local.yaml -o output.yaml --array-key id
  config-patch base.toml new.toml local.toml -o output.json --format json"#)]
struct Cli {
    /// Base configuration file (lowest priority)
    base: PathBuf,

    /// New version configuration file (medium priority)
    new: PathBuf,

    /// Local overrides configuration file (highest priority)
    local: PathBuf,

    /// Output file path (format auto-detected from extension)
    #[arg(short, long)]
    output: PathBuf,

    /// Key field for smart array merging (default: "name")
    #[arg(long, default_value = "name")]
    array_key: String,

    /// Force output format (overrides file extension detection)
    #[arg(long)]
    format: Option<FormatArg>,
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

    if let Err(e) = run(&cli) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn run(cli: &Cli) -> Result<(), ConfigPatchError> {
    let base_content = std::fs::read_to_string(&cli.base)
        .map_err(|_| ConfigPatchError::FileNotFound(cli.base.clone()))?;
    let new_content = std::fs::read_to_string(&cli.new)
        .map_err(|_| ConfigPatchError::FileNotFound(cli.new.clone()))?;
    let local_content = std::fs::read_to_string(&cli.local)
        .map_err(|_| ConfigPatchError::FileNotFound(cli.local.clone()))?;

    let base_value = format::parse(&base_content, &cli.base)?;
    let new_value = format::parse(&new_content, &cli.new)?;
    let local_value = format::parse(&local_content, &cli.local)?;

    let merged = merge::merge_all(&[base_value, new_value, local_value], &cli.array_key);

    let output_format = match cli.format {
        Some(f) => match f {
            FormatArg::Json => format::Format::Json,
            FormatArg::Yaml => format::Format::Yaml,
            FormatArg::Toml => format::Format::Toml,
        },
        None => format::detect(&cli.output)?,
    };

    let output_content = format::serialize(&merged, output_format)?;
    std::fs::write(&cli.output, output_content)?;

    Ok(())
}
