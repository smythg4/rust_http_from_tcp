use std::collections::HashMap;
use crate::http::request::ParseError;

pub struct Headers(HashMap<String, String>);

impl Headers {

    pub fn new() -> Self {
        Headers(HashMap::new())
    }

    pub fn insert(&mut self, key: String, value: String) {
        self.0.insert(key, value);
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.0.get(&key.to_lowercase())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
    
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn is_valid_tchar(c: char) -> bool {
        match c {
            'A'..='Z' | 'a'..='z' => true,
            '0'..='9' => true,
            '!' | '#' | '$' | '%' | '&' | '\'' | '*' | '+' | '-' | '.' |
            '^' | '_' | '`' | '|' | '~' => true,
            _ => false,
        }
    }

    fn is_valid_field_name(name: &str) -> bool {
        !name.is_empty() && name.chars().all(Self::is_valid_tchar)
    }

    pub fn parse(&mut self, data: &[u8]) -> Result<(usize, bool), ParseError> {

            if let Some(line_end) = data.windows(2).position(|w| w == b"\r\n") {

                if line_end == 0 {
                    return Ok((2, true));
                }

                let line = String::from_utf8_lossy(&data[..line_end]);

                if let Some(colon_pos) = line.find(':') {
                    let field_name = &line[..colon_pos];
                    let field_value = &line[colon_pos+1..];

                    if field_name.ends_with(' ') {
                        return Err(ParseError::InvalidFormat("found space between field_name and colon".to_string()));
                    }

                    let key_trimmed = field_name.trim();

                    if !Self::is_valid_field_name(&key_trimmed) {
                        return Err(ParseError::InvalidFormat("found invalid chars within field_name".to_string()));
                    }

                    let key = field_name.trim().to_lowercase();
                    let value = field_value.trim().to_string();

                    self.0.entry(key).and_modify(|existing| {
                        *existing = format!("{}, {}", existing, value);
                    }).or_insert(value);

                    Ok((line_end+2, false))
                } else {
                    Err(ParseError::InvalidFormat("no colon found".to_string()))
                }
            } else {
                Ok((0, false))
            }

    }

}

impl std::fmt::Display for Headers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}",
            self.0.iter().map(|(k,v)| { format!("{}: {}", k, v) })
            .collect::<Vec<_>>()
            .join("\r\n"))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_valid_single_header() {
        let mut headers = Headers::new();
        let data = "Host: localhost:42069\r\n\r\n".as_bytes();

        let (n, done) = headers.parse(data).unwrap();
        assert!(!headers.is_empty());
        assert_eq!("localhost:42069", headers.get("host").unwrap());
        assert_eq!(23, n);
        assert!(!done);
    }

    #[test]
    fn test_headers_invalid_spacing() {
        let mut headers = Headers::new();
        let data = "       Host : localhost:42069       \r\n\r\n".as_bytes();

        let result = headers.parse(data);

        assert!(result.is_err());

        if let Err(ParseError::InvalidFormat(msg)) = result {
            assert!(msg.contains("found space"));
        } else {
            panic!("Expected compilation error");
        }
    }

    #[test]
    fn test_valid_single_header_with_extra_whitespace() {
        let mut headers = Headers::new();
        let data = b"          Host: localhost:42069    \r\n\r\n";

        let (n, _done) = headers.parse(data).unwrap();
        assert_eq!("localhost:42069", headers.get("host").unwrap());
        assert_eq!(37, n);

    }

    #[test]
    fn test_valid_done() {
        let mut headers = Headers::new();
        let data = b"\r\n";

        let (n, done) = headers.parse(data).unwrap();

        assert_eq!(2, n);
        assert!(done);
    }

    #[test]
    fn test_valid_2_headers_with_existing_headers() {
        let mut headers = Headers::new();
        headers.insert("existing".to_string(), "value".to_string());
        
        let data = b"Host: localhost:42069\r\n";
        let (_n, done) = headers.parse(data).unwrap();
        assert_eq!(2, headers.len()); // Should have both existing and new header
        assert!(!done);
    }

    #[test]
    fn test_invalid_chars_in_header() {
        let mut headers = Headers::new();
        let data = b"H@st: localhost:42069\r\n";
        
        let result = headers.parse(data);
        assert!(result.is_err());
        
        if let Err(ParseError::InvalidFormat(msg)) = result {
            assert!(msg.contains("invalid chars"));
        } else {
            panic!("Expected InvalidFormat error");
    }
    }

    #[test]
    fn test_valid_special_characters() {
        let mut headers = Headers::new();
        let data = b"x-custom-header!#$%&'*+-.^_`|~123: value\r\n\r\n";
        
        let (_n, done) = headers.parse(data).unwrap();
        assert_eq!("value", headers.get("x-custom-header!#$%&'*+-.^_`|~123").unwrap());
        assert!(!done);
    }
    #[test]
    fn test_multiple_values_for_same_header() {
        let mut headers = Headers::new();
        // Start with an existing header
        headers.insert("set-person".to_string(), "lane-loves-go".to_string());
        
        // Parse another header with the same key
        let data = b"Set-Person: prime-loves-zig\r\n";
        let (_n, done) = headers.parse(data).unwrap();
        
        // Should combine with comma separator
        assert_eq!("lane-loves-go, prime-loves-zig", headers.get("set-person").unwrap());
        assert!(!done);
    }

    #[test]
    fn test_multiple_header_parsing_calls() {
        let mut headers = Headers::new();
        
        // Parse first Set-Person header
        let data1 = b"Set-Person: lane-loves-go\r\n";
        let (_n1, done1) = headers.parse(data1).unwrap();
        assert_eq!("lane-loves-go", headers.get("set-person").unwrap());
        assert!(!done1);
        
        // Parse second Set-Person header  
        let data2 = b"Set-Person: prime-loves-zig\r\n";
        let (_n2, done2) = headers.parse(data2).unwrap();
        assert_eq!("lane-loves-go, prime-loves-zig", headers.get("set-person").unwrap());
        assert!(!done2);
        
        // Parse third Set-Person header
        let data3 = b"Set-Person: tj-loves-ocaml\r\n";
        let (_n3, done3) = headers.parse(data3).unwrap();
        assert_eq!("lane-loves-go, prime-loves-zig, tj-loves-ocaml", headers.get("set-person").unwrap());
        assert!(!done3);
    }
    
}