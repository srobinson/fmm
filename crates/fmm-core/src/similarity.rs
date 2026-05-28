//! Deterministic structural similarity between symbols.
//!
//! Given a probe (an existing symbol, or a signature you are about to write),
//! rank existing symbols by how structurally similar they are so callers reuse
//! instead of duplicating. No embeddings: the score blends name-token overlap,
//! signature shape, declaration kind, and shared file-level dependency
//! neighborhood — all from data already in the [`Manifest`].
//!
//! Results are threshold-gated, never padded to the cap: a probe with one real
//! match returns one row.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use crate::manifest::Manifest;

/// Signal weights. When the neighborhood signal is unavailable (pre-write probe
/// with no originating file) the remaining three are renormalized to sum to 1.
const W_NAME: f64 = 0.50;
const W_SHAPE: f64 = 0.25;
const W_KIND: f64 = 0.10;
const W_NEIGHBORHOOD: f64 = 0.15;

/// What we search for.
#[derive(Debug, Clone)]
pub struct SymbolProbe {
    pub name: String,
    pub signature: Option<String>,
    pub kind: Option<String>,
    /// `Some` in by-symbol mode (enables the neighborhood signal); `None`
    /// pre-write.
    pub file: Option<String>,
}

/// Query tuning. `limit` starts at 10 and is reduced as we calibrate; the real
/// precision lever is `threshold`.
#[derive(Debug, Clone)]
pub struct SimilarOptions {
    pub limit: usize,
    pub threshold: f64,
    pub directory: Option<String>,
    pub include_tests: bool,
}

impl Default for SimilarOptions {
    fn default() -> Self {
        Self {
            limit: 10,
            threshold: 0.30,
            directory: None,
            include_tests: false,
        }
    }
}

/// Per-signal contributions, surfaced for transparency in the output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Signals {
    pub name: f64,
    pub shape: f64,
    pub kind: f64,
    pub neighborhood: f64,
}

/// A ranked similarity hit.
#[derive(Debug, Clone)]
pub struct SimilarMatch {
    pub name: String,
    pub file: String,
    pub start_line: usize,
    pub end_line: usize,
    pub signature: Option<String>,
    pub kind: Option<String>,
    pub score: f64,
    pub signals: Signals,
}

/// Rank existing symbols against `probe`. Drops the probe itself, test
/// boilerplate (unless `opts.include_tests`), out-of-directory candidates, and
/// sub-threshold scores; sorts by score desc with deterministic tie-breaks;
/// truncates to `opts.limit`.
pub fn find_similar(
    manifest: &Manifest,
    probe: &SymbolProbe,
    opts: &SimilarOptions,
) -> Vec<SimilarMatch> {
    let probe_tokens = tokenize_name(&probe.name);
    let probe_shape = probe
        .signature
        .as_deref()
        .map(signature_shape)
        .unwrap_or_default();

    let mut out: Vec<SimilarMatch> = Vec::new();
    for cand in collect_candidates(manifest) {
        if cand.name == probe.name && probe.file.as_deref() == Some(cand.file.as_str()) {
            continue;
        }
        if let Some(dir) = &opts.directory
            && !cand.file.starts_with(dir.as_str())
        {
            continue;
        }
        if !opts.include_tests && is_test_symbol(&cand) {
            continue;
        }

        let neighborhood = probe
            .file
            .as_deref()
            .map(|pf| neighborhood_score(manifest, pf, &cand.file));

        let (score, signals) = score_against(
            &probe_tokens,
            &probe_shape,
            probe.kind.as_deref(),
            &cand.name,
            cand.signature.as_deref(),
            cand.kind.as_deref(),
            neighborhood,
        );
        if score < opts.threshold {
            continue;
        }

        out.push(SimilarMatch {
            name: cand.name,
            file: cand.file,
            start_line: cand.start,
            end_line: cand.end,
            signature: cand.signature,
            kind: cand.kind,
            score,
            signals,
        });
    }

    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.file.cmp(&b.file))
    });
    out.truncate(opts.limit);
    out
}

/// Split a symbol name into lowercased tokens on `_ - . : / space` and
/// camelCase / acronym / letter-digit boundaries. `HTTPServer` → {http, server}.
pub fn tokenize_name(name: &str) -> BTreeSet<String> {
    let chars: Vec<char> = name.chars().collect();
    let mut tokens = BTreeSet::new();
    let mut current = String::new();

    for i in 0..chars.len() {
        let ch = chars[i];
        if matches!(ch, '_' | '-' | '.' | ':' | '/' | ' ') {
            push_token(&mut current, &mut tokens);
            continue;
        }
        if !current.is_empty() {
            let prev = chars[i - 1];
            // loadConfig -> load | config
            let lower_to_upper = (prev.is_lowercase() || prev.is_numeric()) && ch.is_uppercase();
            // HTTPServer -> http | server (acronym followed by a word)
            let acronym_end = prev.is_uppercase()
                && ch.is_uppercase()
                && chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            // parse2json -> parse | 2 | json
            let alpha_digit = prev.is_alphabetic() && ch.is_numeric();
            let digit_alpha = prev.is_numeric() && ch.is_alphabetic();
            if lower_to_upper || acronym_end || alpha_digit || digit_alpha {
                push_token(&mut current, &mut tokens);
            }
        }
        current.push(ch.to_ascii_lowercase());
    }
    push_token(&mut current, &mut tokens);
    tokens
}

