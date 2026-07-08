use taskit_macros::taskit_test;

#[taskit_test]
fn bare_is_just_a_test() {
    assert_eq!(2 + 2, 4);
}
