"""Limited source/package checks. This is not a Rust compiler or test runner."""
from pathlib import Path
import json, re, sys, tomllib, zipfile
from bisect import bisect_left
root = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path(__file__).resolve().parents[1]
errors = []
RAW_STRING = re.compile(r'(?:br|cr|r)(#*)"')
CHAR_LITERAL = re.compile(r"'(?:[^'\\\n]|\\(?:u\{[0-9a-fA-F_]+\}|x[0-9a-fA-F]{2}|.))'")

def delimiters(path):
    s = path.read_text(encoding='utf-8')
    masked = list(s)
    def mask(start, end):
        masked[start:end] = ['\n' if ch == '\n' else ' ' for ch in s[start:end]]
    newlines = [match.start() for match in re.finditer('\n', s)]
    line_at = lambda position: bisect_left(newlines, position) + 1
    stack = []
    i = 0
    while i < len(s):
        if s.startswith('//', i):
            j = s.find('\n', i); end = len(s) if j < 0 else j + 1
            mask(i, end); i = end; continue
        if s.startswith('/*', i):
            start = i; depth = 1; i += 2
            while i < len(s) and depth:
                if s.startswith('/*', i): depth += 1; i += 2
                elif s.startswith('*/', i): depth -= 1; i += 2
                else: i += 1
            if depth: errors.append(f'{path}: unclosed block comment')
            mask(start, i)
            continue
        raw = RAW_STRING.match(s, i)
        if raw:
            close = '"' + raw.group(1)
            j = s.find(close, raw.end())
            if j < 0: errors.append(f'{path}: unclosed raw string'); return
            mask(i, j + len(close)); i = j + len(close); continue
        if s[i] == '"':
            start = i; i += 1
            while i < len(s):
                if s[i] == '\\': i += 2
                elif s[i] == '"': i += 1; break
                else: i += 1
            else: errors.append(f'{path}: unclosed string')
            mask(start, i)
            continue
        if s[i] == "'":
            char = CHAR_LITERAL.match(s, i)
            if char: mask(i, char.end()); i = char.end(); continue
        ch = s[i]
        if ch in '([{': stack.append((ch, line_at(i)))
        elif ch in ')]}':
            expected = dict(zip(')]}', '([{'))[ch]
            if not stack or stack[-1][0] != expected:
                errors.append(f'{path}:{line_at(i)}: unexpected {ch}'); return
            stack.pop()
        i += 1
    if stack: errors.append(f'{path}: unclosed delimiters {stack[-5:]}')
    return ''.join(masked)

sources = [path for path in (root / 'crates').rglob('*.rs') if '.reference' not in path.parts]
code_only = {path: delimiters(path) or '' for path in sources}

# WGSL reserved tokens are rejected before a graphics adapter is involved.
# This is a lexical guard, not WGSL parsing/type checking (see --check-shaders).
# https://www.w3.org/TR/WGSL/#reserved-words
WGSL_RESERVED = frozenset('''NULL Self abstract active alignas alignof as asm asm_fragment
async attribute auto await become cast catch class co_await co_return co_yield coherent
column_major common compile compile_fragment concept const_cast consteval constexpr constinit
crate debugger decltype delete demote demote_to_helper do dynamic_cast enum explicit export
extends extern external fallthrough filter final finally friend from fxgroup get goto
groupshared highp impl implements import inline instanceof interface layout lowp macro
macro_rules match mediump meta mod module move mut mutable namespace new nil noexcept
noinline nointerpolation non_coherent noncoherent noperspective null nullptr of operator
package packoffset partition pass patch pixelfragment precise precision premerge priv
protected pub public readonly ref regardless register reinterpret_cast require resource
restrict self set shared sizeof smooth snorm static static_assert static_cast std subroutine
super target template this thread_local throw trait try type typedef typeid typename typeof
union unless unorm unsafe unsized use using varying virtual volatile wgsl where with
writeonly yield'''.split())
wgsl_sources = sorted((root / 'crates').rglob('*.wgsl'))
for path in wgsl_sources:
    code = delimiters(path) or ''
    for token in re.finditer(r'\b[A-Za-z_][A-Za-z_0-9]*\b', code):
        if token[0] in WGSL_RESERVED:
            line = code.count('\n', 0, token.start()) + 1
            errors.append(f'{path}:{line}: reserved WGSL word {token[0]!r}')

