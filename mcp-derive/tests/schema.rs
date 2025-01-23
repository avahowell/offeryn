#[test]
fn test_schema() {
    let t = trybuild::TestCases::new();
    t.pass("tests/schema/01-basic.rs");
} 