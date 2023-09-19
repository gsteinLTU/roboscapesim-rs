use serde_json::Value;

pub(crate) fn num_val(val: &Value) -> f32 {
    (if val.is_number() { val.as_f64().unwrap() } else { val.as_str().unwrap().parse().unwrap() }) as f32
}

pub(crate) fn bool_val(val: &Value) -> bool {
    match val {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().unwrap() != 0.0,
        Value::String(s) => s == "true",
        Value::Array(a) => a.len() > 0,
        Value::Object(_) => true, 
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