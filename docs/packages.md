# RCA Packages

Status: implemented for `pkg init`, `pkg add`, and `pkg install` (local
paths and `http://` URLs), plus package-aware module resolution. Git
(`github:user/repo`) and HTTPS sources are not implemented yet (section 6).

## 1. Basics
- A package manifest is `rca.json` in the project root.
- Installed packages are cached under `$HOME/.rca/packages/<name>/main.rca`.
- A project imports a package with the same `import` syntax used for local
  files: `import utils` then `utils.fn(...)`.

## 2. Manifest format
    {
      "name": "my-project",
      "version": "0.1.0",
      "dependencies": {
        "utils":   "https://example.com/utils.rca",
        "helpers": "./local/helpers.rca"
      }
    }

- `name`, `version`: strings.
- `dependencies`: object mapping package name to a source string.
- Source string is one of: local path, `http://` URL (section 5).

## 3. Commands

### 3.1 rcac pkg init
Creates `rca.json` in the current directory. `name` is taken from the
directory name, `version` is `"0.1.0"`, `dependencies` starts empty.
Refuses to overwrite an existing `rca.json`.

### 3.2 rcac pkg add <name> <source>
Inserts or updates the named dependency in `rca.json`. If the name already
exists, its source is replaced (no duplicate key). Refuses without a
manifest.

### 3.3 rcac pkg install
Reads `rca.json`, fetches each dependency, and writes it to
`$HOME/.rca/packages/<name>/main.rca`. Prints per-dependency status plus a
summary line `installed N / M (F failed)`. Exit code 0 if all succeed, 1
if any fail.

## 4. Import resolution
When the compiler sees `import foo`, it looks in this order:

1. `foo.rca` in the same directory as the importing file.
2. `$HOME/.rca/packages/foo/main.rca`.

The first match wins. A project file shadows an installed package of the
same name — the same convention as most package systems.

If neither is found, the error lists both candidate paths:

    cannot find module 'foo' (looked for /proj/foo.rca and /home/u/.rca/packages/foo/main.rca)

If `HOME` is not set, the package lookup is skipped and the error lists
only the project path.

## 5. Source types (current)

| Prefix      | Behavior                                              |
|-------------|-------------------------------------------------------|
| (no prefix) | Local path. `std::fs::read_to_string` then copy.      |
| `http://`   | Fetched with `http_get` (plain HTTP, no TLS).         |
| `https://`  | Error: "https:// not yet supported (no TLS)".         |
| `github:`   | Error: "github: sources not yet supported".           |

`https://` and `github:` are reported as errors instead of silently
failing, so the manifest can be written ahead of the feature.

## 6. Not in this milestone
- `github:user/repo` sources (would use the `git` binary).
- HTTPS (needs a TLS dependency; not decided).
- Lockfile (`rca.lock`) with content hashes.
- Version constraints (`>=1.2.0`, `^1.0`) — version is recorded but not
  resolved.
- A registry server.
- Fetch caching (each `pkg install` refetches).

## 7. Example

    $ mkdir demo && cd demo
    $ rcac pkg init
    created rca.json (name: demo)

    $ cat > lib.rca <<'EOF'
    pub fn greet(n: String) -> String:
        return "hi " + n
    EOF

    $ rcac pkg add mylib ./lib.rca
    added dependency: mylib -> ./lib.rca

    $ rcac pkg install
      mylib : installed -> /home/u/.rca/packages/mylib/main.rca
    installed 1 / 1 (0 failed)

    $ cat > main.rca <<'EOF'
    import mylib
    print(mylib.greet("world"))
    EOF

    $ rcac run main.rca
    hi world

## 8. Tests
- `tests/pkg_init.rs` — init, refuse-overwrite.
- `tests/pkg_add.rs` — insert, update-no-dup, no-manifest, arity.
- `tests/pkg_install.rs` — local install, https reject, github reject,
  no-manifest, empty deps.
- `tests/pkg_loader.rs` — installed-package import, local priority,
  missing-module error, no-HOME.
