#[test]
fn test_schema() {
    let t = trybuild::TestCases::new();
    t.pass("tests/schema/01-basic.rs");
    t.pass("tests/schema/02-doc-comments.rs");
    t.pass("tests/schema/03-stateful.rs");
} 