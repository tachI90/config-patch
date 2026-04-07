# config-patch

A Rust CLI tool for deep-merging configuration files across multiple sources. Designed for Kubernetes ConfigMap merging workflows.

## Quick Start

```bash
config-patch base.json new.json local.json -o output.json
```

Merges config files in priority order (first → last). Later files override earlier ones.

## Installation

### Docker (Recommended)

```bash
docker pull ghcr.io/tachi90/config-patch:main
```

Tags available:
- `main` — latest from main branch
- `v1.0.0` — specific release
- `1.0` — minor release line
- `<sha>` — commit SHA for exact reproducibility

See all tags: https://github.com/tachI90/config-patch/pkgs/container/config-patch

### From Source

```bash
cargo install --path .
```

### Build Release

```bash
cargo build --release
# Binary at: target/release/config-patch
```

## Usage

```
config-patch <FILES...> -o <OUTPUT> [OPTIONS]

Arguments:
  <FILES...>  Configuration files to merge (in priority order, first = lowest priority)

Options:
  -o, --output <OUTPUT>    Output file path (format auto-detected from extension)
      --array-key <KEY>    Key field for smart array merging [default: name]
      --format <FORMAT>    Force output format (json, yaml, toml)
      --debug              Enable debug logging
  -h, --help               Print help
```

## Merge Behavior

### Priority Order

```
File 1 ──┐
File 2 ──┼──► merge ──► merge ──► ... ──► Output
File 3 ──┘
  ...
File N ──┘
```

1. Start with the first file (lowest priority)
2. Deep-merge each subsequent file on top
3. Later files override earlier ones at every level

### Object Merging

Nested objects are merged recursively. Overlay keys override base keys at every level.

```json
// Base
{"database": {"host": "localhost", "port": 5432, "name": "mydb"}}

// Overlay
{"database": {"host": "db.production.internal"}}

// Result
{"database": {"host": "db.production.internal", "port": 5432, "name": "mydb"}}
```

### Smart Array Merging

Arrays of objects are merged by a configurable key field (default: `"name"`). Items with matching keys are deep-merged. New items are appended. Unmatched base items are preserved.

```json
// Base
{"containers": [{"name": "web", "image": "web:1.0", "port": 80}]}

// Overlay
{"containers": [{"name": "web", "image": "web:2.0"}, {"name": "worker", "image": "worker:1.0"}]}

// Result
{"containers": [
  {"name": "web", "image": "web:2.0", "port": 80},
  {"name": "worker", "image": "worker:1.0"}
]}
```

Use `--array-key` to change the matching field:

```bash
config-patch base.json overlay.json -o out.json --array-key id
```

### Primitive Arrays

Arrays of primitives (strings, numbers, booleans) are replaced entirely by the overlay:

```json
// Base
{"tags": ["v1", "stable"]}

// Overlay
{"tags": ["v2", "beta"]}

// Result
{"tags": ["v2", "beta"]}
```

### Null Removal

A `null` value in an overlay removes that key from the output:

```json
// Base
{"debug": true, "verbose": true}

// Overlay
{"debug": null}

// Result
{"verbose": true}
```

### Type Conflicts

When the same key has incompatible types, the overlay value wins:

```json
// Base
{"config": "string"}

// Overlay
{"config": {"nested": "object"}}

// Result
{"config": {"nested": "object"}}
```

## Supported Formats

| Format | Extensions | Notes |
|--------|------------|-------|
| JSON | `.json` | Full support |
| YAML | `.yaml`, `.yml` | Single-document only |
| TOML | `.toml` | Datetime types serialized as ISO 8601 strings |

### Cross-Format Merging

Input files can be in different formats. The output format is auto-detected from the `-o` file extension:

```bash
# JSON base + YAML overlay + TOML local → JSON output
config-patch base.json overlay.yaml local.toml -o output.json

# Same inputs → YAML output
config-patch base.json overlay.yaml local.toml -o output.yaml

# Force output format regardless of extension
config-patch base.json overlay.yaml local.toml -o output.txt --format toml
```