fn push_token(current: &mut String, tokens: &mut BTreeSet<String>) {
    if !current.is_empty() {
        tokens.insert(std::mem::take(current));
    }
}

/// Shallow, language-aware-lite signature shape. Not a parser.
#[derive(Debug, Clone, Default)]
struct Shape {
    arity: Option<usize>,
    ret: Option<String>,
}

fn signature_shape(sig: &str) -> Shape {
    Shape {
        arity: extract_arity(sig),
        ret: extract_return(sig),
    }
}

/// Count top-level params inside the first balanced `(...)`. `()` → 0.
/// Returns `None` if no balanced parameter list is found.
fn extract_arity(sig: &str) -> Option<usize> {
    let open = sig.find('(')?;
    let mut depth = 0i32;
    let mut commas = 0usize;
    let mut saw_content = false;
    let mut closed = false;
    for ch in sig[open..].chars() {
        match ch {
            '(' | '[' | '<' | '{' => depth += 1,
            ')' | ']' | '>' | '}' => {
                depth -= 1;
                if depth == 0 {
                    closed = true;
                    break;
                }
            }
            ',' if depth == 1 => commas += 1,
            c if depth >= 1 && !c.is_whitespace() => saw_content = true,
            _ => {}
        }
    }
    if !closed {
        return None;
    }
    if saw_content {
        Some(commas + 1)
    } else {
        Some(0)
    }
}

/// Extract a normalized return-type token: after `->` (Rust/Python) or the
/// trailing `: T` after the parameter list (TypeScript).
fn extract_return(sig: &str) -> Option<String> {
    if let Some(idx) = sig.rfind("->") {
        let ret = sig[idx + 2..].trim().trim_end_matches('{').trim();
        if !ret.is_empty() {
            return Some(normalize_type(ret));
        }
    }
    if let Some(close) = sig.rfind(')') {
        let tail = &sig[close + 1..];
        if let Some(colon) = tail.find(':') {
            let ret = tail[colon + 1..].trim().trim_end_matches('{').trim();
            if !ret.is_empty() {
                return Some(normalize_type(ret));
            }
        }
    }
    None
}

fn normalize_type(t: &str) -> String {
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn shape_score(a: &Shape, b: &Shape) -> f64 {
    let mut score = 0.0;

    // Arity: exact match full credit, off-by-one half credit.
    if let (Some(x), Some(y)) = (a.arity, b.arity) {
        if x == y {
            score += 1.0;
        } else if x.abs_diff(y) == 1 {
            score += 0.5;
        }
    }

    // Return type: equal types or both unit count as a match.
    match (&a.ret, &b.ret) {
        (Some(x), Some(y)) if x == y => score += 1.0,
        (None, None) => score += 1.0,
        _ => {}
    }

    score / 2.0
}

fn jaccard(a: &BTreeSet<String>, b: &BTreeSet<String>) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let inter = a.intersection(b).count() as f64;
    let union = a.union(b).count() as f64;
    inter / union
}

fn neighborhood_score(manifest: &Manifest, probe_file: &str, cand_file: &str) -> f64 {
    if probe_file == cand_file {
        return 0.0;
    }
    let a = file_neighbors(manifest, probe_file);
    let b = file_neighbors(manifest, cand_file);
    jaccard(&a, &b)
}

fn file_neighbors(manifest: &Manifest, file: &str) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    if let Some(entry) = manifest.files.get(file) {
        set.extend(entry.imports.iter().cloned());
        set.extend(entry.dependencies.iter().cloned());
    }
    set
}

#[allow(clippy::too_many_arguments)]
fn score_against(
    probe_tokens: &BTreeSet<String>,
    probe_shape: &Shape,
    probe_kind: Option<&str>,
    cand_name: &str,
    cand_sig: Option<&str>,
    cand_kind: Option<&str>,
    neighborhood: Option<f64>,
) -> (f64, Signals) {
    let name = jaccard(probe_tokens, &tokenize_name(cand_name));
    let shape = match cand_sig {
        Some(s) => shape_score(probe_shape, &signature_shape(s)),
        None => 0.0,
    };
    let kind = match (probe_kind, cand_kind) {
        (Some(a), Some(b)) if a == b => 1.0,
        _ => 0.0,
    };
    let nbhd = neighborhood.unwrap_or(0.0);

    let signals = Signals {
        name,
        shape,
        kind,
        neighborhood: nbhd,
    };

    let score = if neighborhood.is_some() {
        W_NAME * name + W_SHAPE * shape + W_KIND * kind + W_NEIGHBORHOOD * nbhd
    } else {
        (W_NAME * name + W_SHAPE * shape + W_KIND * kind) / (W_NAME + W_SHAPE + W_KIND)
    };

    (score, signals)
}

