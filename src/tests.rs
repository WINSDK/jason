#![cfg(test)]

#[test]
#[should_panic]
fn empty() {
    super::parse("").unwrap();
}

#[test]
fn object() {
    super::parse("{}").unwrap();
}

#[test]
fn object_with_spacing() {
    super::parse("{      }").unwrap();
}

#[test]
fn object_with_multiple_keys() {
    super::parse(r#"{ "a": 10, "b": 1.0     }"#).unwrap();
    super::parse(r#"{ "a": 10, "b": "value"     }"#).unwrap();
    super::parse(r#"{ "a": 10, "b": {} }"#).unwrap();
}

#[test]
#[should_panic]
fn object_with_invalid_multiple_keys() {
    super::parse(r#"{ "a": 10 "b": "value" }"#).unwrap();
    super::parse(r#"{ "a": 10, "b": "value", }"#).unwrap();
}

#[test]
#[should_panic]
fn object_with_invalid_key() {
    super::parse(r#"{ "key: 10 }"#).unwrap();
}

#[test]
#[should_panic]
fn object_with_invalid_value() {
    super::parse(r#"{ "key"   : value }"#).unwrap();
}

#[test]
fn int() {
    super::parse(r#"{ "key": 10 }"#).unwrap();
}

#[test]
fn fraction() {
    super::parse(r#"{ "key": 10.3 }"#).unwrap();
}

#[test]
fn newline() {
    super::parse(r#"{ "key": [
        1,
        2,
        3
    ] }"#).unwrap();
    super::parse(r#"{ "key": 
                             10.3 }"#).unwrap();
}

#[test]
#[should_panic]
fn large_fraction() {
    super::parse(r#"{ "key": 1213828382340213482838234 }"#).unwrap();
    super::parse(r#"{ "key": 10.333333333333333333333333333333 }"#).unwrap();
    super::parse(r#"{ "key": 10E+200000000000000000000 }"#).unwrap();
}

#[test]
fn scientific_notation() {
    super::parse(r#"{ "key": 10.3e+10 }"#).unwrap();
    super::parse(r#"{ "key": 10.3E+10 }"#).unwrap();
    super::parse(r#"{ "key": 10E+10 }"#).unwrap();
    super::parse(r#"{ "key": 10E-10 }"#).unwrap();
    super::parse(r#"{ "key": 10e-10 }"#).unwrap();
    super::parse(r#"{ "key": 10e+10 }"#).unwrap();
    super::parse(r#"{ "key": 10.123e+10 }"#).unwrap();
}

#[test]
fn array() {
    super::parse(r#"[ 10.3e+10, "value", {} ]"#).unwrap();
    super::parse(r#"[10.3e+10,   "value", {} ]"#).unwrap();
    super::parse(r#"[10.3e+10,   "value", {}    ]"#).unwrap();
}

#[test]
#[should_panic]
fn array_invalid_items() {
    super::parse(r#"[ 10.3e+10, "value" {} ]"#).unwrap();
    super::parse(r#"[ 10.3e+10 "value", {} ]"#).unwrap();
    super::parse(r#"[ 10.3e+10, "value", {}, ]"#).unwrap();
}
