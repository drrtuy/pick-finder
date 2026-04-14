mod matcher;
mod numista;
mod ocr;
mod parser;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use clap::Parser;
use matcher::BanknoteQuery;
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

    /// OpenAI API key for vision-based feature extraction (optional)
    #[arg(short = 'o', long)]
    openai_key: Option<String>,

    /// Output tab-separated lines: filename<TAB>Pick# (best match only)
    #[arg(long)]
    tsv: bool,
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

    // Step 1: Parse filenames
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
            .map(|v| format!(" var.{v}"))
            .unwrap_or_default();
        println!(
            "  {} | {} | {}{} | {}",
            b.country, b.denomination, b.year, variant, side
        );
    }

    // Deduplicate by (country, denomination, year) — variants are copies of the same banknote
    let mut seen = std::collections::HashSet::new();
    let unique: Vec<&BanknoteFile> = banknotes
        .iter()
        .filter(|b| {
            let key = (b.country.clone(), b.denomination.to_bits(), b.year);
            seen.insert(key)
        })
        .collect();

    // Step 2: Optional vision-based feature extraction (one per unique type, obverse only)
    let mut vision_map: HashMap<(String, u64, u16), ocr::VisionFeatures> = HashMap::new();

    if let Some(ref openai_key) = cli.openai_key {
        let unique_obverse: Vec<&&BanknoteFile> = unique
            .iter()
            .filter(|b| b.side == parser::Side::Obverse)
            .collect();

        println!("\nRunning vision analysis on {} unique obverse image(s)...\n", unique_obverse.len());
        for b in &unique_obverse {
            let filename = b.path.file_name().unwrap_or_default().to_string_lossy();
            match ocr::extract_features(&b.path, openai_key) {
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
                    let key = (b.country.clone(), b.denomination.to_bits(), b.year);
                    vision_map.insert(key, f);
                }
                None => {
                    println!("  {filename}  [vision extraction failed]");
                }
            }
        }
    } else {
        println!("\nSkipping vision analysis (no --openai-key provided)");
    }

    // Step 3–5: Search Numista, extract Pick#, filter & rank
    let client = numista::NumistaClient::new(&cli.api_key);

    if !cli.tsv {
        println!("\nSearching Numista for {} unique banknote type(s)...\n", unique.len());
    }

    // Collect results: (banknote, best pick string)
    let mut results: Vec<(&BanknoteFile, String)> = vec![];

    for b in &unique {
        let key = (b.country.clone(), b.denomination.to_bits(), b.year);
        let vision = vision_map.get(&key);
        let query = BanknoteQuery::from_file(b, vision);

        let matches = matcher::find_matches(&query, &client);

        let best_pick = matches
            .first()
            .and_then(|m| m.pick_numbers.first())
            .cloned()
            .unwrap_or_default();

        results.push((b, best_pick.clone()));

        if !cli.tsv {
            let currency_label = query.currency.as_deref().unwrap_or("(unknown currency)");
            println!(
                "  {} {} {} ({}):",
                b.country, b.denomination, currency_label, b.year
            );

            if matches.is_empty() {
                println!("    No matching types found");
            } else {
                for (i, m) in matches.iter().enumerate() {
                    let year_range = match (m.min_year, m.max_year) {
                        (Some(min), Some(max)) if min == max => format!("{min}"),
                        (Some(min), Some(max)) => format!("{min}-{max}"),
                        (Some(min), None) => format!("{min}-"),
                        _ => "?".to_string(),
                    };
                    let pick_str = if m.pick_numbers.is_empty() {
                        "no Pick#".to_string()
                    } else {
                        m.pick_numbers.join(", ")
                    };
                    let marker = if i == 0 { " ◀ best" } else { "" };
                    println!(
                        "    [{}] {} | years: {} | {} (score: {}){marker}",
                        m.numista_id, m.title, year_range, pick_str, m.score
                    );
                }
            }
            println!();
        }
    }

    // TSV output: one line per unique (country, denomination, year), first obverse filename
    if cli.tsv {
        for (b, pick) in &results {
            let filename = b.path.file_name().unwrap_or_default().to_string_lossy();
            println!("{filename}\t{pick}");
        }
    }
}
