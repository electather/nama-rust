use super::rounded;

#[test]
fn a_count_is_rounded_down_to_a_power_of_ten() {
    let counts = [0, 1, 9, 10, 1234, -5].map(rounded);

    assert_eq!(counts, [0, 1, 1, 10, 1000, 0]);
}
