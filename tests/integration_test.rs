use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn config_patch_path() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("target");
    path.push("debug");
    path.push("config-patch");
    path
}

fn build_binary() {
    let status = Command::new("cargo")
        .arg("build")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .status()
        .expect("Failed to build binary");
    assert!(status.success(), "Build failed");
}

#[test]
fn test_json_merge() {
    build_binary();

    let temp_dir = std::env::temp_dir().join("config_patch_test_json");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let base = temp_dir.join("base.json");
    let new = temp_dir.join("new.json");
    let local = temp_dir.join("local.json");
    let output = temp_dir.join("output.json");

    fs::write(&base, r#"{"a": 1, "b": 2, "c": {"x": 10}}"#).unwrap();
    fs::write(&new, r#"{"a": 100, "d": 4}"#).unwrap();
    fs::write(&local, r#"{"b": 200, "c": {"y": 20}}"#).unwrap();

    let status = Command::new(config_patch_path())
        .arg(&base)
        .arg(&new)
        .arg(&local)
        .arg("-o")
        .arg(&output)
        .status()
        .expect("Failed to run config-patch");

    assert!(status.success(), "config-patch exited with error");

    let output_content = fs::read_to_string(&output).unwrap();
    let result: serde_json::Value = serde_json::from_str(&output_content).unwrap();

    assert_eq!(result["a"], 100);
    assert_eq!(result["b"], 200);
    assert_eq!(result["c"]["x"], 10);
    assert_eq!(result["c"]["y"], 20);
    assert_eq!(result["d"], 4);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_yaml_merge() {
    build_binary();

    let temp_dir = std::env::temp_dir().join("config_patch_test_yaml");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let base = temp_dir.join("base.yaml");
    let new = temp_dir.join("new.yaml");
    let local = temp_dir.join("local.yaml");
    let output = temp_dir.join("output.yaml");

    fs::write(
        &base,
        r#"
database:
  host: localhost
  port: 5432
  name: mydb
logging:
  level: info
"#,
    )
    .unwrap();

    fs::write(
        &new,
        r#"
database:
  host: db.production.internal
  port: 5433
features:
  - auth
  - api
"#,
    )
    .unwrap();

    fs::write(
        &local,
        r#"
database:
  name: mydb_override
logging:
  level: debug
"#,
    )
    .unwrap();

    let status = Command::new(config_patch_path())
        .arg(&base)
        .arg(&new)
        .arg(&local)
        .arg("-o")
        .arg(&output)
        .status()
        .expect("Failed to run config-patch");

    assert!(status.success(), "config-patch exited with error");

    let output_content = fs::read_to_string(&output).unwrap();
    let result: serde_yaml::Value = serde_yaml::from_str(&output_content).unwrap();

    assert_eq!(
        result["database"]["host"].as_str().unwrap(),
        "db.production.internal"
    );
    assert_eq!(result["database"]["port"].as_i64().unwrap(), 5433);
    assert_eq!(
        result["database"]["name"].as_str().unwrap(),
        "mydb_override"
    );
    assert_eq!(result["logging"]["level"].as_str().unwrap(), "debug");
    assert_eq!(result["features"][0].as_str().unwrap(), "auth");
    assert_eq!(result["features"][1].as_str().unwrap(), "api");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_toml_merge() {
    build_binary();

    let temp_dir = std::env::temp_dir().join("config_patch_test_toml");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let base = temp_dir.join("base.toml");
    let new = temp_dir.join("new.toml");
    let local = temp_dir.join("local.toml");
    let output = temp_dir.join("output.toml");

    fs::write(
        &base,
        r#"
[server]
host = "localhost"
port = 8080

[database]
url = "postgres://localhost/mydb"
"#,
    )
    .unwrap();

    fs::write(
        &new,
        r#"
[server]
host = "0.0.0.0"

[cache]
enabled = true
ttl = 300
"#,
    )
    .unwrap();

    fs::write(
        &local,
        r#"
[server]
port = 9090
"#,
    )
    .unwrap();

    let status = Command::new(config_patch_path())
        .arg(&base)
        .arg(&new)
        .arg(&local)
        .arg("-o")
        .arg(&output)
        .status()
        .expect("Failed to run config-patch");

    assert!(status.success(), "config-patch exited with error");

    let output_content = fs::read_to_string(&output).unwrap();
    let result: toml::Value = toml::from_str(&output_content).unwrap();

    assert_eq!(result["server"]["host"].as_str().unwrap(), "0.0.0.0");
    assert_eq!(result["server"]["port"].as_integer().unwrap(), 9090);
    assert_eq!(
        result["database"]["url"].as_str().unwrap(),
        "postgres://localhost/mydb"
    );
    assert_eq!(result["cache"]["enabled"].as_bool().unwrap(), true);
    assert_eq!(result["cache"]["ttl"].as_integer().unwrap(), 300);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_cross_format_merge() {
    build_binary();

    let temp_dir = std::env::temp_dir().join("config_patch_test_cross");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let base = temp_dir.join("base.json");
    let new = temp_dir.join("new.yaml");
    let local = temp_dir.join("local.toml");
    let output = temp_dir.join("output.json");

    fs::write(&base, r#"{"app": "myapp", "version": "1.0"}"#).unwrap();
    fs::write(&new, "app:\n  debug: true\n  workers: 4\n").unwrap();
    fs::write(&local, "[app]\nworkers = 8\n").unwrap();

    let status = Command::new(config_patch_path())
        .arg(&base)
        .arg(&new)
        .arg(&local)
        .arg("-o")
        .arg(&output)
        .status()
        .expect("Failed to run config-patch");

    assert!(status.success(), "config-patch exited with error");

    let output_content = fs::read_to_string(&output).unwrap();
    let result: serde_json::Value = serde_json::from_str(&output_content).unwrap();

    assert_eq!(result["version"], "1.0");
    assert_eq!(result["app"]["debug"], true);
    assert_eq!(result["app"]["workers"], 8);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_array_key_merge() {
    build_binary();

    let temp_dir = std::env::temp_dir().join("config_patch_test_array_key");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let base = temp_dir.join("base.json");
    let new = temp_dir.join("new.json");
    let local = temp_dir.join("local.json");
    let output = temp_dir.join("output.json");

    fs::write(
        &base,
        r#"{
  "containers": [
    {"name": "web", "image": "web:1.0", "port": 80},
    {"name": "worker", "image": "worker:1.0"}
  ]
}"#,
    )
    .unwrap();

    fs::write(
        &new,
        r#"{
  "containers": [
    {"name": "web", "image": "web:2.0", "env": "production"},
    {"name": "cache", "image": "redis:7"}
  ]
}"#,
    )
    .unwrap();

    fs::write(
        &local,
        r#"{
  "containers": [
    {"name": "web", "port": 8080}
  ]
}"#,
    )
    .unwrap();

    let status = Command::new(config_patch_path())
        .arg(&base)
        .arg(&new)
        .arg(&local)
        .arg("-o")
        .arg(&output)
        .status()
        .expect("Failed to run config-patch");

    assert!(status.success(), "config-patch exited with error");

    let output_content = fs::read_to_string(&output).unwrap();
    let result: serde_json::Value = serde_json::from_str(&output_content).unwrap();

    let containers = result["containers"].as_array().unwrap();
    assert_eq!(containers.len(), 3);

    let web = containers.iter().find(|c| c["name"] == "web").unwrap();
    assert_eq!(web["image"], "web:2.0");
    assert_eq!(web["port"], 8080);
    assert_eq!(web["env"], "production");

    let worker = containers.iter().find(|c| c["name"] == "worker").unwrap();
    assert_eq!(worker["image"], "worker:1.0");

    let cache = containers.iter().find(|c| c["name"] == "cache").unwrap();
    assert_eq!(cache["image"], "redis:7");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_null_removal_integration() {
    build_binary();

    let temp_dir = std::env::temp_dir().join("config_patch_test_null");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let base = temp_dir.join("base.json");
    let new = temp_dir.join("new.json");
    let local = temp_dir.join("local.json");
    let output = temp_dir.join("output.json");

    fs::write(&base, r#"{"a": 1, "b": 2, "c": 3}"#).unwrap();
    fs::write(&new, r#"{"a": 10}"#).unwrap();
    fs::write(&local, r#"{"b": null}"#).unwrap();

    let status = Command::new(config_patch_path())
        .arg(&base)
        .arg(&new)
        .arg(&local)
        .arg("-o")
        .arg(&output)
        .status()
        .expect("Failed to run config-patch");

    assert!(status.success(), "config-patch exited with error");

    let output_content = fs::read_to_string(&output).unwrap();
    let result: serde_json::Value = serde_json::from_str(&output_content).unwrap();

    assert_eq!(result["a"], 10);
    assert!(result.get("b").is_none());
    assert_eq!(result["c"], 3);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_file_not_found_error() {
    build_binary();

    let status = Command::new(config_patch_path())
        .arg("/nonexistent/base.json")
        .arg("/nonexistent/new.json")
        .arg("/nonexistent/local.json")
        .arg("-o")
        .arg("/tmp/output.json")
        .status()
        .expect("Failed to run config-patch");

    assert!(
        !status.success(),
        "Expected non-zero exit code for missing file"
    );
}

#[test]
fn test_help_output() {
    build_binary();

    let output = Command::new(config_patch_path())
        .arg("--help")
        .output()
        .expect("Failed to run config-patch --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("config-patch"));
    assert!(stdout.contains("BASE"));
    assert!(stdout.contains("NEW"));
    assert!(stdout.contains("LOCAL"));
    assert!(stdout.contains("--output"));
    assert!(stdout.contains("--array-key"));
}
