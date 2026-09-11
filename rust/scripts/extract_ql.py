import re, os, sys, json

root = sys.argv[1]
STR = re.compile(r'"((?:[^"\\]|\\.)*)"', re.S)
# Only literals that open with a SurrealQL top-level keyword — this is a query, not prose.
# Case-SENSITIVE: lb writes SurrealQL keywords in uppercase, so this keeps prose like "select"
# and "delete the row" out of the corpus. A second uppercase clause keyword confirms it is a
# statement and not a one-word label that happens to be spelled in caps.
HEAD = re.compile(r'^\s*(SELECT|UPDATE|UPSERT|CREATE|DELETE|RELATE|INSERT|DEFINE|REMOVE|INFO\s+FOR|LET|RETURN|BEGIN|COMMIT|CANCEL|USE|SHOW|LIVE|KILL)\b')
CLAUSE = re.compile(r'\b(FROM|WHERE|SET|CONTENT|VALUE|GROUP|ORDER|LIMIT|START|FETCH|MERGE|PATCH|RETURN|FOR|ON|TABLE|FIELD|INDEX|TYPE|SPLIT|PARALLEL|TIMEOUT|OMIT|ONLY|WITH|EXPLAIN|TRANSACTION|=|\$)')

def strip_comments(src: str) -> str:
    """Blank out `//` and `/* */` comments, keeping byte offsets so line numbers stay right.

    Prose in a doc comment can read exactly like a query — `//! ... DEFINE TABLE every name at boot
    ...` was picked up as a statement and reported as broken SurrealQL. Comments are never executed,
    so they have no business in this corpus.
    """
    out = list(src)
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        if c == '"':                      # skip over string literals, comments inside them are text
            i += 1
            while i < n and src[i] != '"':
                i += 2 if src[i] == '\\' else 1
            i += 1
        elif src.startswith('//', i):
            while i < n and src[i] != '\n':
                out[i] = ' '; i += 1
        elif src.startswith('/*', i):
            while i < n and not src.startswith('*/', i):
                if src[i] != '\n': out[i] = ' '
                i += 1
            for j in range(i, min(i + 2, n)): out[j] = ' '
            i += 2
        else:
            i += 1
    return ''.join(out)


def unescape(lit: str) -> str | None:
    out, i = [], 0
    while i < len(lit):
        c = lit[i]
        if c != '\\':
            out.append(c); i += 1; continue
        i += 1
        if i >= len(lit): return None
        e = lit[i]
        if e == '\n':                    # Rust line continuation: drop newline + leading ws
            i += 1
            while i < len(lit) and lit[i] in ' \t': i += 1
            continue
        out.append({'n':'\n','t':'\t','r':'\r','0':'\0','\\':'\\','"':'"',"'":"'"}.get(e, e))
        i += 1
    return ''.join(out)

PLACE = re.compile(r'\{[^{}]*\}')
# `format!` placeholders are usually `const X: &str = "..."` — a column list, a table name. Guessing
# a generic substitute for those produces FALSE failures: `SELECT {MEMORY_COLUMNS} ... ORDER BY
# updated_at` becomes `SELECT ph ... ORDER BY updated_at`, which SurrealDB 3 rightly rejects for
# ordering by an unselected field even though the real query selects it. So resolve the constants
# we can actually see, and fall back to guessing only for the rest.
CONST = re.compile(r'\bconst\s+([A-Z][A-Z0-9_]*)\s*:\s*&\s*(?:\'static\s+)?str\s*=\s*"((?:[^"\\]|\\.)*)"', re.S)

def collect_consts(root):
    consts = {}
    for dp, dn, fns in os.walk(root):
        dn[:] = [d for d in dn if d != 'target']
        for f in fns:
            if not f.endswith('.rs'): continue
            src = open(os.path.join(dp, f), encoding='utf-8', errors='replace').read()
            for m in CONST.finditer(src):
                name, val = m.group(1), unescape(m.group(2))
                # A name defined twice with different text is ambiguous; drop it rather than guess.
                if val is None: continue
                if name in consts and consts[name] != val:
                    consts[name] = None
                else:
                    consts.setdefault(name, val)
    return {k: v for k, v in consts.items() if v is not None}
# lb speaks THREE query languages. Only SurrealQL is parsed by this engine, so the other two must
# be kept out of the corpus or they read as false failures:
#   * `federation` and `pack/sqlite.rs` emit Postgres/SQLite DDL and DML (`CREATE TABLE t (a INT)`,
#     `?` binds, `VALUES (...)`) for the mirror/export targets;
#   * `prql` compiles a different language entirely.
# The generated test is itself full of SurrealQL literals; scanning it would double the corpus
# and make the count grow on every regeneration.
FOREIGN_PATHS = ('/federation/', '/prql/', '/pack/sqlite.rs', '/pack/postgres.rs',
                 # `rules` builds plans in TWO dialects: SurrealQL for platform grids and ANSI/
                 # DataFusion (window functions, INTERVAL) for federation grids. Its own module doc
                 # says so. A SurrealQL parser rightly rejects the ANSI half.
                 '/rules/src/verbs/',
                 # Generated from this corpus; scanning either would double the count each run.
                 '/tests/surrealql_parses.rs',
                 '/tests/generated/',
                 # Deliberately malformed SurrealQL — that IS the assertion in these.
                 '/tests/absent_table_reads_empty.rs',
                 '/tests/order_by_idiom_probe.rs',
                 '/tests/group_collect_probe.rs',
                 # Federation speaks to Postgres/SQLite; its fixtures are foreign SQL by design.
                 '/tests/federation_test.rs',
                 '/tests/federation_sqlite_test.rs',
                 # `frame` speaks DataFusion SQL, not SurrealQL.
                 '/crates/frame/')
