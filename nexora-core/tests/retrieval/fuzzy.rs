use nexora_core::fuzzy::score;

#[test]
fn subsequence_matches_are_scored() {
    let direct = score("sm", "school/math").unwrap();
    let weaker = score("sm", "some other note").unwrap();
    assert!(direct > weaker);
    assert!(score("alg", "algebra").unwrap() > score("alg", "x algebra").unwrap());
}

#[test]
fn non_subsequence_and_empty_query() {
    assert_eq!(score("zz", "school/math"), None);
    assert_eq!(score("", "anything"), Some(0));
    assert_eq!(score("   ", "anything"), Some(0));
}

#[test]
fn matching_is_case_insensitive() {
    assert_eq!(score("ALG", "algebra"), score("alg", "algebra"));
}
