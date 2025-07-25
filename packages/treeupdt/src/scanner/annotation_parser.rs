use crate::types::Annotation;
use regex::Regex;
use std::collections::HashMap;

/// Parse treeupdt annotations from comments
pub fn parse_annotation(comment: &str, line_number: usize) -> Option<Annotation> {
    // Look for patterns like:
    // # treeupdt: key=value, key2=value2
    // // treeupdt: key=value
    // /* treeupdt: key=value */
    
    let annotation_re = Regex::new(r"treeupdt:\s*(.+)").unwrap();
    
    if let Some(captures) = annotation_re.captures(comment) {
        let options_str = captures.get(1).unwrap().as_str();
        let mut options = HashMap::new();
        
        // Parse key=value pairs, handling quoted values
        let mut current_key = String::new();
        let mut current_value = String::new();
        let mut in_value = false;
        let mut in_quotes = false;
        let mut quote_char = ' ';
        
        for ch in options_str.chars() {
            match ch {
                '"' | '\'' if !in_quotes => {
                    in_quotes = true;
                    quote_char = ch;
                }
                '"' | '\'' if in_quotes && ch == quote_char => {
                    in_quotes = false;
                }
                '=' if !in_quotes && !in_value => {
                    in_value = true;
                }
                ',' if !in_quotes => {
                    // End of current pair
                    if !current_key.is_empty() {
                        if in_value && !current_value.is_empty() {
                            options.insert(current_key.trim().to_string(), current_value.trim().to_string());
                        } else if !in_value {
                            // Boolean flag
                            options.insert(current_key.trim().to_string(), "true".to_string());
                        }
                    }
                    current_key.clear();
                    current_value.clear();
                    in_value = false;
                }
                _ => {
                    if in_value {
                        current_value.push(ch);
                    } else {
                        current_key.push(ch);
                    }
                }
            }
        }
        
        // Handle the last pair
        if !current_key.is_empty() {
            if in_value && !current_value.is_empty() {
                options.insert(current_key.trim().to_string(), current_value.trim().to_string());
            } else if !in_value {
                // Boolean flag
                options.insert(current_key.trim().to_string(), "true".to_string());
            }
        }
        
        if !options.is_empty() {
            return Some(Annotation {
                line: line_number,
                options,
            });
        }
    }
    
    None
}

/// Extract annotation from a line that might contain both code and a comment
pub fn extract_annotation_from_line(line: &str, line_number: usize) -> Option<Annotation> {
    // Handle different comment styles
    let comment_markers = [
        ("#", None),           // Shell, TOML, Python
        ("//", None),          // Rust, Go, JS
        ("--", None),          // SQL, Haskell
        ("/*", Some("*/")),    // C-style block comments
    ];
    
    for (start, end) in &comment_markers {
        if let Some(comment_start) = line.find(start) {
            let comment = if let Some(end_marker) = end {
                // Block comment
                if let Some(comment_end) = line.find(end_marker) {
                    &line[comment_start + start.len()..comment_end]
                } else {
                    continue;
                }
            } else {
                // Line comment - everything after the marker
                &line[comment_start + start.len()..]
            };
            
            if let Some(annotation) = parse_annotation(comment, line_number) {
                return Some(annotation);
            }
        }
    }
    
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_annotation() {
        let ann = parse_annotation("treeupdt: pin-version=1.0", 10).unwrap();
        assert_eq!(ann.line, 10);
        assert_eq!(ann.options.get("pin-version").unwrap(), "1.0");
    }
    
    #[test]
    fn test_parse_multiple_options() {
        let ann = parse_annotation("treeupdt: pin-version=1.0, update-strategy=conservative", 5).unwrap();
        assert_eq!(ann.options.get("pin-version").unwrap(), "1.0");
        assert_eq!(ann.options.get("update-strategy").unwrap(), "conservative");
    }
    
    #[test]
    fn test_parse_boolean_flag() {
        let ann = parse_annotation("treeupdt: ignore", 1).unwrap();
        assert_eq!(ann.options.get("ignore").unwrap(), "true");
    }
    
    #[test]
    fn test_extract_from_toml_line() {
        let line = r#"serde = "1.0"  # treeupdt: pin-version=1.0"#;
        let ann = extract_annotation_from_line(line, 15).unwrap();
        assert_eq!(ann.options.get("pin-version").unwrap(), "1.0");
    }
    
    #[test]
    fn test_extract_from_rust_line() {
        let line = r#"let version = "1.0"; // treeupdt: ignore"#;
        let ann = extract_annotation_from_line(line, 20).unwrap();
        assert_eq!(ann.options.get("ignore").unwrap(), "true");
    }
    
    #[test]
    fn test_quoted_values() {
        let ann = parse_annotation(r#"treeupdt: ignore-versions="*-beta*,*-rc*""#, 1).unwrap();
        assert_eq!(ann.options.get("ignore-versions").unwrap(), "*-beta*,*-rc*");
    }
}