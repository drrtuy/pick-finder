use serde::Deserialize;

const BASE_URL: &str = "https://api.numista.com/v3";

#[derive(Debug, Clone, Deserialize)]
pub struct SearchResult {
    pub count: u32,
    pub types: Vec<TypeSummary>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TypeSummary {
    pub id: u64,
    pub title: String,
    pub category: String,
    pub issuer: Option<Issuer>,
    pub min_year: Option<i32>,
    pub max_year: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Issuer {
    pub code: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TypeDetail {
    pub id: u64,
    pub title: String,
    pub url: Option<String>,
    pub category: Option<String>,
    pub issuer: Option<Issuer>,
    pub min_year: Option<i32>,
    pub max_year: Option<i32>,
    pub value: Option<Value>,
    pub obverse: Option<SideDescription>,
    pub reverse: Option<SideDescription>,
    pub references: Option<Vec<Reference>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Value {
    pub text: Option<String>,
    pub numeric_value: f64,
    pub currency: Option<Currency>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Currency {
    pub id: Option<u64>,
    pub name: Option<String>,
    pub full_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SideDescription {
    pub description: Option<String>,
    pub thumbnail: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Reference {
    pub catalogue: Option<CatalogueInfo>,
    pub number: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CatalogueInfo {
    pub id: Option<u64>,
    pub code: Option<String>,
}

pub struct NumistaClient {
    api_key: String,
    http: reqwest::blocking::Client,
}

impl NumistaClient {
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            http: reqwest::blocking::Client::new(),
        }
    }

    /// Search for banknote types by query string and issuer country code.
    ///
    /// Example: search_types("5 escudos", "portugal")
    pub fn search_types(&self, query: &str, issuer: &str) -> Option<SearchResult> {
        let url = format!("{BASE_URL}/types");

        let resp = self
            .http
            .get(&url)
            .header("Numista-API-Key", &self.api_key)
            .query(&[
                ("q", query),
                ("issuer", issuer),
                ("category", "banknote"),
            ])
            .send()
            .ok()?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            eprintln!("  [numista] search error {status}: {text}");
            return None;
        }

        resp.json().ok()
    }

    /// Get full details for a specific type by its Numista ID.
    pub fn get_type(&self, type_id: u64) -> Option<TypeDetail> {
        let url = format!("{BASE_URL}/types/{type_id}");

        let resp = self
            .http
            .get(&url)
            .header("Numista-API-Key", &self.api_key)
            .send()
            .ok()?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            eprintln!("  [numista] get_type({type_id}) error {status}: {text}");
            return None;
        }

        resp.json().ok()
    }

}
