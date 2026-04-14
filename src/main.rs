mod ocr;
mod parser;

use std::path::{Path, PathBuf};

use clap::Parser;
use parser::BanknoteFile;

/// Look up Pick numbers for scanned banknote images
#[derive(Parser)]
#[command(name = "pick-finder")]
struct Cli {
    /// Image file prefix (e.g. /git/numismatic-data/Portugal-00005-1914-)
    prefix: String,

    /// Numista API key
    #[arg(short = 'k', long)]
    api_key: String,

    /// OpenAI API key for vision-based feature extraction
    #[arg(short = 'o', long)]
    openai_key: String,
}

fn find_matching_files(prefix: &str) -> Vec<PathBuf> {
    let prefix_path = Path::new(prefix);

    let dir = prefix_path
        .parent()
        .expect("prefix must include a directory component");

    let stem = prefix_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("cannot read directory")
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.starts_with(&stem))
        })
        .map(|e| e.path())
        .collect();

    files.sort();
    files
}

fn main() {
    let cli = Cli::parse();
    let files = find_matching_files(&cli.prefix);

    let banknotes: Vec<BanknoteFile> = files
        .iter()
        .filter_map(|f| parser::parse_banknote_file(f))
        .collect();

    println!("Parsed {} banknote image(s) from {} file(s):\n", banknotes.len(), files.len());
    for b in &banknotes {
        let side = match b.side {
            parser::Side::Obverse => "front",
            parser::Side::Reverse => "back",
        };
        let variant = b.variant
            .map(|v| format!(" var.{}", v))
            .unwrap_or_default();
        println!(
            "  {} | {} escudos | {}{} | {}",
            b.country, b.denomination, b.year, variant, side
        );
    }

    // Run OCR on obverse (side-A) images only
    let obverse: Vec<&BanknoteFile> = banknotes
        .iter()
        .filter(|b| b.side == parser::Side::Obverse)
        .collect();

    println!("\nRunning vision analysis on {} obverse image(s)...\n", obverse.len());
    for b in &obverse {
        let filename = b.path.file_name().unwrap_or_default().to_string_lossy();
        match ocr::extract_features(&b.path, &cli.openai_key) {
            Some(f) => {
                let denom = f.denomination.as_deref().unwrap_or("?");
                let currency = f.currency.as_deref().unwrap_or("");
                let year = f.year.map(|y| y.to_string()).unwrap_or_else(|| "?".into());
                let portrait = f.portrait.as_deref().unwrap_or("unknown");
                println!("  {filename}");
                println!("    denomination: {denom} {currency}");
                println!("    year:         {year}");
                println!("    portrait:     {portrait}");
                if let Some(sigs) = &f.signatures {
                    println!("    signatures:   {}", sigs.join(", "));
                }
                println!();
            }
            None => {
                println!("  {filename}  [vision extraction failed]");
            }
        }
    }
}