/// Flattened view of one indexed symbol.
struct Candidate {
    name: String,
    file: String,
    start: usize,
    end: usize,
    signature: Option<String>,
    kind: Option<String>,
}

fn collect_candidates(manifest: &Manifest) -> Vec<Candidate> {
    let mut out = Vec::new();
    for (file, entry) in &manifest.files {
        for (i, name) in entry.exports.iter().enumerate() {
            let lines = entry.export_lines.as_ref().and_then(|v| v.get(i));
            let meta = entry.export_metadata.get(name);
            out.push(Candidate {
                name: name.clone(),
                file: file.clone(),
                start: lines.map(|l| l.start).unwrap_or(0),
                end: lines.map(|l| l.end).unwrap_or(0),
                signature: meta.and_then(|m| m.signature.clone()),
                kind: meta.and_then(|m| m.declaration_kind.clone()),
            });
        }
        if let Some(methods) = &entry.methods {
            for (dotted, lines) in methods {
                let meta = entry.method_metadata.get(dotted);
                out.push(Candidate {
                    name: dotted.clone(),
                    file: file.clone(),
                    start: lines.start,
                    end: lines.end,
                    signature: meta.and_then(|m| m.signature.clone()),
                    kind: meta.and_then(|m| m.declaration_kind.clone()),
                });
            }
        }
    }
    out
}

fn is_test_symbol(cand: &Candidate) -> bool {
    matches!(cand.kind.as_deref(), Some("test"))
        || cand.name == "tests"
        || cand.name.starts_with("test_")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set<const N: usize>(items: [&str; N]) -> BTreeSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn tokenize_splits_snake_camel_and_dots() {
        assert_eq!(tokenize_name("load_config"), set(["load", "config"]));
        assert_eq!(tokenize_name("loadConfig"), set(["load", "config"]));
        assert_eq!(
            tokenize_name("ClassName.method"),
            set(["class", "name", "method"])
        );
        assert_eq!(tokenize_name("HTTPServer"), set(["http", "server"]));
    }

    /// Score a probe symbol against a candidate, neighborhood unavailable
    /// (pre-write path). Exercises the real public scoring blend.
    fn score_pair(probe: (&str, &str, &str), cand: (&str, &str, &str)) -> f64 {
        let probe_tokens = tokenize_name(probe.0);
        let probe_shape = signature_shape(probe.1);
        let (score, _) = score_against(
            &probe_tokens,
            &probe_shape,
            Some(probe.2),
            cand.0,
            Some(cand.1),
            Some(cand.2),
            None,
        );
        score
    }

    #[test]
    fn clone_outscores_coincidental_shape() {
        // Shared "imports" token + identical shape -> high.
        let clone = score_pair(
            ("extract_imports", "(&str) -> Vec<String>", "fn"),
            ("collect_imports", "(&str) -> Vec<String>", "fn"),
        );
        // No shared tokens, different return -> low.
        let coincidence = score_pair(
            ("extract_imports", "(&str) -> Vec<String>", "fn"),
            ("is_ready", "(&str) -> bool", "fn"),
        );

        assert!(
            clone > coincidence,
            "clone {clone} must beat coincidence {coincidence}"
        );
        assert!(clone >= 0.30, "clone {clone} must clear the default threshold");
        assert!(
            coincidence < 0.30,
            "coincidence {coincidence} must be gated out"
        );
    }

    #[test]
    fn arity_and_return_shape() {
        assert_eq!(extract_arity("(&str) -> bool"), Some(1));
        assert_eq!(extract_arity("() -> bool"), Some(0));
        assert_eq!(extract_arity("(a: A, b: B) -> C"), Some(2));
        // commas inside generics are not counted
        assert_eq!(extract_arity("(x: HashMap<K, V>) -> bool"), Some(1));
        assert_eq!(extract_return("fn f(x: A) -> Vec<String>"), Some("Vec<String>".to_string()));
        assert_eq!(
            extract_return("createSession(u: User): string"),
            Some("string".to_string())
        );
    }

    #[test]
    fn jaccard_basics() {
        assert_eq!(jaccard(&set(["a", "b"]), &set(["a", "b"])), 1.0);
        assert_eq!(jaccard(&set(["a", "b"]), &set([])), 0.0);
        assert!((jaccard(&set(["a", "b"]), &set(["b", "c"])) - 1.0 / 3.0).abs() < 1e-9);
    }
}