## Logging

### Default (INFO)

Only shows high-level status:

```
INFO Merging 3 files
INFO Merge complete input_count=3 output=out.json
```

### Debug Mode (`--debug`)

Shows detailed trace of every merge operation:

```
INFO  Merging 3 files
DEBUG Reading file file=base.json step=1 total=3
DEBUG Parsing file file=base.json step=1 total=3
DEBUG Merging source into accumulated result source=1
DEBUG MERGE 'key' action="MERGE"
DEBUG ADD   'key' action="ADD"
DEBUG REMOVE 'key' action="REMOVE"
DEBUG ARRAY -> replaced action="REPLACE"
DEBUG ARRAY -> smart merge by 'name' action="SMART_MERGE"
DEBUG   MATCH 'web' action="MATCH"
DEBUG   NEW   'worker' action="NEW"
DEBUG   KEEP  'sidecar' action="KEEP"
INFO  Merge complete input_count=3 output=out.json
```

## Examples

### Using Docker

```bash
docker run --rm -v $(pwd):/config ghcr.io/tachi90/config-patch:main \
  /config/base.json /config/new.json /config/local.json \
  -o /config/output.json
```

### Kubernetes ConfigMap Merging

```yaml
initContainers:
- name: config-merge
  image: ghcr.io/tachi90/config-patch:main
  command: ["config-patch"]
  args:
    - /configmaps/base/settings.json
    - /configmaps/new/settings.json
    - /configmaps/local/settings.json
    - -o
    - /shared-config/settings.json
  volumeMounts:
    - name: base-config
      mountPath: /configmaps/base
    - name: new-config
      mountPath: /configmaps/new
    - name: local-config
      mountPath: /configmaps/local
    - name: shared-config
      mountPath: /shared-config
```

### Container List Patching

```bash
# Base deployment config
cat > base.json << 'EOF'
{
  "containers": [
    {"name": "web", "image": "myapp:1.0", "port": 80},
    {"name": "sidecar", "image": "logger:1.0"}
  ]
}
EOF

# New version bumps images and adds a container
cat > new.json << 'EOF'
{
  "containers": [
    {"name": "web", "image": "myapp:2.0"},
    {"name": "cache", "image": "redis:7"}
  ]
}
EOF

# Local overrides the web port
cat > local.json << 'EOF'
{
  "containers": [
    {"name": "web", "port": 8080}
  ]
}
EOF

config-patch base.json new.json local.json -o output.json
```

**Result:**
```json
{
  "containers": [
    {"name": "web", "image": "myapp:2.0", "port": 8080},
    {"name": "cache", "image": "redis:7"},
    {"name": "sidecar", "image": "logger:1.0"}
  ]
}
```

### Environment Variable Merging

```bash
config-patch base.yaml overlay.yaml -o output.yaml --array-key name
```

### Removing a Key

```bash
cat > base.json << 'EOF'
{"debug": true, "log_level": "info", "metrics": true}
EOF

cat > local.json << 'EOF'
{"debug": null}
EOF

config-patch base.json base.json local.json -o output.json
# Result: {"log_level": "info", "metrics": true}
```

### Merging More Than 3 Files

```bash
config-patch defaults.json env.json team.json user.json -o output.json
```

Files are merged left to right: `defaults` → `env` → `team` → `user`, with `user.json` having the highest priority.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Error (file not found, parse error, unsupported format, insufficient files) |

## Limitations

- **YAML**: Only single-document files are supported (no `---` multi-document streams)
- **TOML**: Datetime types (`Datetime`, `Date`, `Time`, `OffsetDatetime`) are serialized as ISO 8601 strings and may not round-trip with full type fidelity
- **Large files**: Entire files are loaded into memory; very large configs (>100MB) may cause issues

## License

MIT
