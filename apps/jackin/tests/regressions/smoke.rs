use jackin_console::domain::instance::InstanceId;

#[test]
fn instance_row_keys_stay_distinct_when_names_contain_the_separator() {
    let left = InstanceId::new("a:b", "c");
    let right = InstanceId::new("a", "b:c");

    assert_ne!(left.row_key(), right.row_key());
}
