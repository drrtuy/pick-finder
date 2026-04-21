use crate::numista::{NumistaClient, Reference, TypeDetail};
use crate::ocr::VisionFeatures;
use crate::parser::BanknoteFile;

/// Merged query combining filename features + optional vision features.
#[derive(Debug)]
pub struct BanknoteQuery {
    pub country: String,
    pub denomination: f64,
    pub year: u16,
    pub variant: Option<u16>,
    /// From vision: currency name (e.g. "Escudos", "Dollars")
    pub currency: Option<String>,
    /// From vision: portrait name
    pub portrait: Option<String>,
    /// From file path: issuing bank (e.g. "North of Scotland Bank Limited")
    pub issuing_bank: Option<String>,
}

impl BanknoteQuery {
    pub fn from_file(b: &BanknoteFile, vision: Option<&VisionFeatures>) -> Self {
        Self {
            country: b.country.clone(),
            denomination: b.denomination,
            year: b.year,
            variant: b.variant,
            currency: vision.and_then(|v| v.currency.clone()),
            portrait: vision.and_then(|v| v.portrait.clone()),
            issuing_bank: b.issuing_bank.clone(),
        }
    }
}

/// A matched Numista type with its Pick number(s).
#[derive(Debug)]
pub struct PickMatch {
    pub numista_id: u64,
    pub title: String,
    pub url: Option<String>,
    pub min_year: Option<i32>,
    pub max_year: Option<i32>,
    pub pick_numbers: Vec<String>,
    pub score: u32,
}

/// Run the full matching pipeline:
///   1. Build Numista search query from filename + vision features
///   2. Search Numista → candidates
///   3. For each candidate, fetch details and extract Pick#
///   4. Filter & rank by denomination, year, portrait
pub fn find_matches(query: &BanknoteQuery, client: &NumistaClient) -> Vec<PickMatch> {
    let issuer = normalize_issuer(&query.country);

    // Build search query: "{denomination} {currency}" or just "{denomination}"
    let search_q = build_search_query(query);
    eprintln!("  [match] query=\"{search_q}\" issuer=\"{issuer}\"");

    let results = match client.search_types(&search_q, &issuer) {
        Some(r) => r,
        None => return vec![],
    };

    let mut matches: Vec<PickMatch> = vec![];

    for t in &results.types {
        let detail = match client.get_type(t.id) {
            Some(d) => d,
            None => continue,
        };

        // Extract Pick numbers from references
        let pick_numbers = extract_pick_numbers(&detail);

        // Score this candidate against the query
        let score = score_candidate(query, &t.title, &detail);

        // Skip candidates with zero score (no denomination or year match at all)
        if score == 0 {
            continue;
        }

        matches.push(PickMatch {
            numista_id: t.id,
            title: t.title.clone(),
            url: detail.url,
            min_year: t.min_year,
            max_year: t.max_year,
            pick_numbers,
            score,
        });
    }

    // Sort by score descending
    matches.sort_by(|a, b| b.score.cmp(&a.score));
    matches
}

fn build_search_query(query: &BanknoteQuery) -> String {
    let denom = format_denomination(query.denomination);
    let year = query.year;
    match &query.currency {
        Some(c) => format!("{denom} {c} {year}"),
        None => format!("{denom} {year}"),
    }
}

