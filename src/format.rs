use crate::ConfigPatchError;
use serde_json::Value;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Format {
    Json,
    Yaml,
    Toml,
}

pub fn detect(path: &Path) -> Result<Format, ConfigPatchError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .ok_or_else(|| {
            ConfigPatchError::UnsupportedFormat(format!(
                "No file extension found: {}",
                path.display()
            ))
        })?;

    match ext.as_str() {
        "json" => Ok(Format::Json),
        "yaml" | "yml" => Ok(Format::Yaml),
        "toml" => Ok(Format::Toml),
        _ => Err(ConfigPatchError::UnsupportedFormat(format!(
            "Unsupported file extension: .{ext}"
        ))),
    }
}

pub fn parse(content: &str, path: &Path) -> Result<Value, ConfigPatchError> {
    let format = detect(path)?;

    match format {
        Format::Json => serde_json::from_str(content).map_err(|e| ConfigPatchError::ParseError {
            path: path.to_path_buf(),
            source: Box::new(e),
        }),
        Format::Yaml => {
            let yaml_value: serde_yaml::Value =
                serde_yaml::from_str(content).map_err(|e| ConfigPatchError::ParseError {
                    path: path.to_path_buf(),
                    source: Box::new(e),
                })?;
            serde_json::to_value(yaml_value).map_err(|e| ConfigPatchError::ParseError {
                path: path.to_path_buf(),
                source: Box::new(e),
            })
        }
        Format::Toml => {
            let toml_value: toml::Value =
                toml::from_str(content).map_err(|e| ConfigPatchError::ParseError {
                    path: path.to_path_buf(),
                    source: Box::new(e),
                })?;
            toml_to_json(toml_value).map_err(|e| ConfigPatchError::ParseError {
                path: path.to_path_buf(),
                source: Box::new(e),
            })
        }
    }
}

pub fn serialize(value: &Value, format: Format) -> Result<String, ConfigPatchError> {
    match format {
        Format::Json => serde_json::to_string_pretty(value).map_err(|e| {
            ConfigPatchError::WriteError(std::io::Error::new(std::io::ErrorKind::Other, e))
        }),
        Format::Yaml => {
            let yaml_value = json_to_yaml(value);
            serde_yaml::to_string(&yaml_value).map_err(|e| {
                ConfigPatchError::WriteError(std::io::Error::new(std::io::ErrorKind::Other, e))
            })
        }
        Format::Toml => {
            let toml_value = json_to_toml(value);
            toml::to_string(&toml_value).map_err(|e| {
                ConfigPatchError::WriteError(std::io::Error::new(std::io::ErrorKind::Other, e))
            })
        }
    }
}

fn toml_to_json(value: toml::Value) -> serde_json::Result<Value> {
    match value {
        toml::Value::String(s) => Ok(Value::String(s)),
        toml::Value::Integer(i) => Ok(Value::Number(i.into())),
        toml::Value::Float(f) => {
            serde_json::Number::from_f64(f).map_or(Ok(Value::Null), |n| Ok(Value::Number(n)))
        }
        toml::Value::Boolean(b) => Ok(Value::Bool(b)),
        toml::Value::Datetime(dt) => Ok(Value::String(dt.to_string())),
        toml::Value::Array(arr) => {
            let json_arr: Result<Vec<_>, _> = arr.into_iter().map(toml_to_json).collect();
            Ok(Value::Array(json_arr?))
        }
        toml::Value::Table(table) => {
            let json_obj: Result<serde_json::Map<String, Value>, _> = table
                .into_iter()
                .map(|(k, v)| toml_to_json(v).map(|jv| (k, jv)))
                .collect();
            Ok(Value::Object(json_obj?))
        }
    }
}

fn json_to_yaml(value: &Value) -> serde_yaml::Value {
    match value {
        Value::Null => serde_yaml::Value::Null,
        Value::Bool(b) => serde_yaml::Value::Bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_yaml::Value::Number(i.into())
            } else if let Some(f) = n.as_f64() {
                serde_yaml::Value::Number(serde_yaml::Number::from(f))
            } else {
                serde_yaml::Value::Null
            }
        }
        Value::String(s) => serde_yaml::Value::String(s.clone()),
        Value::Array(arr) => serde_yaml::Value::Sequence(arr.iter().map(json_to_yaml).collect()),
        Value::Object(obj) => {
            let map: serde_yaml::Mapping = obj
                .iter()
                .map(|(k, v)| (serde_yaml::Value::String(k.clone()), json_to_yaml(v)))
                .collect();
            serde_yaml::Value::Mapping(map)
        }
    }
}

fn json_to_toml(value: &Value) -> toml::Value {
    match value {
        Value::Null => toml::Value::String("null".to_string()),
        Value::Bool(b) => toml::Value::Boolean(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                toml::Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                toml::Value::Float(f)
            } else {
                toml::Value::String(n.to_string())
            }
        }
        Value::String(s) => toml::Value::String(s.clone()),
        Value::Array(arr) => toml::Value::Array(arr.iter().map(json_to_toml).collect()),
        Value::Object(obj) => {
            let table: toml::map::Map<String, toml::Value> = obj
                .iter()
                .map(|(k, v)| (k.clone(), json_to_toml(v)))
                .collect();
            toml::Value::Table(table)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_detect_json() {
        assert_eq!(detect(Path::new("config.json")).unwrap(), Format::Json);
    }

    #[test]
    fn test_detect_yaml() {
        assert_eq!(detect(Path::new("config.yaml")).unwrap(), Format::Yaml);
        assert_eq!(detect(Path::new("config.yml")).unwrap(), Format::Yaml);
    }

    #[test]
    fn test_detect_toml() {
        assert_eq!(detect(Path::new("config.toml")).unwrap(), Format::Toml);
    }

    #[test]
    fn test_detect_unsupported() {
        assert!(detect(Path::new("config.xml")).is_err());
    }

    #[test]
    fn test_json_roundtrip() {
        let original = json!({"a": 1, "b": {"c": "hello"}, "d": [1, 2, 3]});
        let serialized = serialize(&original, Format::Json).unwrap();
        let parsed = parse(&serialized, Path::new("test.json")).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_yaml_roundtrip() {
        let original = json!({"a": 1, "b": {"c": "hello"}, "d": [1, 2, 3]});
        let serialized = serialize(&original, Format::Yaml).unwrap();
        let parsed = parse(&serialized, Path::new("test.yaml")).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_toml_roundtrip() {
        let original = json!({"a": 1, "b": {"c": "hello"}, "d": [1, 2, 3]});
        let serialized = serialize(&original, Format::Toml).unwrap();
        let parsed = parse(&serialized, Path::new("test.toml")).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_cross_format_json_to_yaml() {
        let original = json!({"a": 1, "b": "test"});
        let yaml_str = serialize(&original, Format::Yaml).unwrap();
        let parsed = parse(&yaml_str, Path::new("test.yaml")).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_cross_format_yaml_to_toml() {
        let original = json!({"a": 1, "b": {"c": "hello"}});
        let toml_str = serialize(&original, Format::Toml).unwrap();
        let parsed = parse(&toml_str, Path::new("test.toml")).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_null_serialization_toml() {
        let value = json!({"a": null});
        let serialized = serialize(&value, Format::Toml).unwrap();
        assert!(serialized.contains("a = \"null\""));
    }
}
