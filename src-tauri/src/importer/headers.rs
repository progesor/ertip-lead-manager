pub const PRODUCT_INTEREST_HEADER: &str =
    "which_product_would_you_like_to_receive_more_information_about?";

pub const REQUIRED_HEADERS: [&str; 5] = [
    "id",
    "created_time",
    "full_name",
    "email",
    "phone_number",
];

pub const AGENCY_IGNORED_CRM_COLUMNS: [&str; 2] = ["Status", "İletişime Geçme Tarihi"];

pub fn normalize_header(value: &str) -> String {
    value
        .trim_start_matches('\u{feff}')
        .trim()
        .to_lowercase()
}

pub fn has_required_headers(headers: &[String]) -> bool {
    REQUIRED_HEADERS.iter().all(|required| {
        let required = normalize_header(required);
        headers
            .iter()
            .any(|header| normalize_header(header) == required)
    })
}

pub fn is_agency_ignored_crm_column(header: &str) -> bool {
    AGENCY_IGNORED_CRM_COLUMNS
        .iter()
        .any(|candidate| header.trim().eq_ignore_ascii_case(candidate))
}

#[cfg(test)]
mod tests {
    use super::{has_required_headers, is_agency_ignored_crm_column, normalize_header};

    #[test]
    fn header_normalization_strips_utf8_bom_and_whitespace() {
        assert_eq!(normalize_header("\u{feff} ID  "), "id");
    }

    #[test]
    fn required_headers_are_order_independent() {
        let headers = vec![
            "phone_number".to_string(),
            "full_name".to_string(),
            "created_time".to_string(),
            "id".to_string(),
            "email".to_string(),
        ];

        assert!(has_required_headers(&headers));
    }

    #[test]
    fn agency_columns_are_explicitly_recognized() {
        assert!(is_agency_ignored_crm_column("Status"));
        assert!(is_agency_ignored_crm_column("İletişime Geçme Tarihi"));
        assert!(!is_agency_ignored_crm_column("lead_status"));
    }
}
