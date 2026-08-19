//! Phone number verification tool built on Google's libphonenumber metadata
//! (via the `phonenumber` crate): parsing, validation, E.164 normalization,
//! country detection and mobile/landline classification.

use async_trait::async_trait;
use phonenumber::{country, Mode, Type};
use pr_core::{ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::registry::{Tool, ToolContext};

// ─── Result types ───

/// Full result of a phone number verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhoneVerification {
    /// The input as provided by the caller.
    pub original: String,
    /// Normalized E.164 representation (e.g. `+14155552671`).
    pub normalized: String,
    /// ISO 3166-1 alpha-2 region code (e.g. `US`), empty when unknown.
    pub country_code: String,
    /// Human-readable country name (e.g. `United States`), empty when unknown.
    pub country_name: String,
    /// International dialing code (e.g. `1`), empty when unknown.
    pub dial_code: String,
    pub is_valid: bool,
    /// Mobile (or fixed-line-or-mobile where regions don't distinguish).
    pub is_mobile: bool,
    /// Number type as classified by libphonenumber, e.g. `mobile`.
    pub number_type: String,
    /// Carrier when one can be extracted from the number itself.
    pub carrier: Option<String>,
    /// Parse/validation problem when `is_valid` is false.
    pub error: Option<String>,
}

// ─── Verifier ───

pub struct PhoneVerifier;

impl PhoneVerifier {
    /// Verify and normalize a phone number.
    ///
    /// 1. Formatting (spaces, dashes, parentheses) is stripped by the parser.
    /// 2. The country code is detected from a leading `+` or the `default_country`.
    /// 3. The number is validated against libphonenumber's metadata.
    /// 4. Mobile vs landline is classified.
    /// 5. The number is normalized to E.164.
    pub fn verify(&self, phone: &str, default_country: Option<&str>) -> PhoneVerification {
        let original = phone.trim().to_string();

        let region = default_country
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .and_then(|c| c.to_uppercase().parse::<country::Id>().ok());

        // Unknown default country → report a clear error rather than guessing.
        if default_country.map(str::trim).filter(|c| !c.is_empty()).is_some() && region.is_none() {
            return PhoneVerification {
                original,
                normalized: strip_formatting(phone),
                country_code: String::new(),
                country_name: String::new(),
                dial_code: String::new(),
                is_valid: false,
                is_mobile: false,
                number_type: "unknown".to_string(),
                carrier: None,
                error: Some(format!(
                    "unknown default country '{}' (use ISO 3166-1 alpha-2, e.g. US)",
                    default_country.unwrap_or_default()
                )),
            };
        }

        match phonenumber::parse(region, &original) {
            Ok(number) => {
                let is_valid = number.is_valid();
                let normalized = number.format().mode(Mode::E164).to_string();
                let dial_code = number.country().code().to_string();
                let region_id = number.country().id();
                let country_code = region_id
                    .map(|id| id.as_ref().to_string())
                    .unwrap_or_default();
                let country_name = region_id
                    .map(|id| country_name_from_id(id.as_ref()))
                    .unwrap_or_default();
                let number_type = number.number_type(&phonenumber::metadata::DATABASE);
                let is_mobile = matches!(
                    number_type,
                    Type::Mobile | Type::FixedLineOrMobile
                );
                let carrier = number.carrier().map(|c| c.to_string());

                PhoneVerification {
                    original,
                    normalized,
                    country_code,
                    country_name,
                    dial_code,
                    is_valid,
                    is_mobile,
                    number_type: type_name(number_type).to_string(),
                    carrier,
                    error: if is_valid {
                        None
                    } else {
                        Some("number parsed but fails length/type validation for its region".into())
                    },
                }
            }
            Err(e) => PhoneVerification {
                original,
                normalized: strip_formatting(phone),
                country_code: String::new(),
                country_name: String::new(),
                dial_code: String::new(),
                is_valid: false,
                is_mobile: false,
                number_type: "unknown".to_string(),
                carrier: None,
                error: Some(e.to_string()),
            },
        }
    }
}

/// Strip common formatting characters, keeping digits and a leading `+`.
/// Used only as a fallback `normalized` value for unparseable input.
pub fn strip_formatting(phone: &str) -> String {
    let trimmed = phone.trim();
    let plus = if trimmed.starts_with('+') { "+" } else { "" };
    let digits: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();
    format!("{plus}{digits}")
}

