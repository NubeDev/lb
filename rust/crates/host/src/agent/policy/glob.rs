//! A tiny `*`-wildcard matcher for the policy's tool-name glob (agent-run scope Part 2).
//!
//! Why not a glob crate: the scope is deliberately small — one wildcard character — and the
//! workspace has no `glob`/`globset` dependency for this. Pulling a crate in for a single `*` is
//! more surface than the feature warrants (FILE-LAYOUT: one responsibility, no gratuitous deps). So
//! this is a minimal star-glob: `*` matches any run of characters (including empty); every other
//! character matches literally. No `?`, no character classes, no escaping — if a real caller ever
//! needs them, that is the moment to add a glob crate, not before.
//!
//! The algorithm is the classic linear-time two-pointer star match (no backtracking blowup): it
//! handles multiple `*` (e.g. `*.echo`, `shell.*`, `a*b*c`) in O(n) over the pattern + name.

/// Whether `name` matches `pattern`, where `*` in the pattern is the only wildcard (matches any run
/// of characters, including empty). All other characters match literally.
pub fn matches(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let n: Vec<char> = name.chars().collect();
    // Two-pointer scan with a remembered star position so a `*` can absorb more characters on a later
    // mismatch — linear, no recursion (the standard wildcard-match without backtracking explosion).
    let (mut pi, mut ni) = (0usize, 0usize);
    let (mut star, mut matchpos) = (None::<usize>, 0usize);
    while ni < n.len() {
        if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            matchpos = ni;
            pi += 1;
        } else if pi < p.len() && p[pi] == n[ni] {
            pi += 1;
            ni += 1;
        } else if let Some(s) = star {
            // Backtrack to just after the last `*`, letting it absorb one more character.
            pi = s + 1;
            matchpos += 1;
            ni = matchpos;
        } else {
            return false;
        }
    }
    // Trailing `*`s in the pattern still match the now-empty remainder of the name.
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::matches;

    #[test]
    fn literal_match() {
        assert!(matches("hello.echo", "hello.echo"));
        assert!(!matches("hello.echo", "hello.echox"));
        assert!(!matches("hello.echo", "hello.ech"));
    }

    #[test]
    fn star_is_any_run_including_empty() {
        assert!(matches("*", ""));
        assert!(matches("*", "anything"));
        assert!(matches("shell.*", "shell.run"));
        assert!(matches("shell.*", "shell.")); // trailing star matches empty
        assert!(!matches("shell.*", "shellx.run"));
    }

    #[test]
    fn prefix_and_infix_stars() {
        assert!(matches("*.echo", "hello.echo"));
        assert!(matches("a*b*c", "axxbyyc"));
        assert!(matches("a*c", "ac"));
        assert!(!matches("a*c", "ab"));
    }
}