# Narrow namespace guards for the reported E0433 regressions. Importing types
# does not import their containing module. These do not resolve Rust scopes or
# types; parent glob imports are accepted conservatively.
for path, code in code_only.items():
    if path.parent != root / 'crates/app/src': continue
    uses = re.findall(r'\buse\s+([^;]+);', code)
    body = re.sub(r'\buse\s+[^;]+;', lambda m: ' ' * len(m[0]), code)
    for module, qualified in [('ui', 'crate::ui'), ('empty_controls', 'crate::app::empty_controls')]:
        bare = re.search(r'(?<![\w:])' + module + r'::\w+', body)
        if not bare: continue
        imported = bool(re.search(r'\bmod\s+' + module + r'\s*;', code))
        for use in uses:
            imported |= bool(
                re.fullmatch(r'\s*(?:\w+::)*' + module + r'\s*', use)
                or re.search(r'\b' + module + r'\s*::\s*\{[^{}]*\bself\s*(?:,|\})', use)
                or re.search(r'(?:\{|,)\s*' + module + r'\s*(?:,|\})', use)
                or re.search(r'\bas\s+' + module + r'\b', use)
                or re.fullmatch(r'\s*(?:super|crate::app)::\*\s*', use)
            )
        if not imported:
            line = code.count('\n', 0, bare.start()) + 1
            errors.append(f'{path}:{line}: {module}:: is used without importing the {module} module; use {qualified}:: or import {qualified}')

# Restrict this ordering check to Action::hint, with literals/comments masked.
# A catch-all dropdown hint must follow every specialized dropdown match arm.
ui_path = root / 'crates/app/src/ui.rs'
ui_code = code_only.get(ui_path, '')
tooltip = re.search(r'\bfn\s+hint\s*\([^)]*\)[^{]*\{', ui_code)
if tooltip:
    start = tooltip.end(); end = start; depth = 1
    while end < len(ui_code) and depth:
        depth += (ui_code[end] == '{') - (ui_code[end] == '}')
        end += 1
    hints = ui_code[start:end]
    fallback = re.search(r'Self::DropdownOpen\(\s*_\s*\)\s*=>', hints)
    if fallback:
        for arm in re.finditer(r'Self::DropdownOpen\(\s*DropdownKind::', hints[fallback.end():]):
            line = ui_code.count('\n', 0, start + fallback.end() + arm.start()) + 1
            errors.append(f'{ui_path}:{line}: unreachable dropdown tooltip after Self::DropdownOpen(_) fallback')


# Explicit guard for the 0.47.15 empty-controls import regression. Its parent
# imports Vec3 but does not export either Vec2 or the crate-root gizmo module.
# This is intentionally narrow and is not a substitute for cargo check.
empty_controls = root / 'crates/app/src/empty_controls.rs'
if empty_controls in code_only:
    code = code_only[empty_controls]
    body = re.sub(r'\buse\s+[^;]+;', lambda m: ' ' * len(m[0]), code)
    for symbol, namespace in [('Vec2', 'glam'), ('gizmo', 'crate')]:
        if re.search(r'(?<![\w:])' + symbol + r'::', body):
            direct = re.search(r'\buse\s+' + namespace + r'::(?:' + symbol +
                              r'\s*;|\{[^{}]*\b' + symbol + r'\b[^{}]*\}\s*;)', code)
            if not direct:
                errors.append(f'{empty_controls}: explicit {namespace}::{symbol} import is missing (E0433 regression)')


# Cargo's --locked build rejects a path package whose manifest version does not
# occur in Cargo.lock. This validates metadata only, not dependency resolution.
workspace_packages = []
path_lock_entries = []
try:
    workspace_manifest = tomllib.loads((root / 'Cargo.toml').read_text())
    lock = tomllib.loads((root / 'Cargo.lock').read_text())
    workspace = workspace_manifest.get('workspace', {})
    inherited = workspace.get('package', {})
    path_lock_entries = [package for package in lock.get('package', []) if 'source' not in package]
    for member in workspace.get('members', []):
        directories = [path for path in root.glob(member) if path.is_dir()]
        if not directories:
            errors.append(f'workspace member does not exist: {member}')
        for directory in directories:
            manifest_path = directory / 'Cargo.toml'
            package = tomllib.loads(manifest_path.read_text()).get('package', {})
            name = package.get('name')
            version = package.get('version')
            if isinstance(version, dict) and version.get('workspace') is True:
                version = inherited.get('version')
            if not isinstance(name, str) or not isinstance(version, str):
                errors.append(f'{manifest_path}: missing package name or resolved version')
                continue
            matching = [entry for entry in path_lock_entries if entry.get('name') == name]
            if len(matching) != 1 or matching[0].get('version') != version:
                actual = [entry.get('version') for entry in matching]
                errors.append(f'{name}: manifest version {version!r} does not match path package in Cargo.lock: {actual!r}')
            workspace_packages.append({'name': name, 'version': version})
except (OSError, ValueError) as error:
    errors.append(f'Cargo metadata: {error}')

# A transitive dependency is not available by name inside another Rust crate.
# This narrow check catches the serde trait bound regression from the Windows log.
for manifest_path in (root / 'crates').glob('*/Cargo.toml'):
    try:
        manifest = tomllib.loads(manifest_path.read_text())
        source_dir = manifest_path.parent / 'src'
        uses_serde = any(re.search(r'\bserde::', path.read_text()) for path in source_dir.rglob('*.rs'))
        if uses_serde:
            name = manifest.get('package', {}).get('name', '')
            if 'serde' not in manifest.get('dependencies', {}):
                errors.append(f'{manifest_path}: serde:: is used without a direct serde dependency')
            entry = next((p for p in path_lock_entries if p.get('name') == name), {})
            if not any(dep.split()[0] == 'serde' for dep in entry.get('dependencies', [])):
                errors.append(f'{manifest_path}: direct serde dependency is missing from Cargo.lock')
    except (OSError, ValueError) as error:
        errors.append(f'{manifest_path}: {error}')

