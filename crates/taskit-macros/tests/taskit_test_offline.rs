use taskit_macros::taskit_test;

#[taskit_test(offline)]
fn skipped_when_offline() {
    // This test body runs only when TASKIT_OFFLINE != "1".
    // We just verify it compiles and can run.
}
