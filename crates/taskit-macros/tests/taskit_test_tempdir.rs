use std::path::Path;
use taskit_macros::taskit_test;

#[taskit_test(tempdir)]
fn tempdir_sets_cwd(dir: &Path) {
    assert!(dir.exists());
    assert!(dir.is_dir());
    std::fs::write("marker.txt", "hello").unwrap();
    assert!(Path::new("marker.txt").exists());
}

#[taskit_test(tempdir)]
fn tempdir_is_isolated(dir: &Path) {
    // Files from other tests should not be visible
    assert!(!Path::new("marker.txt").exists());
    let _ = dir;
}