# Literal compile-time resources must be shipped next to their Rust source.
# Dynamic concat!/env! forms are recorded separately rather than guessed.
resources_checked = []
unresolved_resources = []
modules_checked = 0
for path in sources:
    source = path.read_text(encoding='utf-8')
    for declaration in re.finditer(r'(?m)^\s*(?:#\[path\s*=\s*"([^"\\]+)"\]\s*)?(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*;', source):
        explicit, module = declaration.groups()
        if explicit:
            candidates = [path.parent / explicit]
        else:
            base = path.parent if path.name in {'lib.rs', 'main.rs', 'mod.rs'} else path.parent / path.stem
            candidates = [base / f'{module}.rs', base / module / 'mod.rs']
        if not any(candidate.is_file() for candidate in candidates):
            errors.append(f'{path}: source module {module!r} is missing')
        modules_checked += 1
    for match in re.finditer(r'\b(include_str|include_bytes)!\s*\(\s*([^\n]*)', source):
        literal = re.match(r'"([^"\\]*)"\s*\)', match.group(2))
        if not literal:
            unresolved_resources.append(f'{path.relative_to(root)}:{source.count(chr(10), 0, match.start()) + 1}')
            continue
        resource = (path.parent / literal.group(1)).resolve()
        if not resource.is_relative_to(root):
            errors.append(f'{path}: compile-time resource escapes package root: {literal.group(1)}')
        elif not resource.is_file():
            errors.append(f'{path}: missing compile-time resource: {literal.group(1)}')
        else:
            resources_checked.append(str(resource.relative_to(root)))
            if match.group(1) == 'include_str':
                try:
                    resource.read_text(encoding='utf-8')
                except (OSError, UnicodeError) as error:
                    errors.append(f'{resource}: include_str resource is not readable UTF-8: {error}')
projects = list((root / 'examples').glob('*.poseforge')) + list((root / 'examples').glob('*.artforge'))
for path in projects:
    try: json.loads(path.read_text())
    except (ValueError, OSError) as e: errors.append(f'{path}: {e}')
ui = (root / 'crates/app/src/ui.rs').read_text()
match = re.search(r'pub enum Action\s*\{(.*?)\n\}', ui, re.S)
if match:
    variants = set(re.findall(r'^\s*(\w+)(?:\([^\n]*\))?,?\s*$', match.group(1), re.M))
    action_impl = re.search(r'impl Action\s*\{(.*?)\n\}', ui, re.S)
    if action_impl:
        variants.update(re.findall(r'\b(?:pub\s+)?fn\s+(\w+)\s*\(', action_impl.group(1)))
    for path in (root / 'crates/app/src').glob('*.rs'):
        for name in set(re.findall(r'\bAction::(\w+)', path.read_text())):
            if name not in variants: errors.append(f'{path}: unknown Action::{name}')
archive_result = None
if len(sys.argv) > 2:
    archive = Path(sys.argv[2])
    with zipfile.ZipFile(archive) as z:
        bad_crc = z.testzip()
        if bad_crc: errors.append(f'zip CRC failed: {bad_crc}')
        names = set(z.namelist())
        for name in names:
            normalized = name.replace('\\', '/')
            if normalized.startswith('/') or '..' in normalized.split('/') or re.match(r'^[A-Za-z]:', normalized):
                errors.append(f'unsafe archive name: {name}')
        # A source ZIP may use a containing directory. Detect its workspace
        # manifest rather than assuming the archive's filename is its root.
        workspace_roots = []
        for name in names:
            if name == 'Cargo.toml' or name.endswith('/Cargo.toml'):
                try:
                    manifest = tomllib.loads(z.read(name).decode('utf-8'))
                    if 'workspace' in manifest:
                        workspace_roots.append(name[:-len('Cargo.toml')])
                except (UnicodeError, ValueError):
                    errors.append(f'cannot decode archived Cargo manifest: {name}')
        for prefix in workspace_roots:
            for resource in resources_checked:
                if prefix + resource not in names:
                    errors.append(f'source archive omits compile-time resource: {prefix + resource}')
        archive_result = {'file':str(archive), 'entries':len(z.namelist()), 'crc_ok':bad_crc is None}
result = {'kind':'static_source_audit', 'rust_compilation':'not_run_by_static_audit', 'gui_validation':'not_run_by_static_audit', 'rust_files_scanned':len(sources), 'wgsl_files_scanned':len(wgsl_sources), 'source_modules_checked':modules_checked, 'workspace_packages':workspace_packages, 'compile_time_resources_checked':resources_checked, 'compile_time_resources_unresolved':unresolved_resources, 'example_json_files_decoded':len(projects), 'archive':archive_result, 'errors':errors}
print(json.dumps(result,ensure_ascii=False,indent=2))
raise SystemExit(bool(errors))
