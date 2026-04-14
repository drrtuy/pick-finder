# pick-finder

A command-line tool that identifies **Pick catalog numbers** for scanned banknote images. It parses structured filenames, optionally analyzes images with OpenAI GPT-4o Vision, and queries the [Numista API](https://en.numista.com/api/doc/index.php) to find the best matching catalog entry.

## Features

- **Filename-based feature extraction** — country, denomination, year, variant, side parsed from image filenames
- **OpenAI Vision analysis** *(optional)* — extracts currency name, portrait identity, and signatures from obverse images
- **Numista catalog search** — queries 230+ country issuers with denomination + year filtering
- **Smart scoring** — ranks candidates by denomination, year range, portrait match, and catalog presence
- **Deduplication** — processes each unique banknote type once, even with multiple variants/sides
- **TSV output** — machine-readable `filename<TAB>Pick#` for batch processing

## Installation

```bash
git clone <repo-url>
cd pick-finder
cargo build --release
```

The binary will be at `target/release/pick-finder`.

### Requirements

- Rust 2024 edition (1.85+)
- [Numista API key](https://en.numista.com/api/) (free registration)
- [OpenAI API key](https://platform.openai.com/api-keys) *(optional, for vision analysis)*

## Filename convention

Images must follow this naming pattern:

```
Country-Denomination-Year[-Variant]-Side.ext
```

| Field | Format | Examples |
|-------|--------|---------|
| **Country** | Title case, `_` for spaces | `Portugal`, `Sri_Lanka`, `Korea_South` |
| **Denomination** | Zero-padded; `_` = decimal point; `0-NN` = 0.NN | `00020` → 20, `00002_50` → 2.50, `0-50` → 0.50 |
| **Year** | 4-digit | `1987`, `2022` |
| **Variant** | Optional digit (physical copies of same note) | `1`, `2` |
| **Side** | `A` = obverse (front), `B` = reverse (back) | `A`, `B` |

**Examples:**

| Filename | Parsed as |
|----------|-----------|
| `Portugal-00100-1987-1-A.jpg` | Portugal, 100, 1987, variant 1, obverse |
| `Iran-000005-1938-1-B.jpg` | Iran, 5, 1938, variant 1, reverse |
| `Sri_Lanka-00002_50-1920-A.jpg` | Sri Lanka, 2.50, 1920, no variant, obverse |

## Usage

```bash
pick-finder <PREFIX> -k <NUMISTA_API_KEY> [-o <OPENAI_API_KEY>] [--tsv]
```

**`PREFIX`** is a path prefix — all files whose name starts with it are included.

### Examples

```bash
# All Iran banknotes, filename matching only
pick-finder /path/to/images/Iran/Iran -k $NUMISTA_KEY

# All Iran banknotes with OpenAI vision for better matching
pick-finder /path/to/images/Iran/Iran -k $NUMISTA_KEY -o $OPENAI_KEY

# Single denomination with OpenAI vision
pick-finder /path/to/images/Portugal-00100-1987- -k $NUMISTA_KEY -o $OPENAI_KEY

# Batch TSV output for scripting
pick-finder /path/to/images/Iran/Iran -k $NUMISTA_KEY --tsv > iran_picks.tsv
```

### CLI options

| Option | Required | Description |
|--------|----------|-------------|
| `<PREFIX>` | yes | Path prefix to match image files |
| `-k`, `--api-key` | yes | Numista API key |
| `-o`, `--openai-key` | no | OpenAI API key (enables vision analysis) |
| `--tsv` | no | Output one `filename\tPick#` line per unique banknote |

## Pipeline

```
Image files
  │
  ├─ 1. parser::parse_banknote_file()   →  BanknoteFile
  │                                         (country, denomination, year, variant, side)
  │
  ├─ 2. Deduplicate by (country, denomination, year)
  │       Variants are physical copies of the same note
  │
  ├─ 3. ocr::extract_features()         →  VisionFeatures  [optional, -o flag]
  │       Sends unique obverse images to GPT-4o
  │       Extracts: currency name, portrait, signatures
  │
  ├─ 4. BanknoteQuery::from_file()      →  Merge filename + vision features
  │       Without vision: denomination + year only
  │       With vision:    denomination + currency + year + portrait
  │
  ├─ 5. matcher::find_matches()
  │       ├─ normalize_issuer()          →  Numista issuer code (230+ mappings)
  │       ├─ build_search_query()
  │       │     Without vision: "100 1987"
  │       │     With vision:    "100 Escudo 1987"
  │       ├─ numista::search_types()     →  Candidate list
  │       ├─ numista::get_type()         →  Full details per candidate
  │       └─ score_candidate()           →  Ranked PickMatch results
  │
  └─ 6. Output
          Default: verbose ranked list with scores
          --tsv:   one "filename\tPick#" line per unique banknote
```

## Output formats

### Default (verbose)

```
Parsed 4 banknote image(s) from 4 file(s):

  Portugal | 100 | 1987 var.1 | front
  Portugal | 100 | 1987 var.1 | back
  Portugal | 100 | 1987 var.2 | front
  Portugal | 100 | 1987 var.2 | back

Running vision analysis on 1 unique obverse image(s)...

  Portugal-00100-1987-1-A.jpg
    denomination: 100 Escudo
    year:         1987
    portrait:     Fernando Pessoa

Searching Numista for 1 unique banknote type(s)...

  Portugal 100 Escudo (1987):
    [202789] 100 Escudos (9th. print) | years: 1986-1988 | P#179 (score: 180) ◀ best
```

### TSV mode (`--tsv`)

One line per unique banknote type (country + denomination + year):

```
Iran-000005-1938-1-A.jpg	P#32
Iran-000005-1944-A.jpg	P#39
Iran-000010-1932-A.jpg	P#19
...
```

Empty Pick# column means no match was found for that banknote.

## Scoring algorithm

Each Numista candidate is scored against the query. Candidates with a denomination mismatch are immediately discarded (score = 0).

| Criterion | Points | Notes |
|-----------|--------|-------|
| **Denomination match** | +100 | Compares `value.numeric_value`; falls back to title parsing |
| **Year in range** | +50 | Query year falls within `[min_year, max_year]` |
| **Exact year** | +30 | Single-year issue matching the query year exactly |
| **Portrait match** | +20 | Vision-detected portrait surname found in obverse description |
| **Has Pick number** | +10 | At least one `P#` reference in the catalog entry |

Results are sorted by score descending. The top result is marked `◀ best` in verbose output and used for TSV output.

## Architecture

```
src/
├── main.rs       # CLI, file discovery, deduplication, orchestration
├── parser.rs     # Filename → BanknoteFile (country, denom, year, variant, side)
├── ocr.rs        # OpenAI GPT-4o Vision → VisionFeatures (currency, portrait, signatures)
├── numista.rs    # Numista API client (search types, get type details)
└── matcher.rs    # Query building, issuer normalization (230+ countries), scoring, Pick# extraction
```

## Supported countries

The `normalize_issuer` function maps **~230 country prefixes** to Numista issuer codes:

- **Standard** — `Portugal` → `portugal`, `Japan` → `japon`, `Germany` → `germany`
- **Local-language** — `Norge` → `norway`, `Sverige` → `sweden_section`, `Oesterreich` → `austria`, `Schweiz` → `switzerland`, `Polski` → `pologne`, `Danmark` → `denmark`
- **Historical** — `Ceskoslovenska` → `tchecoslovaquie`, `Jugoslavia` → `yougoslavie`, `Siam` → `thailande`, `Ceylon` → `ceylon`, `Burma` → `birmanie`, `Zaire` → `zaire_period`
- **Colonial/regional** — `Indochina` → `indochine`, `Africa_East` → `east-africa`, `Malaya_British_Borneo` → `malaya_borneo`
- **Fantasy/novelty** (Disney, Kamberra, Atlantic_Forest, etc.) — passthrough, won't match on Numista

## Dependencies

| Crate | Purpose |
|-------|---------|
| [clap](https://crates.io/crates/clap) 4 | CLI argument parsing with derive macros |
| [reqwest](https://crates.io/crates/reqwest) 0.12 | HTTP client (blocking, rustls-tls) |
| [serde](https://crates.io/crates/serde) 1 | JSON deserialization |
| [serde_json](https://crates.io/crates/serde_json) 1 | JSON parsing for OpenAI responses |
| [base64](https://crates.io/crates/base64) 0.22 | Image encoding for OpenAI Vision API |

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.
