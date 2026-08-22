use std::collections::BTreeSet;

pub const PRODUCT_CODES: [&str; 7] = [
    "FUE_MICROMOTOR_SYSTEMS",
    "LONG_HAIR_FUE_SOLUTIONS",
    "FUE_PUNCHES",
    "IMPLANTERS_FORCEPS_SURGICAL_INSTRUMENTS",
    "MEDICAL_CHAIRS_CLINIC_FURNITURE",
    "OTHER_GENERAL_INFORMATION",
    "UNKNOWN",
];

pub fn is_valid_product_code(value: &str) -> bool {
    PRODUCT_CODES.contains(&value)
}

pub fn effective_product_interests(
    automatic: impl IntoIterator<Item = String>,
    latest_overrides: impl IntoIterator<Item = (String, String)>,
) -> Vec<String> {
    let mut effective = automatic.into_iter().collect::<BTreeSet<_>>();

    for (product_code, action) in latest_overrides {
        match action.as_str() {
            "ADD" => {
                effective.insert(product_code);
            }
            "REMOVE" => {
                effective.remove(&product_code);
            }
            _ => {}
        }
    }

    effective.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::{effective_product_interests, is_valid_product_code};

    #[test]
    fn latest_manual_decisions_apply_over_automatic_interests() {
        let effective = effective_product_interests(
            vec!["FUE_PUNCHES".to_string(), "UNKNOWN".to_string()],
            vec![
                ("FUE_PUNCHES".to_string(), "REMOVE".to_string()),
                ("LONG_HAIR_FUE_SOLUTIONS".to_string(), "ADD".to_string()),
                ("UNKNOWN".to_string(), "REMOVE".to_string()),
            ],
        );

        assert_eq!(effective, vec!["LONG_HAIR_FUE_SOLUTIONS"]);
    }

    #[test]
    fn canonical_product_code_validation_is_stable() {
        assert!(is_valid_product_code("FUE_MICROMOTOR_SYSTEMS"));
        assert!(is_valid_product_code("UNKNOWN"));
        assert!(!is_valid_product_code("FUTURE_PRODUCT"));
    }
}