fn type_name(t: Type) -> &'static str {
    match t {
        Type::FixedLine => "fixed_line",
        Type::Mobile => "mobile",
        Type::FixedLineOrMobile => "fixed_line_or_mobile",
        Type::TollFree => "toll_free",
        Type::PremiumRate => "premium_rate",
        Type::SharedCost => "shared_cost",
        Type::PersonalNumber => "personal_number",
        Type::Voip => "voip",
        Type::Pager => "pager",
        Type::Uan => "uan",
        Type::Voicemail => "voicemail",
        Type::Emergency => "emergency",
        Type::ShortCode => "short_code",
        Type::StandardRate => "standard_rate",
        Type::Carrier => "carrier",
        Type::NoInternational => "no_international",
        Type::Unknown => "unknown",
    }
}

/// Human-readable country name for common ISO region codes; falls back to the
/// code itself for rare regions.
pub fn country_name_from_id(id: &str) -> String {
    let name = match id {
        "US" => "United States",
        "CA" => "Canada",
        "GB" => "United Kingdom",
        "DE" => "Germany",
        "FR" => "France",
        "IT" => "Italy",
        "ES" => "Spain",
        "PT" => "Portugal",
        "NL" => "Netherlands",
        "BE" => "Belgium",
        "CH" => "Switzerland",
        "AT" => "Austria",
        "SE" => "Sweden",
        "NO" => "Norway",
        "DK" => "Denmark",
        "FI" => "Finland",
        "IS" => "Iceland",
        "IE" => "Ireland",
        "PL" => "Poland",
        "CZ" => "Czechia",
        "SK" => "Slovakia",
        "HU" => "Hungary",
        "RO" => "Romania",
        "BG" => "Bulgaria",
        "GR" => "Greece",
        "UA" => "Ukraine",
        "BY" => "Belarus",
        "RU" => "Russia",
        "KZ" => "Kazakhstan",
        "TR" => "Turkey",
        "IL" => "Israel",
        "AE" => "United Arab Emirates",
        "SA" => "Saudi Arabia",
        "EG" => "Egypt",
        "NG" => "Nigeria",
        "KE" => "Kenya",
        "ZA" => "South Africa",
        "IN" => "India",
        "CN" => "China",
        "JP" => "Japan",
        "KR" => "South Korea",
        "SG" => "Singapore",
        "MY" => "Malaysia",
        "ID" => "Indonesia",
        "TH" => "Thailand",
        "VN" => "Vietnam",
        "PH" => "Philippines",
        "AU" => "Australia",
        "NZ" => "New Zealand",
        "BR" => "Brazil",
        "MX" => "Mexico",
        "AR" => "Argentina",
        "CL" => "Chile",
        "CO" => "Colombia",
        _ => return id.to_string(),
    };
    name.to_string()
}

// ─── Tool ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct VerifyPhoneParams {
    /// Phone number to verify (any formatting: spaces, dashes, parentheses)
    phone: String,
    /// Default country for numbers without an international prefix (ISO 3166-1 alpha-2, e.g. "US")
    #[serde(default)]
    default_country: Option<String>,
}

#[async_trait]
impl Tool for PhoneVerifier {
    fn name(&self) -> &str {
        "verify_phone"
    }
    fn description(&self) -> &str {
        "Validate and normalize a phone number. Returns E.164 format, detected country, dialing code, and mobile vs landline classification.

## Capability

Parses phone numbers in any common format (spaces, dashes, parentheses, leading zeros) using Google's libphonenumber metadata. Detects the country from the international prefix (or from `default_country` when the number is in national format), validates the length and type for that region, classifies mobile vs landline where the region allows it, and returns the normalized E.164 form.

## When to Use

- Normalizing phone numbers collected during OSINT / lead generation.
- Checking whether a phone number is plausibly valid before adding it to a contact record.
- Determining the country a number belongs to.

## When NOT to Use

- Do NOT treat `is_valid` as proof the number is active — validation is offline and metadata-based; it cannot detect disconnected numbers.
- For regions like the US/Canada mobile vs landline cannot be determined from the number alone; `is_mobile` then reflects \"fixed-line-or-mobile\".

## Output

E.164 normalized number, ISO country code + name, dialing code, validity, number type, and the parse error (if any).

## Failure Modes

- Numbers without `+` require `default_country`, otherwise parsing may fail or pick the wrong region.
- Very short input (< 3 digits) is rejected as \"not a number\"."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(VerifyPhoneParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: VerifyPhoneParams = serde_json::from_value(args)?;
        let result = self.verify(&params.phone, params.default_country.as_deref());

        let mut out = format!("Phone verification: {}\n", result.original);
        out.push_str(&format!(
            "Valid: {}\n",
            if result.is_valid { "yes" } else { "NO" }
        ));
        out.push_str(&format!("Normalized (E.164): {}\n", result.normalized));
        if !result.country_code.is_empty() {
            out.push_str(&format!(
                "Country: {} ({}) dial code +{}\n",
                result.country_name, result.country_code, result.dial_code
            ));
        }
        out.push_str(&format!("Type: {}\n", result.number_type));
        out.push_str(&format!(
            "Mobile: {}\n",
            if result.is_mobile { "yes" } else { "no" }
        ));
        if let Some(ref carrier) = result.carrier {
            out.push_str(&format!("Carrier: {carrier}\n"));
        }
        if let Some(ref error) = result.error {
            out.push_str(&format!("Error: {error}\n"));
        }

        let meta = serde_json::to_value(&result).unwrap_or_default();
        Ok(ToolOutput::ok_with_meta(out, meta))
    }
}

// ─── Tests ───

#[cfg(test)]
mod tests {
    use super::*;

