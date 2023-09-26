use serde_json::Value;

pub(crate) fn num_val(val: &Value) -> f32 {
    (if val.is_number() { val.as_f64().unwrap_or_default() } else { val.as_str().unwrap_or_default().parse().unwrap_or_default() }) as f32
}

pub(crate) fn bool_val(val: &Value) -> bool {
    match val {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().unwrap_or_default() != 0.0,
        Value::String(s) => s == "true",
        Value::Array(a) => a.len() > 0,
        Value::Object(_) => true, 
    }
}

pub(crate) fn str_val(val: &Value) -> String {
    match val {
        Value::Bool(b) => if *b { "true".to_string() } else  { false.to_string() },
        Value::Number(n) => n.as_f64().unwrap_or_default().to_string(),
        Value::String(s) => s.clone(),
        Value::Array(a) => if a.len() > 0 { str_val( &a[0]) } else { String::new() },
        _ => String::new(),
    }
}

pub fn bytes_to_hex_string(bytes: &[u8]) -> String {
    let mut result = String::new();

    for i in 0..bytes.len() {
        result += &format!("{:02x}", bytes[i]);
    }
    
    result
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