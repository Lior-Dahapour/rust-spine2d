use spine2d_core::json::{parse, reference_number, JsonExt};

#[test]
fn objects_keep_order_and_lookup_is_case_insensitive() {
    let j = parse(r#"{"b":1,"A":2,"c":{"x":[1,2,3]}}"#).unwrap();
    let keys: Vec<_> = j.children().map(|(k, _)| k).collect();
    assert_eq!(keys, ["b", "A", "c"]);
    assert_eq!(j.int("a", 0), 2);
    assert_eq!(j.get_ci("c").unwrap().get_ci("x").unwrap().child_count(), 3);
}

#[test]
fn numbers_follow_the_reference_algorithm() {
    let j = parse(r#"[1, -2.5, 3e2, 4.5E-1, 7.9]"#).unwrap();
    let v: Vec<_> = j
        .children()
        .map(|(_, n)| (n.as_float(), n.as_int()))
        .collect();
    assert_eq!(v[0], (1.0, 1));
    assert_eq!(v[1], (-2.5, -2)); // int truncates toward zero
    assert_eq!(v[2], (300.0, 300));
    assert_eq!(v[3], (0.45, 0));
    assert_eq!(v[4], (7.9, 7));
    // The reference algorithm is not correctly rounded; the text drives the result.
    assert_eq!(reference_number("0.1").0, 0.1f32);
}

#[test]
fn accessor_defaults_and_odd_types_match_reference() {
    let j = parse(r#"{"s":"hi","n":3,"t":true,"nul":null,"str":"true","arr":[1]}"#).unwrap();
    assert_eq!(j.str("s"), Some("hi"));
    assert_eq!(j.str("n"), None);
    assert_eq!(j.float("missing", 9.0), 9.0);
    assert_eq!(j.float("s", 9.0), 0.0); // present but not a number
    assert!(j.boolean("t", false));
    assert!(!j.boolean("nul", true));
    assert!(j.boolean("str", false));
    assert!(j.boolean("arr", true)); // containers fall back to the default
    assert_eq!(j.int("t", 0), 1);
}

#[test]
fn invalid_documents_are_errors() {
    assert!(parse("{").is_err());
    assert!(parse("[1,").is_err());
    assert!(parse(r#"{"a" 1}"#).is_err());
}