/// Normalize a bank name for fuzzy comparison:
///   - lowercase
///   - strip common corporate suffixes (Ltd, Limited, plc, Inc, Company, Co, &)
///   - collapse punctuation/whitespace
fn normalize_bank(s: &str) -> String {
    let lower = s.to_lowercase();
    let cleaned: String = lower
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    cleaned
        .split_whitespace()
        .filter(|w| !matches!(*w, "ltd" | "limited" | "plc" | "inc" | "co" | "company" | "the" | "of"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_denomination(d: f64) -> String {
    if d.fract() == 0.0 {
        format!("{}", d as i64)
    } else {
        format!("{:.2}", d)
    }
}

/// Normalize country name from filename to Numista issuer code.
///
/// Numista uses its own issuer codes (often French-influenced).
/// This table maps every country prefix found in the banknote image catalog
/// to the corresponding Numista code.
fn normalize_issuer(country: &str) -> String {
    let lower = country.to_lowercase();
    let code = match lower.as_str() {
        // A
        "abkhazia" => "abkhazia",
        "afghanistan" => "afghanistan",
        "africa_centrafricaine" => "centrafrique",
        "africa_centrale" => "beac",
        "africa_east" => "east-africa",
        "africa_francaise_libre" => "aef",
        "africa_louest" => "bceao",
        "africa_occidentale" => "aof",
        "africa_west_british" => "british-west-africa",
        "albania" => "albania_section",
        "algeria" => "algerie",
        "america" | "usa" | "us" | "united_states" => "united-states",
        "andorra" => "andorra",
        "angola" => "angola",
        "antigua_and_barbuda" => "antigua-et-barbuda",
        "arab" | "united_arab" => "uae",
        "argentina" => "argentina",
        "armenia" => "armenia",
        "aruba" => "aruba",
        "australia" => "australia",
        "azerbaijan" => "azerbaijan",
        "azores" => "acores",
        // B
        "bahamas" => "bahamas",
        "bahrain" => "bahrein",
        "bangladesh" => "bangladesh",
        "barbados" => "barbados",
        "belarus" => "belarus",
        "belgie" => "belgium",
        "belize" => "belize",
        "bermuda" => "bermuda",
        "bhutan" => "bhutan",
        "biafra" => "biafra",
        "bohmen_und_mahren" => "bohemia_section",
        "bolivia" => "bolivia",
        "bosniahr" | "bosnaher" => "bosnia-herzegovina",
        "botswana" => "botswana",
        "brazil" | "brasil" => "brazil",
        "british_armed" => "british_armed_forces",
        "british_caribbean" => "british-caribbean-territories",
        "british_guiana" => "british_guiana",
        "british_honduras" => "british_honduras_period",
        "british_military_authority" => "bma",
        "british_north_borneo" => "borneo",
        "brunei" => "brunei",
        "bulgaria" => "bulgaria",
        "burma" => "birmanie",
        "burundi" => "burundi",
        // C
        "cabo_verde" => "cap-vert",
        "cambodia" => "cambodia",
        "cambodia_laos_vietnam" => "indochine",
        "cameroun" => "cameroun",
        "canada" => "canada",
        "cayman_islands" => "iles_caimanes",
        "ceskoslovenska" => "tchecoslovaquie",
        "ceylon" => "ceylon",
        "chatman_islands" => "chatham_islands",
        "chile" => "chile",
        "china" => "china",
        "colombia" => "colombia",
        "comores" => "comoro_section",
        "congo" => "congo",
        "cook_island" => "iles_cook",
        "costa_rica" => "costa_rica",
        "croatia" => "croatia",
        "cuba" => "cuba",
        "curacao" => "curacao",
        "curacao_and_sint_maarten" => "curacao_sint_marteen",
        "cyprus" => "cyprus",
        "czech" => "czech-republic",
        // D
        "danmark" => "denmark",
        "danzig" => "dantzig",
        "djibouti" => "djibouti",
        "dominicana" => "dominican-republic",
        // E
        "eastern_caribbean" => "eastern-caribbean",
        "ecuador" => "ecuador",
        "egypt" => "egypt",
        "el_salvador" => "el-salvador",
        "england" | "uk" | "united_kingdom" => "united-kingdom",
        "eritrea" => "eritrea",
        "estonia" => "estonia",
        "eswatini" => "swaziland_eswatini_section",
        "ethiopia" => "ethiopia",
        // F
        "falkland" => "iles_malouines",
        "faroarna" => "iles_feroe",
        "fiji" => "fiji",
        "france" => "france",
        "french_antilles" => "antilles_francaises",
        // G
        "gabonaise" => "gabon",
        "gambia" => "gambie",
        "georgia" => "georgia_section",
        "germany" => "germany",
        "ghana" => "ghana",
        "gibraltar" => "gibraltar",
        "greece" => "greece",
        "greenland" => "groenland",
        "guadeloupe" => "guadeloupe",
        "guatemala" => "guatemala",
        "guernsey" => "guernsey",
        "guine" => "guinea-bissau",
        "guine_bissau" => "guinea-bissau",
        "guinea_ecuatorial" => "guinee_equatoriale",
        "guinee" => "guinee",
        "guyana" => "guyana",
        "guyane" => "french-guiana",
        // H
        "haiti" => "haiti",
        "honduras" => "honduras",
        "hongkong" => "hong_kong",
        "hungary" => "hungary",
        // I
        "india" => "india",
        "indochina" => "indochine",
        "indonesia" => "indonesia",
        "iran" => "iran",
        "iraq" => "iraq",
        "ireland" => "irlande",
        "ireland_north" => "northern-ireland",
        "island" => "islande",
        "isle_of_man" => "ile_de_man",
        "israel" => "israel",
        "italy" => "italy",
        "ivory_coast" => "cote-d-ivoire",
        // J
        "jamaica" => "jamaique",
        "japan" => "japon",
        "jersey" => "jersey",
        "jordan" => "jordanie",
        "jugoslavia" => "yougoslavie",
        // K
        "katanga" => "katanga",
        "kazakhstan" => "kazakhstan",
        "keeling_cocos_islands" => "cocos",
        "kenya" => "kenya",
        "kinmen" => "taiwan",
        "korea" => "korea",
        "korea_north" => "coree_du_nord",
        "korea_south" => "coree_du_sud",
        "kosovo" => "kosovo",
        "kuwait" => "koweit",
        "kyrgyzstan" => "kirghizistan",
        // L
        "laos" => "laos",
        "latvia" => "lettonie",
        "lesotho" => "lesotho",
        "liban" => "liban",
        "liberia" => "liberia",
        "libya" => "libye",
        "liechtenstein" => "liechtenstein",
        "lietuvos" => "lituanie",
        "luxembourg" => "luxembourg",
        // M
        "macau" => "macao",
        "madagascar" => "madagascar",
        "makedonia" => "macedoine",
        "malagasy" => "madagascar",
        "malawi" => "malawi",
        "malaya" => "malaya",
        "malaya_british_borneo" => "malaya_borneo",
        "malaysia" => "malaysia",
        "maldives" => "maldives",
        "mali" => "mali",
        "malta" => "malte",
        "martinique" => "martinique",
        "matsu" => "taiwan",
        "mauritania" => "mauritanie",
        "mauritius" => "maurice",
        "mexico" => "mexico",
        "moldova" => "moldavie",
        "monaco" => "monaco",
        "mongolia" => "mongolie",
        "montenegro" => "montenegro",
        "morocco" => "maroc",
        "mozambique" => "mozambique",
        "muscat_oman" => "muscat_and_oman",
        "myanmar" => "myanmar",
        // N
        "nagorno_karabakh" => "haut-karabagh",
        "namibia" => "namibie",
        "nederland" => "netherlands",
        "nederlands_antilles" => "antilles_neerlandaises",
        "nederlands_new_guinea" => "netherlands_new_guinea",
        "nepal" => "nepal",
        "new_caledonia" => "nouvelle-caledonie",
        "new_hebrides" => "new_hebrides_period",
        "new_zealand" => "new_zealand_section",
        "newfoundland" => "newfoundland",
        "nicaragua" => "nicaragua",
        "nigeria" => "nigeria",
        "norge" => "norway",
        // O
        "oceania" => "french-oceania",
        "oesterreich" => "austria",
        "oman" => "oman",
        // P
        "pakistan" => "pakistan",
        "palestine" => "palestine",
        "papua_new_guinea" => "papua-new-guinea",
        "paraguay" => "paraguay",
        "peru" => "perou",
        "philippines" => "philippines",
        "polski" => "pologne",
        "portugal" => "portugal",
        "puerto_rico" => "porto_rico",
        // Q
        "qatar" => "qatar",
        "qatar_and_dubai" => "qatar-et-dubai",
        // R
        "reunion_island" => "reunion",
        "rhodesia" => "rhodesie",
        "rhodesia_and_nyasaland" => "rhodesie_et_nyassaland",
        "romania" => "roumanie",
        "russia" => "russia",
        "rwanda" => "rwanda",
        "rwanda_burundi" => "rwanda-burundi",
        // S
        "saint_helena" => "saint-helena",
        "saint_pierre_et_miquelon" => "st-pierre-et-miquelon",
        "saint_tome" => "sao_tome-et-principe",
        "samoa" => "samoa",
        "san_marino" => "san-marino",
        "sarawak" => "sarawak",
        "saudi_arabia" => "saudi-arabia",
        "schweiz" => "switzerland",
        "scotland" => "scotland",
        "senegal" => "senegal",
        "serbia" => "serbie",
        "seychelles" => "seychelles",
        "siam" => "thailande",
        "sierra_leone" => "sierra_leone",
        "singapore" => "singapour",
        "slovakia" => "slovaquie",
        "slovenija" => "slovenie",
        "solomon" => "solomon",
        "somalia" => "somalia",
        "somaliland" => "somaliland",
        "south_africa" => "south-africa",
        "south_arabia" => "arabie_sud",
        "south_sudan" => "sud_soudan",
        "southern_rhodesia" => "rhodesie_du_sud",
        "spain" => "spain",
        "sri_lanka" => "sri-lanka",
        "straits_settlements" => "straits",
        "sudan" => "sudan",
        "suriname" => "suriname",
        "sverige" => "sweden_section",
        "swaziland" => "swaziland",
        "syria" => "syria",
        // T
        "tahiti" => "tahiti",
        "taiwan" => "taiwan",
        "tajikistan" => "tadjikistan",
        "tanger" => "tangier_city",
        "tannu_tuva" => "tannou-touva",
        "tanzania" => "tanzania",
        "tatarstan" => "tatarstan",
        "tchad" => "tchad",
        "thailand" => "thailande",
        "theresienstadt" => "theresienstadt",
        "tibet" => "tibet",
        "timor" => "timor_oriental",
        "togo" => "togo",
        "tonga" => "tonga",
        "transnistria" => "transnistria",
        "trinidad" => "trinite-et-tobago_section",
        "tunisie" => "tunisie",
        "turkiye" => "turquie",
        "turkmenistan" => "turkmenistan",
        // U
        "uganda" => "ouganda",
        "ukraine" => "ukraine",
        "uruguay" => "uruguay",
        "uzbekistan" => "ouzbekistan",
        // V
        "vanuatu" => "vanuatu",
        "venezuela" => "venezuela",
        "vietnam" => "viet_nam",
        "vietnam_north" => "viet_nam_nord",
        "vietnam_south" => "viet_nam_sud",
        // W
        "western_samoa" => "samoa",
        // Y
        "yemen" => "yemen",
        // Z
        "zaire" => "zaire_period",
        "zambia" => "zambie",
        "zimbabwe" => "zimbabwe",
        // Fantasy / novelty issuers — pass through as-is (unlikely to match on Numista)
        _ => return lower.replace(' ', "_"),
    };
    code.to_string()
}

/// Extract Pick catalogue numbers from a TypeDetail's references.
fn extract_pick_numbers(detail: &TypeDetail) -> Vec<String> {
    detail
        .references
        .as_ref()
        .map(|refs| {
            refs.iter()
                .filter(|r| is_pick_reference(r))
                .filter_map(|r| r.number.as_ref())
                .map(|n| format!("Pick#{n}"))
                .collect()
        })
        .unwrap_or_default()
}

fn is_pick_reference(r: &Reference) -> bool {
    r.catalogue
        .as_ref()
        .and_then(|c| c.code.as_deref())
        .unwrap_or_default()
        == "P"
}

/// Score a Numista candidate against the banknote query.
///
/// Scoring:
///   +100  denomination matches (from value.numeric_value in title)
///   +50   year falls within [min_year, max_year]
///   +30   exact year match (min_year == max_year == year)
///   +20   portrait name found in obverse description
///   +10   has Pick number
fn score_candidate(query: &BanknoteQuery, title: &str, detail: &TypeDetail) -> u32 {
    let mut score: u32 = 0;

    // Denomination match: check numeric value first, then title as fallback
    let denom_matched = if let Some(val) = &detail.value {
        (val.numeric_value - query.denomination).abs() < 0.01
    } else {
        // Fallback: check title starts with denomination followed by a non-digit
        let denom_str = format_denomination(query.denomination);
        let title_lower = title.to_lowercase();
        let denom_lower = denom_str.to_lowercase();
        title_lower.starts_with(&format!("{denom_lower} "))
            || title_lower.starts_with(&format!("{denom_lower}."))
    };
    if denom_matched {
        score += 100;
    } else {
        // Denomination doesn't match — not a candidate
        return 0;
    }

    // Year match
    let year = query.year as i32;
    let in_range = match (detail.min_year, detail.max_year) {
        (Some(min), Some(max)) => year >= min && year <= max,
        (Some(min), None) => year >= min,
        (None, Some(max)) => year <= max,
        (None, None) => false,
    };
    if in_range {
        score += 50;
    }

    // Exact year match bonus
    if let (Some(min), Some(max)) = (detail.min_year, detail.max_year) {
        if min == year && max == year {
            score += 30;
        }
    }

    // Issuing bank match: hard filter when set on the query.
    // Numista's search API ignores the issuing_entity param, so we filter here.
    if let Some(bank) = &query.issuing_bank {
        let wanted = normalize_bank(bank);
        let candidate = detail
            .issuing_entity
            .as_ref()
            .map(|e| normalize_bank(&e.name));
        match candidate {
            Some(c) if c.contains(&wanted) || wanted.contains(&c) => {
                score += 100;
            }
            Some(_) => return 0,
            None => {
                // No issuing_entity on the candidate — fall back to title match
                let title_norm = normalize_bank(title);
                if title_norm.contains(&wanted) {
                    score += 100;
                } else {
                    return 0;
                }
            }
        }
    }

    // Portrait match (from vision)
    if let Some(portrait) = &query.portrait {
        if let Some(obverse) = &detail.obverse {
            if let Some(desc) = &obverse.description {
                let desc_lower = desc.to_lowercase();
                let portrait_lower = portrait.to_lowercase();
                // Check if portrait surname is in description
                let parts: Vec<&str> = portrait_lower.split_whitespace().collect();
                let fallback = portrait_lower.as_str();
                let surname = parts.last().unwrap_or(&fallback);
                if desc_lower.contains(surname) {
                    score += 20;
                }
            }
        }
    }

    // Has Pick number bonus
    let has_pick = detail
        .references
        .as_ref()
        .is_some_and(|refs| refs.iter().any(|r| is_pick_reference(r)));
    if has_pick {
        score += 10;
    }

    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_issuer() {
        assert_eq!(normalize_issuer("Portugal"), "portugal");
        assert_eq!(normalize_issuer("USA"), "united-states");
        assert_eq!(normalize_issuer("UK"), "united-kingdom");
        assert_eq!(normalize_issuer("Brasil"), "brazil");
        assert_eq!(normalize_issuer("New Zealand"), "new_zealand");
    }

    #[test]
    fn test_build_search_query() {
        let q = BanknoteQuery {
            country: "Portugal".into(),
            denomination: 5.0,
            year: 1914,
            variant: None,
            currency: Some("Escudos".into()),
            portrait: None,
            issuing_bank: None,
        };
        assert_eq!(build_search_query(&q), "5 Escudos 1914");

        let q2 = BanknoteQuery {
            country: "Portugal".into(),
            denomination: 0.5,
            year: 1918,
            variant: None,
            currency: None,
            portrait: None,
            issuing_bank: None,
        };
        assert_eq!(build_search_query(&q2), "0.50 1918");
    }

    #[test]
    fn test_format_denomination() {
        assert_eq!(format_denomination(5.0), "5");
        assert_eq!(format_denomination(2.5), "2.50");
        assert_eq!(format_denomination(10000.0), "10000");
    }
}
