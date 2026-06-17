//! Repo wide structural duplicate candidate clustering.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::manifest::{Manifest, is_test_export};
use crate::similarity::{
    Candidate, DEFAULT_THRESHOLD, candidate_shape_key, collect_candidates, score_candidates,
    tokenize_name,
};

pub const DEFAULT_DUPES_LIMIT: usize = 10;

const MAX_BLOCK_CANDIDATES: usize = 64;

#[derive(Debug, Clone)]
pub struct DupeOptions {
    pub directory: Option<String>,
    pub kinds: Vec<String>,
    pub min_score: f64,
    pub limit: usize,
    pub include_tests: bool,
}

impl Default for DupeOptions {
    fn default() -> Self {
        Self {
            directory: None,
            kinds: Vec::new(),
            min_score: DEFAULT_THRESHOLD,
            limit: DEFAULT_DUPES_LIMIT,
            include_tests: false,
        }
    }
}

impl DupeOptions {
    pub fn from_args(
        directory: Option<String>,
        kinds: Vec<String>,
        min_score: Option<f64>,
        limit: Option<usize>,
        include_tests: bool,
    ) -> Self {
        Self {
            directory,
            kinds,
            min_score: min_score.unwrap_or(DEFAULT_THRESHOLD),
            limit: limit.unwrap_or(DEFAULT_DUPES_LIMIT),
            include_tests,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DupeClustersResult {
    pub clusters: Vec<DupeCluster>,
    pub stats: DupeStats,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<DupeDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DupeCluster {
    pub score: f64,
    pub members: Vec<DupeMember>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DupeMember {
    pub name: String,
    pub file: String,
    pub lines: [usize; 2],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DupeStats {
    pub candidates: usize,
    pub blocks: usize,
    pub comparisons: usize,
    pub clusters: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DupeDiagnostic {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone)]
struct IndexedCandidate {
    candidate: Candidate,
    tokens: BTreeSet<String>,
    shape_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct MemberKey {
    file: String,
    name: String,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct BlockKey {
    kind: String,
    key_kind: &'static str,
    key_value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct PairKey {
    left: usize,
    right: usize,
}

pub fn find_dupe_clusters(manifest: &Manifest, opts: &DupeOptions) -> DupeClustersResult {
    let candidates = indexed_candidates(filtered_candidates(manifest, opts));
    let blocks = build_blocks(&candidates);
    let (pairs, block_count, mut diagnostics) = candidate_pairs(&blocks, &candidates);

    let mut edges = Vec::new();
    for pair in &pairs {
        let left = &candidates[pair.left].candidate;
        let right = &candidates[pair.right].candidate;
        let (score, _) = score_candidates(manifest, left, right);
        if score >= opts.min_score {
            edges.push((pair.left, pair.right, score));
        }
    }

    let mut clusters = clusters_from_edges(&candidates, &edges);
    clusters.sort_by(compare_clusters);
    clusters.truncate(opts.limit);

    diagnostics.sort_by(|a, b| a.message.cmp(&b.message));

    DupeClustersResult {
        stats: DupeStats {
            candidates: candidates.len(),
            blocks: block_count,
            comparisons: pairs.len(),
            clusters: clusters.len(),
        },
        clusters,
        diagnostics,
    }
}

fn filtered_candidates(manifest: &Manifest, opts: &DupeOptions) -> Vec<Candidate> {
    collect_candidates(manifest)
        .into_iter()
        .filter(|candidate| {
            opts.directory
                .as_deref()
                .map(|dir| candidate.file.starts_with(dir))
                .unwrap_or(true)
        })
        .filter(|candidate| {
            opts.kinds.is_empty()
                || candidate
                    .kind
                    .as_deref()
                    .is_some_and(|kind| opts.kinds.iter().any(|wanted| wanted == kind))
        })
        .filter(|candidate| {
            opts.include_tests
                || !is_test_export(&candidate.name, &candidate.file, candidate.kind.as_deref())
        })
        .collect()
}

fn indexed_candidates(mut candidates: Vec<Candidate>) -> Vec<IndexedCandidate> {
    candidates.sort_by_key(member_key);
    candidates
        .into_iter()
        .map(|candidate| {
            let tokens = tokenize_name(&candidate.name)
                .into_iter()
                .filter(|token| token.len() > 1)
                .collect();
            let shape_key = candidate_shape_key(&candidate);
            IndexedCandidate {
                candidate,
                tokens,
                shape_key,
            }
        })
        .collect()
}

fn build_blocks(candidates: &[IndexedCandidate]) -> BTreeMap<BlockKey, BTreeSet<usize>> {
    let token_frequency = token_frequency(candidates);
    let mut blocks: BTreeMap<BlockKey, BTreeSet<usize>> = BTreeMap::new();

    for (index, candidate) in candidates.iter().enumerate() {
        for key in block_keys(candidate, &token_frequency) {
            blocks.entry(key).or_default().insert(index);
        }
    }

    blocks
}

fn token_frequency(candidates: &[IndexedCandidate]) -> BTreeMap<(String, String), usize> {
    let mut frequency = BTreeMap::new();
    for candidate in candidates {
        let kind = kind_key(&candidate.candidate);
        for token in &candidate.tokens {
            *frequency.entry((kind.clone(), token.clone())).or_insert(0) += 1;
        }
    }
    frequency
}

fn block_keys(
    candidate: &IndexedCandidate,
    token_frequency: &BTreeMap<(String, String), usize>,
) -> BTreeSet<BlockKey> {
    let kind = kind_key(&candidate.candidate);
    let mut keys = BTreeSet::new();

    let mut tokens: Vec<_> = candidate.tokens.iter().collect();
    tokens.sort_by(|a, b| {
        token_frequency
            .get(&(kind.clone(), (*a).clone()))
            .cmp(&token_frequency.get(&(kind.clone(), (*b).clone())))
            .then_with(|| a.cmp(b))
    });

    for token in tokens.into_iter().take(2) {
        keys.insert(BlockKey {
            kind: kind.clone(),
            key_kind: "name-token",
            key_value: token.clone(),
        });
    }

    if candidate.candidate.signature.is_some() {
        keys.insert(BlockKey {
            kind: kind.clone(),
            key_kind: "shape",
            key_value: candidate.shape_key.clone(),
        });
    }

    if keys.is_empty() {
        keys.insert(BlockKey {
            kind,
            key_kind: "fallback-shape",
            key_value: candidate.shape_key.clone(),
        });
    }

    keys
}

fn candidate_pairs(
    blocks: &BTreeMap<BlockKey, BTreeSet<usize>>,
    candidates: &[IndexedCandidate],
) -> (BTreeSet<PairKey>, usize, Vec<DupeDiagnostic>) {
    let mut pairs = BTreeSet::new();
    let mut diagnostics = Vec::new();
    let mut block_count = 0;

    for (key, indexes) in blocks {
        if indexes.len() < 2 {
            continue;
        }
        if indexes.len() <= MAX_BLOCK_CANDIDATES {
            block_count += 1;
            add_pairs(indexes.iter().copied(), &mut pairs);
            continue;
        }

        let mut by_shape: BTreeMap<&str, BTreeSet<usize>> = BTreeMap::new();
        for index in indexes {
            by_shape
                .entry(candidates[*index].shape_key.as_str())
                .or_default()
                .insert(*index);
        }

        for (shape, shape_indexes) in by_shape {
            if shape_indexes.len() < 2 {
                continue;
            }
            block_count += 1;
            add_capped_pairs(key, shape, &shape_indexes, &mut pairs, &mut diagnostics);
        }
    }

    (pairs, block_count, diagnostics)
}

fn add_capped_pairs(
    key: &BlockKey,
    shape: &str,
    indexes: &BTreeSet<usize>,
    pairs: &mut BTreeSet<PairKey>,
    diagnostics: &mut Vec<DupeDiagnostic>,
) {
    if indexes.len() <= MAX_BLOCK_CANDIDATES {
        add_pairs(indexes.iter().copied(), pairs);
        return;
    }

    let skipped = pair_total(indexes.len()) - pair_total(MAX_BLOCK_CANDIDATES);
    diagnostics.push(DupeDiagnostic {
        code: "block_overflow".to_string(),
        message: format!(
            "Skipped {skipped} overflow comparisons in {}/{}/{} shape {}",
            key.kind, key.key_kind, key.key_value, shape
        ),
    });
    add_pairs(indexes.iter().copied().take(MAX_BLOCK_CANDIDATES), pairs);
}

fn add_pairs(indexes: impl IntoIterator<Item = usize>, pairs: &mut BTreeSet<PairKey>) {
    let indexes: Vec<_> = indexes.into_iter().collect();
    for (offset, left) in indexes.iter().enumerate() {
        for right in indexes.iter().skip(offset + 1) {
            pairs.insert(PairKey {
                left: *left,
                right: *right,
            });
        }
    }
}

fn clusters_from_edges(
    candidates: &[IndexedCandidate],
    edges: &[(usize, usize, f64)],
) -> Vec<DupeCluster> {
    let mut union_find = UnionFind::new(candidates.len());
    for (left, right, _) in edges {
        union_find.union(*left, *right);
    }

    let mut members_by_root: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    let mut score_by_root: BTreeMap<usize, f64> = BTreeMap::new();
    for (left, right, score) in edges {
        let root = union_find.find(*left);
        members_by_root.entry(root).or_default().insert(*left);
        members_by_root.entry(root).or_default().insert(*right);
        score_by_root
            .entry(root)
            .and_modify(|existing| *existing = existing.max(*score))
            .or_insert(*score);
    }

    members_by_root
        .into_iter()
        .filter_map(|(root, indexes)| {
            if indexes.len() < 2 {
                return None;
            }
            let mut members: Vec<_> = indexes
                .into_iter()
                .map(|index| dupe_member(&candidates[index].candidate))
                .collect();
            members.sort_by(|a, b| {
                a.file
                    .cmp(&b.file)
                    .then_with(|| a.name.cmp(&b.name))
                    .then_with(|| a.lines.cmp(&b.lines))
            });
            Some(DupeCluster {
                score: *score_by_root.get(&root).unwrap_or(&0.0),
                members,
            })
        })
        .collect()
}

fn compare_clusters(left: &DupeCluster, right: &DupeCluster) -> Ordering {
    right
        .score
        .partial_cmp(&left.score)
        .unwrap_or(Ordering::Equal)
        .then_with(|| right.members.len().cmp(&left.members.len()))
        .then_with(|| {
            cluster_first_key(left)
                .unwrap_or_default()
                .cmp(&cluster_first_key(right).unwrap_or_default())
        })
}

fn cluster_first_key(cluster: &DupeCluster) -> Option<(String, String, [usize; 2])> {
    cluster
        .members
        .first()
        .map(|member| (member.file.clone(), member.name.clone(), member.lines))
}

fn dupe_member(candidate: &Candidate) -> DupeMember {
    DupeMember {
        name: candidate.name.clone(),
        file: candidate.file.clone(),
        lines: [candidate.start, candidate.end],
        signature: candidate.signature.clone(),
        kind: candidate.kind.clone(),
    }
}

fn member_key(candidate: &Candidate) -> MemberKey {
    MemberKey {
        file: candidate.file.clone(),
        name: candidate.name.clone(),
        start: candidate.start,
        end: candidate.end,
    }
}

fn kind_key(candidate: &Candidate) -> String {
    candidate
        .kind
        .clone()
        .unwrap_or_else(|| "unknown".to_string())
}

fn pair_total(size: usize) -> usize {
    size.saturating_mul(size.saturating_sub(1)) / 2
}

struct UnionFind {
    parent: Vec<usize>,
}

impl UnionFind {
    fn new(size: usize) -> Self {
        Self {
            parent: (0..size).collect(),
        }
    }

    fn find(&self, mut index: usize) -> usize {
        while self.parent[index] != index {
            index = self.parent[index];
        }
        index
    }

    fn union(&mut self, left: usize, right: usize) {
        let left_root = self.find(left);
        let right_root = self.find(right);
        if left_root == right_root {
            return;
        }
        let root = left_root.min(right_root);
        self.parent[left_root] = root;
        self.parent[right_root] = root;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{DeclarationKind, ExportEntry, Metadata};

    fn manifest_with_exports(exports: Vec<(&str, &str, &str)>) -> Manifest {
        let mut manifest = Manifest::new();
        for (file, name, signature) in exports {
            let mut entry = ExportEntry::new(name.to_string(), 1, 3);
            entry.signature = Some(signature.to_string());
            entry.declaration_kind = Some(DeclarationKind::Fn);
            manifest.add_file(
                file,
                Metadata {
                    exports: vec![entry],
                    ..Default::default()
                },
            );
        }
        manifest
    }

    #[test]
    fn blocking_deduplicates_pairs_and_caps_oversized_blocks() {
        let exports: Vec<_> = (0..240)
            .map(|index| {
                (
                    format!("src/{index}.rs"),
                    format!("load_item_{index}"),
                    "() -> String".to_string(),
                )
            })
            .collect();
        let exports: Vec<_> = exports
            .iter()
            .map(|(file, name, signature)| (file.as_str(), name.as_str(), signature.as_str()))
            .collect();
        let manifest = manifest_with_exports(exports);
        let candidates =
            indexed_candidates(filtered_candidates(&manifest, &DupeOptions::default()));
        let blocks = build_blocks(&candidates);
        let (pairs, _, diagnostics) = candidate_pairs(&blocks, &candidates);

        assert!(
            pairs.len() < pair_total(240),
            "blocking must avoid all pairs"
        );
        assert!(
            pairs.len() <= pair_total(MAX_BLOCK_CANDIDATES) * 3,
            "overflow cap must bound comparisons"
        );
        assert!(
            diagnostics.iter().any(|diag| diag.code == "block_overflow"),
            "overflow must be visible"
        );
    }

    #[test]
    fn union_find_clustering_is_order_independent() {
        let manifest = manifest_with_exports(vec![
            ("src/a.rs", "load_user", "() -> User"),
            ("src/b.rs", "fetch_user", "() -> User"),
            ("src/c.rs", "read_user", "() -> User"),
        ]);
        let candidates =
            indexed_candidates(filtered_candidates(&manifest, &DupeOptions::default()));
        let forward = clusters_from_edges(&candidates, &[(0, 1, 0.7), (1, 2, 0.5)]);
        let reverse = clusters_from_edges(&candidates, &[(1, 2, 0.5), (0, 1, 0.7)]);

        assert_eq!(forward, reverse);
        assert_eq!(forward[0].score, 0.7);
    }

    #[test]
    fn near_duplicate_clusters_but_same_shape_coincidence_does_not() {
        let manifest = manifest_with_exports(vec![
            ("src/extract.rs", "extract_imports", "(&str) -> Vec<String>"),
            ("src/collect.rs", "collect_imports", "(&str) -> Vec<String>"),
            ("src/ready.rs", "is_ready", "(&str) -> bool"),
        ]);

        let result = find_dupe_clusters(&manifest, &DupeOptions::default());
        let names: BTreeSet<_> = result.clusters[0]
            .members
            .iter()
            .map(|member| member.name.as_str())
            .collect();

        assert!(names.contains("extract_imports"));
        assert!(names.contains("collect_imports"));
        assert!(!names.contains("is_ready"));
    }

    #[test]
    fn json_output_is_byte_stable() {
        let manifest = manifest_with_exports(vec![
            ("src/extract.rs", "extract_imports", "(&str) -> Vec<String>"),
            ("src/collect.rs", "collect_imports", "(&str) -> Vec<String>"),
        ]);
        let first = find_dupe_clusters(&manifest, &DupeOptions::default());
        let second = find_dupe_clusters(&manifest, &DupeOptions::default());

        assert_eq!(
            serde_json::to_string_pretty(&first).unwrap(),
            serde_json::to_string_pretty(&second).unwrap()
        );
    }
}
