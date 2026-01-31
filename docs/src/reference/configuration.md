# Configuration

fmm is configured via `.fmmrc.json` in your project root. All fields are optional — sensible defaults are used when the file is missing or fields are omitted.

## Default configuration

```json
{
  "format": "yaml",
  "include_loc": true,
  "max_file_size": 500,
  "languages": [
    "ts", "tsx", "js", "jsx",
    "py",
    "rs",
    "go",
    "java",
    "cpp", "cc", "cxx", "hpp", "h",
    "cs",
    "rb"
  ]
}
```

## Fields

### `format`

**Type:** `"yaml"` | `"json"`
**Default:** `"yaml"`

Output format for sidecar files. YAML is recommended — it's more compact and readable.

### `include_loc`

**Type:** `boolean`
**Default:** `true`

Whether to include line-of-code counts in sidecars. Useful for finding large files with `fmm search --loc ">500"`.

### `max_file_size`

**Type:** `integer` (KB)
**Default:** `500`

Maximum file size in kilobytes to process. Files larger than this are skipped. Set to `0` to disable the limit.

### `languages`

**Type:** `string[]`
**Default:** all supported extensions

List of file extensions to process. Only files matching these extensions will have sidecars generated.

## Ignore files

### `.fmmignore`

Create a `.fmmignore` file (same syntax as `.gitignore`) to exclude files from sidecar generation:

```
# Skip generated code
src/generated/
*.generated.ts

# Skip vendor directories
vendor/
third_party/
```

### `.gitignore`

fmm automatically respects `.gitignore` rules. Files ignored by git are also ignored by fmm.

## MCP server configuration

The MCP server is configured in `.mcp.json`:

```json
{
  "mcpServers": {
    "fmm": {
      "command": "fmm",
      "args": ["serve"]
    }
  }
}
```

Run `fmm init --mcp` to generate this automatically.

## Claude Code skill

The Claude Code skill is installed at `.claude/skills/fmm-navigate.md`. It teaches Claude how to navigate using fmm sidecars.

Run `fmm init --skill` to install or update it.
