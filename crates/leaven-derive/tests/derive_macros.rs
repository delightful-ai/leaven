#[test]
fn reserved_derives_fail_explicitly() {
    let suite = trybuild::TestCases::new();
    suite.compile_fail("tests/ui/reserved_derives.rs");
}
