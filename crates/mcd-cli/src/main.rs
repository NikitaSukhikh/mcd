//! Command line interface for Markdown CSV Document packages.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

mod commands;

#[derive(Debug, Parser)]
#[command(
    name = "mcd",
    version,
    about = "Markdown CSV Document command line tools"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Inspect an MCD package.
    Inspect {
        /// Package file to inspect.
        file: PathBuf,
    },
    /// Validate an MCD package.
    Validate {
        /// Package file to validate.
        file: PathBuf,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
    /// Extract content from an MCD package.
    Extract {
        /// Package file to extract from.
        file: PathBuf,
        /// Emit canonical JSON.
        #[arg(long)]
        json: bool,
        /// Emit Markdown.
        #[arg(long)]
        markdown: bool,
        /// Expand table directives in Markdown output.
        #[arg(long)]
        expand_tables: bool,
        /// Emit table data.
        #[arg(long)]
        tables: bool,
        /// Emit image metadata.
        #[arg(long)]
        images: bool,
        /// Emit annotation metadata.
        #[arg(long)]
        annotations: bool,
        /// Emit chart metadata and source data.
        #[arg(long)]
        charts: bool,
    },
    /// Render an MCD package.
    Render {
        /// Package file to render.
        file: PathBuf,
        /// Emit standalone HTML.
        #[arg(long)]
        html: bool,
        /// Emit Markdown with package tables embedded as plain Markdown tables.
        #[arg(long)]
        markdown: bool,
        /// Output rendered file path.
        #[arg(long)]
        output: PathBuf,
    },
    /// Pack an unpacked directory into an MCD package.
    Pack {
        /// Unpacked directory.
        directory: PathBuf,
        /// Output package path.
        #[arg(long)]
        output: PathBuf,
    },
    /// Unpack an MCD package into a directory.
    Unpack {
        /// Package file to unpack.
        file: PathBuf,
        /// Output directory.
        #[arg(long)]
        output: PathBuf,
    },
    /// Initialize a minimal unpacked MCD directory.
    Init {
        /// Directory to initialize.
        directory: PathBuf,
    },
}

#[derive(Clone, Debug, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Inspect { file } => commands::inspect::run(&file),
        Command::Validate { file, format } => commands::validate::run(&file, format),
        Command::Extract {
            file,
            json,
            markdown,
            expand_tables,
            tables,
            images,
            annotations,
            charts,
        } => commands::extract::run(
            &file,
            json,
            markdown,
            expand_tables,
            tables,
            images,
            annotations,
            charts,
        ),
        Command::Render {
            file,
            html,
            markdown,
            output,
        } => commands::render::run(&file, html, markdown, &output),
        Command::Pack { directory, output } => commands::pack::run(&directory, &output),
        Command::Unpack { file, output } => commands::unpack::run(&file, &output),
        Command::Init { directory } => commands::init::run(&directory),
    }
}