    fn verifier() -> PhoneVerifier {
        PhoneVerifier
    }

    #[test]
    fn test_valid_us_number_e164() {
        let r = verifier().verify("+1 (415) 555-2671", None);
        assert!(r.is_valid, "error: {:?}", r.error);
        assert_eq!(r.normalized, "+14155552671");
        assert_eq!(r.country_code, "US");
        assert_eq!(r.country_name, "United States");
        assert_eq!(r.dial_code, "1");
    }

    #[test]
    fn test_national_format_with_default_country() {
        let r = verifier().verify("030 123456", Some("DE"));
        assert!(r.is_valid, "error: {:?}", r.error);
        assert_eq!(r.normalized, "+4930123456");
        assert_eq!(r.country_code, "DE");
        assert_eq!(r.country_name, "Germany");
    }

    #[test]
    fn test_russian_mobile() {
        let r = verifier().verify("+7 916 123-45-67", None);
        assert!(r.is_valid, "error: {:?}", r.error);
        assert_eq!(r.normalized, "+79161234567");
        assert_eq!(r.country_code, "RU");
        assert!(r.is_mobile, "916 is a mobile prefix, got {}", r.number_type);
    }

    #[test]
    fn test_uk_number() {
        let r = verifier().verify("+44 20 7946 0958", None);
        assert!(r.is_valid, "error: {:?}", r.error);
        assert_eq!(r.country_code, "GB");
        assert_eq!(r.country_name, "United Kingdom");
        // London 020 numbers are fixed line.
        assert!(!r.is_mobile);
    }

    #[test]
    fn test_invalid_number_wrong_length() {
        let r = verifier().verify("+1 415", None);
        assert!(!r.is_valid);
        assert!(r.error.is_some());
    }

    #[test]
    fn test_garbage_input() {
        let r = verifier().verify("not a phone", None);
        assert!(!r.is_valid);
        assert!(r.error.is_some());
    }

    #[test]
    fn test_unknown_default_country_reports_error() {
        let r = verifier().verify("123456789", Some("XX"));
        assert!(!r.is_valid);
        assert!(r.error.unwrap().contains("unknown default country"));
    }

    #[test]
    fn test_lowercase_country_code_accepted() {
        let r = verifier().verify("030 123456", Some("de"));
        assert!(r.is_valid, "error: {:?}", r.error);
        assert_eq!(r.country_code, "DE");
    }

    #[test]
    fn test_strip_formatting() {
        assert_eq!(strip_formatting("+1 (415) 555-2671"), "+14155552671");
        assert_eq!(strip_formatting("8-800-555-35-35"), "88005553535");
        assert_eq!(strip_formatting("abc"), "");
    }

    #[test]
    fn test_country_name_fallback() {
        assert_eq!(country_name_from_id("FR"), "France");
        // Rare codes fall back to the code itself.
        assert_eq!(country_name_from_id("XK"), "XK");
    }

    #[test]
    fn test_tool_metadata() {
        let tool = PhoneVerifier;
        assert_eq!(tool.name(), "verify_phone");
        let schema = tool.schema();
        assert_eq!(schema.name, "verify_phone");
        assert!(schema.parameters.get("properties").is_some());
    }
}
