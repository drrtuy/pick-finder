use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub enum Side {
    Obverse,
    Reverse,
}

#[derive(Debug, Clone)]
pub struct BanknoteFile {
    pub country: String,
    pub denomination: f64,
    pub year: u16,
    pub variant: Option<u16>,
    pub side: Side,
    pub path: PathBuf,
    /// Issuing bank, extracted from the parent directory when it differs from
    /// the country directory. E.g. for
    /// `/.../Scotland/North_of_Scotland_Bank_Limited/Scotland-0020-1930-A.jpg`
    /// this will be `Some("North of Scotland Bank Limited")`.
    pub issuing_bank: Option<String>,
}

/// Parse a banknote filename into structured data.
///
/// Expected patterns:
///   Portugal-0-50-1918-1-A.jpg      → 0.50, 1918, variant 1, obverse
///   Portugal-00001-1917-A.jpg       → 1.0,  1917, no variant, obverse
///   Portugal-00002_50-1920-1-B.jpg  → 2.50, 1920, variant 1, reverse
///   Portugal-00005-1914-B.jpg       → 5.0,  1914, no variant, reverse
pub fn parse_banknote_file(path: &Path) -> Option<BanknoteFile> {
    let filename = path.file_stem()?.to_str()?;

    let parts: Vec<&str> = filename.split('-').collect();
    if parts.len() < 3 {
        return None;
    }

    let country = parts[0].to_string();

    // Parse denomination — handle special cases:
    //   "0", "50"      → two parts forming 0.50
    //   "00002_50"     → 2.50  (underscore = decimal point)
    //   "00005"        → 5.0
    let (denomination, rest_start) = parse_denomination(&parts[1..])?;

    // Remaining parts after denomination: year, optional variant, side
    let rest = &parts[rest_start + 1..];
    if rest.is_empty() {
        return None;
    }

    let year: u16 = rest[0].parse().ok()?;

    let (variant, side) = match rest.len() {
        // year-side
        2 => {
            let side = parse_side(rest[1])?;
            (None, side)
        }
        // year-variant-side
        3 => {
            let variant: u16 = rest[1].parse().ok()?;
            let side = parse_side(rest[2])?;
            (Some(variant), side)
        }
        _ => return None,
    };

    // Issuing bank = immediate parent directory, if it differs from country
    let issuing_bank = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .filter(|n| *n != country)
        .map(|n| n.replace('_', " "));

    Some(BanknoteFile {
        country,
        denomination,
        year,
        variant,
        side,
        path: path.to_path_buf(),
        issuing_bank,
    })
}

/// Parse denomination from the parts slice starting after the country.
/// Returns (denomination_value, index_of_last_consumed_part_in_original_parts).
fn parse_denomination(parts: &[&str]) -> Option<(f64, usize)> {
    if parts.is_empty() {
        return None;
    }

    let first = parts[0];

    // Case: "0" followed by centavo value → "0-50" means 0.50
    if first == "0" && parts.len() >= 2 {
        if let Ok(centavos) = parts[1].parse::<u32>() {
            if centavos < 100 {
                let denom = centavos as f64 / 100.0;
                return Some((denom, 2));
            }
        }
    }

    // Case: underscore decimal like "00002_50" → 2.50
    if first.contains('_') {
        let sub: Vec<&str> = first.splitn(2, '_').collect();
        if sub.len() == 2 {
            let whole: f64 = sub[0].parse().ok()?;
            let frac: f64 = sub[1].parse().ok()?;
            let divisor = 10_f64.powi(sub[1].len() as i32);
            return Some((whole + frac / divisor, 1));
        }
    }

    // Case: plain integer like "00005", "01000", "10000"
    let denom: f64 = first.parse().ok()?;
    Some((denom, 1))
}

fn parse_side(s: &str) -> Option<Side> {
    match s {
        "A" => Some(Side::Obverse),
        "B" => Some(Side::Reverse),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_half_escudo() {
        let b = parse_banknote_file(Path::new("/data/Portugal-0-50-1918-1-A.jpg")).unwrap();
        assert_eq!(b.country, "Portugal");
        assert!((b.denomination - 0.50).abs() < f64::EPSILON);
        assert_eq!(b.year, 1918);
        assert_eq!(b.variant, Some(1));
        assert_eq!(b.side, Side::Obverse);
    }

    #[test]
    fn test_half_escudo_no_variant() {
        let b = parse_banknote_file(Path::new("/data/Portugal-0-50-1920-A.jpg")).unwrap();
        assert!((b.denomination - 0.50).abs() < f64::EPSILON);
        assert_eq!(b.year, 1920);
        assert_eq!(b.variant, None);
    }

    #[test]
    fn test_one_escudo() {
        let b = parse_banknote_file(Path::new("/data/Portugal-00001-1917-A.jpg")).unwrap();
        assert!((b.denomination - 1.0).abs() < f64::EPSILON);
        assert_eq!(b.year, 1917);
        assert_eq!(b.variant, None);
    }

    #[test]
    fn test_two_and_half() {
        let b = parse_banknote_file(Path::new("/data/Portugal-00002_50-1920-1-A.jpg")).unwrap();
        assert!((b.denomination - 2.50).abs() < f64::EPSILON);
        assert_eq!(b.year, 1920);
        assert_eq!(b.variant, Some(1));
    }

    #[test]
    fn test_five_escudos() {
        let b = parse_banknote_file(Path::new("/data/Portugal-00005-1914-A.jpg")).unwrap();
        assert!((b.denomination - 5.0).abs() < f64::EPSILON);
        assert_eq!(b.year, 1914);
        assert_eq!(b.variant, None);
        assert_eq!(b.side, Side::Obverse);
    }

    #[test]
    fn test_thousand_with_variant() {
        let b = parse_banknote_file(Path::new("/data/Portugal-01000-1967-2-B.jpg")).unwrap();
        assert!((b.denomination - 1000.0).abs() < f64::EPSILON);
        assert_eq!(b.year, 1967);
        assert_eq!(b.variant, Some(2));
        assert_eq!(b.side, Side::Reverse);
    }

    #[test]
    fn test_ten_thousand() {
        let b = parse_banknote_file(Path::new("/data/Portugal-10000-1996-A.jpg")).unwrap();
        assert!((b.denomination - 10000.0).abs() < f64::EPSILON);
        assert_eq!(b.year, 1996);
        assert_eq!(b.variant, None);
    }
}