# Belt and braces for text that is NOT SurrealQL but survives the head/clause shape test. Each
# alternative below was a real false alarm before it was added — the corpus is worth nothing if it
# cries wolf, because the next person simply stops reading it.
FOREIGN_SQL = re.compile(
    # Postgres/SQLite the federation and pack exporters emit.
    r'\bCREATE\s+TABLE\b|\bINSERT\s+INTO\b|\bDOUBLE\s+PRECISION\b|\bPRIMARY\s+KEY\b|\?'
    # Grafana macro templates: `$__timeFilter(t)`, `$__timeGroup(...)`, `$__interval_ms`. These are
    # placeholders SUBSTITUTED before execution, so they are not meant to parse as written.
    r'|\$__'
    # `count(*)` / `SUM(...)`: SurrealQL spells these `count()` and `math::sum()`, so a `(*)` or a
    # bare `SUM(` is SQL for another engine.
    r'|(?i:\bcount\s*\(\s*\*\s*\))|(?i:\bsum\s*\()'
    # A table ALIAS (`FROM point_reading pr`) — SurrealQL has no such syntax.
    r'|(?i:\bfrom\s+[a-z_][a-z0-9_]*\s+[a-z][a-z0-9_]{0,3}\s+(?:group|where|order|limit)\b)'
    # `SELECT value FROM t`: in SurrealQL `VALUE` is a KEYWORD introducing a single-expression
    # projection (`SELECT VALUE name FROM t`), so a bare `value` column right before `FROM` is
    # another engine's SQL. A real `SELECT VALUE <expr> FROM` still goes through.
    r'|(?i:\bselect\s+value\s+from\b)'
)

# A FRAGMENT, not a statement: the caller concatenates a table name onto it at run time. Parsing it
# alone is meaningless, and it is the caller's composed string that matters.
FRAGMENT = re.compile(r'(?i)\b(?:from|where|set|by)\s*$')

# `LIVE` opens a SurrealQL statement, so a log line that happens to start with the word matches the
# head test. A real one is always `LIVE SELECT`.
NOT_A_STATEMENT = re.compile(r'(?i)^\s*live\s+(?!select\b)')

CONSTS = collect_consts(root)
print(f'resolved {len(CONSTS)} string constants')

found = []
for dp, dn, fns in os.walk(root):
    dn[:] = [d for d in dn if d != 'target']
    for f in fns:
        if not f.endswith('.rs'): continue
        p = os.path.join(dp, f)
        if any(k in p for k in FOREIGN_PATHS):
            continue
        src = strip_comments(open(p, encoding='utf-8', errors='replace').read())
        for m in STR.finditer(src):
            raw = unescape(m.group(1))
            if raw and (
                FOREIGN_SQL.search(raw)
                or FRAGMENT.search(raw.strip())
                or NOT_A_STATEMENT.match(raw)
            ):
                continue
            if raw is None or not HEAD.match(raw) or not CLAUSE.search(raw): continue
            line = src.count('\n', 0, m.start()) + 1
            where = f"{os.path.relpath(p, root)}:{line}"
            is_fmt = '{{' in m.group(1) or '}}' in m.group(1) or bool(PLACE.search(raw))
            if is_fmt:
                body = raw.replace('{{', '\0L\0').replace('}}', '\0R\0')

                def resolve(m, _c=CONSTS):
                    name = m.group(0)[1:-1].split(':')[0].strip()
                    return _c.get(name, '\0P\0')

                # Pass 1: swap in every constant we resolved. Pass 2: guess for whatever is left.
                resolved = PLACE.sub(resolve, body)
                variants = [resolved.replace('\0P\0', sub).replace('\0L\0','{').replace('\0R\0','}')
                            # `ASC` covers a placeholder standing in for an ORDER BY direction,
                            # which is a keyword and parses as neither an identifier nor a value.
                            for sub in ('ph', '1', "'ph'", 'ASC')]
            else:
                variants = [raw]
            # `\0P\0` marks a placeholder no constant resolved. The guesses below stand in for it,
            # but a `{}` in a structural position (a SET clause, a JOIN) cannot be guessed at all —
            # so if one survived, a parse failure is the GENERATOR's limit, not the query's, and
            # claiming otherwise is how a corpus loses its credibility.
            unresolved = is_fmt and '\0P\0' in resolved
            found.append({'where': where, 'variants': variants, 'unverifiable': unresolved})

json.dump(found, open(sys.argv[2], 'w'), indent=0)
print(f"{len(found)} SurrealQL literals extracted")
