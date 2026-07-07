// The shared SQL tokenizer for the SQL→model parsers (`fromStandardSql.ts` / `fromSurrealQL.ts`) —
// the query-builder slice-1 follow-up ("Code→Builder sync"). One responsibility (FILE-LAYOUT): turn a
// SQL string into a flat token stream both recursive-descent parsers consume. The grammar is exactly
// the subset the emitters (`toStandardSql.ts` / `toSurrealQL.ts`) produce, plus the hand-written
// variants of it (bare identifiers, `<>`, stray whitespace) — NOT general SQL. Anything the lexer
// cannot classify becomes a thrown `SqlParseError`, which the parsers catch and turn into `null`
// ("not expressible in the builder") — malformed/injection-shaped input must never panic.

/** A lexed token. `upper` is the uppercased text for keyword matching on `word` tokens; `qident` is
 *  a double-quoted identifier (standard dialect) with the `""` escape already unfolded; `string` is a
 *  single-quoted literal with `''` unfolded; `number` carries the parsed numeric `value`. */
export interface SqlToken {
  kind: "word" | "qident" | "string" | "number" | "punct" | "eof";
  /** The token text — for `qident`/`string` the UNQUOTED, unescaped content. */
  text: string;
  /** Uppercased `text` (keyword matching) — only meaningful for `word`. */
  upper: string;
  /** The numeric value — only for `number`. */
  value?: number;
}

/** The parse-failure signal both parsers throw internally and catch at their boundary. */
export class SqlParseError extends Error {}

/** The multi-char / single-char punctuation the emitter subset uses. Order matters: longest first. */
const PUNCT = ["::", "!=", ">=", "<=", "<>", "(", ")", ",", ".", ";", "*", "=", ">", "<"];

const WORD_RE = /^[A-Za-z_][A-Za-z0-9_]*/;

/** Tokenize `sql` into a flat stream ending with one `eof` token. Throws `SqlParseError` on any
 *  character the emitter subset never produces (backticks, `--` comments, `$` params, …). */
export function tokenize(sql: string): SqlToken[] {
  const out: SqlToken[] = [];
  let i = 0;
  const n = sql.length;
  while (i < n) {
    const ch = sql[i];
    if (/\s/.test(ch)) {
      i++;
      continue;
    }
    if (ch === '"') {
      const { content, next } = readQuoted(sql, i, '"');
      out.push({ kind: "qident", text: content, upper: content.toUpperCase() });
      i = next;
      continue;
    }
    if (ch === "'") {
      const { content, next } = readQuoted(sql, i, "'");
      out.push({ kind: "string", text: content, upper: "" });
      i = next;
      continue;
    }
    if (/[0-9]/.test(ch) || (ch === "-" && /[0-9]/.test(sql[i + 1] ?? ""))) {
      const m = /^-?[0-9]+(\.[0-9]+)?/.exec(sql.slice(i));
      if (!m) throw new SqlParseError(`bad number at ${i}`);
      out.push({ kind: "number", text: m[0], upper: "", value: Number(m[0]) });
      i += m[0].length;
      continue;
    }
    const word = WORD_RE.exec(sql.slice(i));
    if (word) {
      out.push({ kind: "word", text: word[0], upper: word[0].toUpperCase() });
      i += word[0].length;
      continue;
    }
    const punct = PUNCT.find((p) => sql.startsWith(p, i));
    if (punct) {
      out.push({ kind: "punct", text: punct, upper: punct });
      i += punct.length;
      continue;
    }
    throw new SqlParseError(`unexpected character ${JSON.stringify(ch)} at ${i}`);
  }
  out.push({ kind: "eof", text: "", upper: "" });
  return out;
}

/** Read a quoted region starting at `start` (which holds the quote char), unfolding the doubled-quote
 *  escape. Returns the content and the index just past the closing quote. Unterminated ⇒ throw. */
function readQuoted(sql: string, start: number, quote: string): { content: string; next: number } {
  let content = "";
  let i = start + 1;
  while (i < sql.length) {
    if (sql[i] === quote) {
      if (sql[i + 1] === quote) {
        content += quote;
        i += 2;
        continue;
      }
      return { content, next: i + 1 };
    }
    content += sql[i];
    i++;
  }
  throw new SqlParseError(`unterminated ${quote}…${quote} starting at ${start}`);
}

/** A tiny cursor over the token stream — the shared walk state for both recursive-descent parsers. */
export class TokenCursor {
  private pos = 0;
  constructor(private readonly tokens: SqlToken[]) {}

  peek(ahead = 0): SqlToken {
    return this.tokens[Math.min(this.pos + ahead, this.tokens.length - 1)];
  }

  next(): SqlToken {
    const t = this.peek();
    if (t.kind !== "eof") this.pos++;
    return t;
  }

  /** Consume the next token iff it is the keyword `word` (case-insensitive). */
  eatWord(word: string): boolean {
    const t = this.peek();
    if (t.kind === "word" && t.upper === word) {
      this.pos++;
      return true;
    }
    return false;
  }

  /** Consume the next token iff it is the punctuation `p`. */
  eatPunct(p: string): boolean {
    const t = this.peek();
    if (t.kind === "punct" && t.text === p) {
      this.pos++;
      return true;
    }
    return false;
  }

  /** Require the keyword `word` next, else throw. */
  expectWord(word: string): void {
    if (!this.eatWord(word)) throw new SqlParseError(`expected ${word}, got ${this.peek().text || "end"}`);
  }

  /** Require the punctuation `p` next, else throw. */
  expectPunct(p: string): void {
    if (!this.eatPunct(p)) throw new SqlParseError(`expected ${p}, got ${this.peek().text || "end"}`);
  }

  /** True when only an optional trailing `;` remains. Consumes it. */
  atEnd(): boolean {
    this.eatPunct(";");
    return this.peek().kind === "eof";
  }
}
