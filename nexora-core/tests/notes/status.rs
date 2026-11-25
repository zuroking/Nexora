use nexora_core::status::saved_label;

#[test]
fn labels_relative_age() {
    assert_eq!(saved_label(0), "just now");
    assert_eq!(saved_label(1), "just now");
    assert_eq!(saved_label(12), "12s ago");
    assert_eq!(saved_label(60), "1m ago");
    assert_eq!(saved_label(184), "3m 4s ago");
}
