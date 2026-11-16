//! A small glob matcher for profile paths.
//!
//! Patterns and paths are both normalised text (lowercase, `/`-separated). A segment
//! pattern supports `*` (any run within one segment) and `?` (one character); a segment
//! that is exactly `**` matches zero or more whole segments.

/// Whether `path` matches `pattern`, segment by segment.
pub(crate) fn matches(pattern: &str, path: &str) -> bool {
    let pat: Vec<&str> = pattern.split('/').collect();
    let segs: Vec<&str> = path.split('/').collect();
    match_segments(&pat, &segs)
}

/// A pattern with no `/` matches the last segment of the path, at any depth. A leading `/`
/// anchors the rest of the pattern to the source root, so `/*` names only root-level files:
/// `*` never crosses a `/`, and a bare `*` would match every file.
pub(crate) fn matches_basename_or_path(pattern: &str, path: &str) -> bool {
    if let Some(anchored) = pattern.strip_prefix('/') {
        matches(anchored, path)
    } else if pattern.contains('/') {
        matches(pattern, path)
    } else {
        path.rsplit('/').next().is_some_and(|name| segment(pattern, name))
    }
}

/// How specific a pattern is: the length of its literal prefix, the text before the first
/// wildcard (doc 10 § Overlapping globs). Longer prefixes win; ties fall to declaration order.
pub(crate) fn specificity(pattern: &str) -> usize {
    pattern.find(['*', '?']).unwrap_or(pattern.len())
}

fn match_segments(pat: &[&str], segs: &[&str]) -> bool {
    match pat.split_first() {
        None => segs.is_empty(),
        Some((&"**", rest)) => (0..=segs.len()).any(|skip| match_segments(rest, &segs[skip..])),
        Some((p, rest)) => {
            segs.split_first().is_some_and(|(s, tail)| segment(p, s) && match_segments(rest, tail))
        }
    }
}

fn segment(pattern: &str, text: &str) -> bool {
    let (p, t): (Vec<char>, Vec<char>) = (pattern.chars().collect(), text.chars().collect());
    let (mut pi, mut ti) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some((pi, ti));
            pi += 1;
        } else if let Some((sp, st)) = star {
            pi = sp + 1;
            ti = st + 1;
            star = Some((sp, st + 1));
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|c| *c == '*')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn double_star_spans_zero_or_more_segments() {
        assert!(matches("common/defines/**/*.txt", "common/defines/a.txt"));
        assert!(matches("common/defines/**/*.txt", "common/defines/x/y/a.txt"));
        assert!(!matches("common/defines/**/*.txt", "common/defines/a.yml"));
        assert!(!matches("common/defines/**/*.txt", "common/traits/a.txt"));
        assert!(matches("gfx/**", "gfx/a/b.dds"));
    }

    #[test]
    fn single_star_stays_inside_a_segment() {
        assert!(matches("common/governments/*.txt", "common/governments/a.txt"));
        assert!(!matches("common/governments/*.txt", "common/governments/civics/a.txt"));
    }

    #[test]
    fn bare_patterns_match_the_basename_at_any_depth() {
        assert!(matches_basename_or_path("readme*", "deep/dir/readme.txt"));
        assert!(matches_basename_or_path("*.md", "notes.md"));
        assert!(matches_basename_or_path(".git/**", ".git/config"));
        assert!(!matches_basename_or_path("thumbnail.png", "gfx/thumbnail2.png"));
    }

    #[test]
    fn a_leading_slash_anchors_a_pattern_to_the_source_root() {
        assert!(matches_basename_or_path("/*", "bigpicture.jpg"));
        assert!(!matches_basename_or_path("/*", "gfx/a.dds"));
        assert!(matches_basename_or_path("/*.jpg", "a.jpg"));
        assert!(!matches_basename_or_path("/*.jpg", "gfx/a.jpg"));
        assert!(matches_basename_or_path("*.jpg", "gfx/a.jpg"), "bare patterns still float");
    }

    #[test]
    fn longer_literals_are_more_specific() {
        assert!(
            specificity("localisation/replace/**/*.yml") > specificity("localisation/**/*.yml")
        );
        assert!(
            specificity("common/traits/**/*.txt") > specificity("common/**/long_literal.txt"),
            "the literal prefix decides, not the count of literal characters"
        );
    }
}
