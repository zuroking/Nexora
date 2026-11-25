use nexora_core::tabs::cycle;

#[test]
fn no_tabs_has_no_next_or_previous() {
    assert_eq!(cycle(0, 0, false), None);
    assert_eq!(cycle(0, 0, true), None);
}

#[test]
fn single_tab_stays_selected() {
    assert_eq!(cycle(0, 1, false), Some(0));
    assert_eq!(cycle(0, 1, true), Some(0));
}

#[test]
fn forward_wraps_after_the_last_tab() {
    assert_eq!(cycle(0, 3, false), Some(1));
    assert_eq!(cycle(1, 3, false), Some(2));
    assert_eq!(cycle(2, 3, false), Some(0));
}

#[test]
fn backward_wraps_before_the_first_tab() {
    assert_eq!(cycle(2, 3, true), Some(1));
    assert_eq!(cycle(1, 3, true), Some(0));
    assert_eq!(cycle(0, 3, true), Some(2));
}

#[test]
fn out_of_range_active_is_clamped() {
    assert_eq!(cycle(9, 3, false), Some(0));
    assert_eq!(cycle(9, 3, true), Some(1));
}
