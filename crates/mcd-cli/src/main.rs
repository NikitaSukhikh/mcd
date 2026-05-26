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
    /// Add a plain-text annotation to an MCD package.
    AddAnnotation {
        /// Package file to update.
        file: PathBuf,
        /// Annotation body text.
        text: String,
        /// Package path/page the annotation targets, for example content/main.md.
        #[arg(long)]
        page: String,
        /// Optional 1-based line in the target page.
        #[arg(long)]
        line: Option<usize>,
        /// Optional stable annotation id. Generated when omitted.
        #[arg(long)]
        id: Option<String>,
    },
    /// Convert a PDF into a minimal MCD package.
    ConvertPdf {
        /// PDF file to convert.
        file: PathBuf,
        /// Output MCD package path.
        #[arg(long)]
        output: PathBuf,
        /// Optional document title.
        #[arg(long)]
        title: Option<String>,
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
        /// Export a named content type.
        #[arg(long, value_enum)]
        export: Option<ExportMode>,
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
        /// Emit table schemas, keys, relationships, and units.
        #[arg(long)]
        schemas: bool,
        /// Emit image metadata.
        #[arg(long)]
        images: bool,
        /// Emit annotation metadata.
        #[arg(long)]
        annotations: bool,
        /// Emit external data references.
        #[arg(long)]
        external_data: bool,
        /// Emit package-level provenance metadata.
        #[arg(long)]
        provenance: bool,
        /// Filter annotation export by package page/path.
        #[arg(long)]
        page: Option<String>,
        /// Filter annotation export by 1-based source line.
        #[arg(long)]
        line: Option<usize>,
        /// Emit chart metadata and source data.
        #[arg(long)]
        charts: bool,
    },
    /// Query package tables with read-only SQL.
    Query {
        /// Package file to query.
        file: PathBuf,
        /// SQL SELECT query to run against manifest table ids.
        sql: String,
        /// Output format.
        #[arg(long, value_enum, default_value_t = QueryOutputFormat::Table)]
        format: QueryOutputFormat,
    },
    /// Search package content and metadata.
    Search {
        /// Package file to search.
        file: PathBuf,
        /// Search query.
        query: String,
        /// Output format.
        #[arg(long, value_enum, default_value_t = SearchOutputFormat::Text)]
        format: SearchOutputFormat,
        /// Maximum result count.
        #[arg(long, default_value_t = 10)]
        limit: usize,
        /// Filter by indexed content kind.
        #[arg(long, value_enum)]
        kind: Option<SearchKindArg>,
        /// Filter by internal package path/page.
        #[arg(long)]
        page: Option<String>,
    },
    /// Run multiple read-only SQL queries against one loaded package.
    QueryBatch {
        /// Package file to query.
        file: PathBuf,
        /// SQL SELECT query to run. Repeat for multiple result sets.
        #[arg(long = "sql", required = true)]
        sql: Vec<String>,
    },
    /// Show Python and SQL tool capabilities for agents.
    Tools {
        /// Optional package file whose table schemas should be listed.
        file: Option<PathBuf>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = ToolsOutputFormat::Text)]
        format: ToolsOutputFormat,
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
        /// Output rendered file path, or a directory for HTML project output.
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

#[derive(Clone, Debug, ValueEnum)]
enum ExportMode {
    Annotations,
}

#[derive(Clone, Debug, ValueEnum)]
enum QueryOutputFormat {
    Table,
    Json,
    Csv,
}

#[derive(Clone, Debug, ValueEnum)]
enum SearchOutputFormat {
    Text,
    Json,
}

#[derive(Clone, Debug, ValueEnum)]
enum SearchKindArg {
    Markdown,
    Schema,
    Manifest,
    Annotation,
    Provenance,
}

#[derive(Clone, Debug, ValueEnum)]
enum ToolsOutputFormat {
    Text,
    Json,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Inspect { file } => commands::inspect::run(&file),
        Command::AddAnnotation {
            file,
            text,
            page,
            line,
            id,
        } => commands::add_annotation::run(&file, &text, &page, line, id.as_deref()),
        Command::ConvertPdf {
            file,
            output,
            title,
        } => commands::convert_pdf::run(&file, &output, title.as_deref()),
        Command::Validate { file, format } => commands::validate::run(&file, format),
        Command::Extract {
            file,
            export,
            json,
            markdown,
            expand_tables,
            tables,
            schemas,
            images,
            annotations,
            external_data,
            provenance,
            page,
            line,
            charts,
        } => commands::extract::run(
            &file,
            commands::extract::ExtractOptions {
                export: export.map(|mode| match mode {
                    ExportMode::Annotations => commands::extract::ExportMode::Annotations,
                }),
                json,
                markdown,
                expand_tables,
                tables,
                schemas,
                images,
                annotations,
                external_data,
                provenance,
                page: page.as_deref(),
                line,
                charts,
            },
        ),
        Command::Query { file, sql, format } => commands::query::run(
            &file,
            &sql,
            match format {
                QueryOutputFormat::Table => commands::query::OutputFormat::Table,
                QueryOutputFormat::Json => commands::query::OutputFormat::Json,
                QueryOutputFormat::Csv => commands::query::OutputFormat::Csv,
            },
        ),
        Command::Search {
            file,
            query,
            format,
            limit,
            kind,
            page,
        } => commands::search::run(
            &file,
            &query,
            commands::search::SearchCommandOptions {
                format: match format {
                    SearchOutputFormat::Text => commands::search::OutputFormat::Text,
                    SearchOutputFormat::Json => commands::search::OutputFormat::Json,
                },
                limit,
                kind: kind.map(|kind| match kind {
                    SearchKindArg::Markdown => mcd_core::SearchKind::Markdown,
                    SearchKindArg::Schema => mcd_core::SearchKind::Schema,
                    SearchKindArg::Manifest => mcd_core::SearchKind::Manifest,
                    SearchKindArg::Annotation => mcd_core::SearchKind::Annotation,
                    SearchKindArg::Provenance => mcd_core::SearchKind::Provenance,
                }),
                page,
            },
        ),
        Command::QueryBatch { file, sql } => commands::query::run_batch(&file, &sql),
        Command::Tools { file, format } => commands::tools::run(
            file.as_deref(),
            match format {
                ToolsOutputFormat::Text => commands::tools::OutputFormat::Text,
                ToolsOutputFormat::Json => commands::tools::OutputFormat::Json,
            },
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
