use serde_json::Value;

/// Get number value from JSON value, with implicit conversion from other types (from string representation)
pub(crate) fn num_val(val: &Value) -> f32 {
    (if val.is_number() { val.as_f64().unwrap_or_default() } else { val.as_str().unwrap_or_default().parse().unwrap_or_default() }) as f32
}

/// Get boolean value from JSON value, with implicit conversion from other types
pub(crate) fn bool_val(val: &Value) -> bool {
    match val {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().unwrap_or_default() != 0.0,
        Value::String(s) => s == "true",
        Value::Array(a) => !a.is_empty(),
        Value::Object(_) => true, 
    }
}

/// Get string value from JSON value, with implicit conversion from other types
pub(crate) fn str_val(val: &Value) -> String {
    match val {
        Value::Bool(b) => if *b { "true".to_string() } else  { false.to_string() },
        Value::Number(n) => n.as_f64().unwrap_or_default().to_string(),
        Value::String(s) => s.clone(),
        Value::Array(a) => if !a.is_empty() { str_val( &a[0]) } else { String::new() },
        _ => String::new(),
    }
}

/// Convert bytes to hex string
pub fn bytes_to_hex_string(bytes: &[u8]) -> String {
    let mut result = String::new();

    for i in 0..bytes.len() {
        result += &format!("{:02x}", bytes[i]);
    }
    
    result
}

/// Get current timestamp in seconds
pub fn get_timestamp() -> i64 {
    let now = std::time::SystemTime::now();
    let unix_timestamp = now.duration_since(std::time::UNIX_EPOCH).unwrap();
    unix_timestamp.as_secs() as i64
}

#[test]
fn test_bytes_to_hex_string() {
    assert_eq!(bytes_to_hex_string(&[0]), "00".to_owned());
    assert_eq!(bytes_to_hex_string(&[1]), "01".to_owned());
    assert_eq!(bytes_to_hex_string(&[255]), "ff".to_owned());
    assert_eq!(bytes_to_hex_string(&[0,1]), "0001".to_owned());
    assert_eq!(bytes_to_hex_string(&[0,1,0,255,15]), "000100ff0f".to_owned());
    assert_eq!(bytes_to_hex_string(&[1,2,3,4,5]), "0102030405".to_owned());
}