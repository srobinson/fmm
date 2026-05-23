use super::ReadSymbolGuidance;
use fmm_core::manifest::private_members;
use fmm_core::manifest::{ExportLines, Manifest, SymbolMetadata};
use std::collections::{HashMap, HashSet};
use std::path::Path;

const DISPLAY_CAP: usize = 20;
const SUGGESTION_CAP: usize = 3;
const CROSS_TYPE_CAP: usize = 2;
const WRAP_WIDTH: usize = 88;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum MemberKind {
    Field,
    Method,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct MemberSummary {
    name: String,
    kind: MemberKind,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
struct MemberCatalog {
    members: Vec<MemberSummary>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct CrossTypeSuggestion {
    owner: String,
    member: String,
    kind: MemberKind,
    relationship_hint: String,
    owner_in_query_file: bool,
}

pub(super) fn format_missing_member(
    manifest: &Manifest,
    root: &Path,
    guidance: ReadSymbolGuidance,
    name: &str,
    member_name: &str,
    class_name: &str,
    file: &str,
) -> String {
    format_missing_member_with_catalog(
        manifest,
        guidance,
        name,
        member_name,
        class_name,
        file,
        collect_member_catalog(manifest, root, file, class_name),
    )
}

fn format_missing_member_with_catalog(
    manifest: &Manifest,
    guidance: ReadSymbolGuidance,
    name: &str,
    member_name: &str,
    class_name: &str,
    file: &str,
    catalog: Option<MemberCatalog>,
) -> String {
    let Some(catalog) = catalog.filter(|catalog| !catalog.members.is_empty()) else {
        return guidance.short_missing_member(name, member_name, class_name, file);
    };

    let mut lines = vec![format!(
        "Member '{}' not found. '{}' is not a member of '{}'.",
        name, member_name, class_name
    )];

    let suggestions = suggest_members(member_name, &catalog.members);
    if !suggestions.is_empty() {
        lines.push(format!("Did you mean: {}?", suggestions.join(", ")));
    }

    if let Some(cross_type) = format_cross_type_suggestions(&cross_type_suggestions(
        manifest,
        member_name,
        class_name,
        file,
    )) {
        lines.push(cross_type);
    }

    if let Some(fields) = format_member_group("Fields", catalog.by_kind(MemberKind::Field)) {
        lines.push(fields);
    }
    if let Some(methods) = format_member_group("Methods", catalog.by_kind(MemberKind::Method)) {
        lines.push(methods);
    }

    let total = catalog.members.len();
    let noun = if total == 1 { "member" } else { "members" };
    lines.push(format!(
        "({total} {noun} total; {}.)",
        full_list_hint(guidance, file)
    ));
    lines.join("\n")
}

fn collect_member_catalog(
    manifest: &Manifest,
    root: &Path,
    file: &str,
    class_name: &str,
) -> Option<MemberCatalog> {
    let entry = manifest.files.get(file)?;
    let prefix = format!("{class_name}.");
    let mut members = Vec::new();

    collect_indexed_members(
        &mut members,
        entry.methods.as_ref(),
        &entry.method_metadata,
        &prefix,
        MemberKind::Method,
    );
    collect_indexed_members(
        &mut members,
        Some(&entry.nested_fns),
        &entry.method_metadata,
        &prefix,
        MemberKind::Method,
    );
    collect_indexed_members(
        &mut members,
        Some(&entry.closure_state),
        &entry.method_metadata,
        &prefix,
        MemberKind::Field,
    );
    collect_private_members(&mut members, root, file, class_name);

    members.sort_by(|a, b| {
        a.start
            .cmp(&b.start)
            .then(a.end.cmp(&b.end))
            .then(a.name.cmp(&b.name))
    });
    members.dedup_by(|a, b| a.name == b.name && a.start == b.start && a.end == b.end);

    Some(MemberCatalog { members })
}

fn collect_indexed_members(
    members: &mut Vec<MemberSummary>,
    source: Option<&HashMap<String, ExportLines>>,
    metadata: &HashMap<String, SymbolMetadata>,
    prefix: &str,
    default_kind: MemberKind,
) {
    let Some(source) = source else {
        return;
    };
    for (dotted_name, lines) in source {
        if !dotted_name.starts_with(prefix) {
            continue;
        }
        members.push(MemberSummary {
            name: dotted_name.trim_start_matches(prefix).to_string(),
            kind: member_kind(metadata.get(dotted_name), default_kind),
            start: lines.start,
            end: lines.end,
        });
    }
}

fn collect_private_members(
    members: &mut Vec<MemberSummary>,
    root: &Path,
    file: &str,
    class_name: &str,
) {
    let by_class = private_members::extract_private_members(root, file, &[class_name]);
    let Some(private_members) = by_class.get(class_name) else {
        return;
    };
    for member in private_members {
        if members.iter().any(|existing| {
            existing.name == member.name
                && existing.start == member.start
                && existing.end == member.end
        }) {
            continue;
        }
        members.push(MemberSummary {
            name: member.name.clone(),
            kind: if member.is_method {
                MemberKind::Method
            } else {
                MemberKind::Field
            },
            start: member.start,
            end: member.end,
        });
    }
}

fn member_kind(metadata: Option<&SymbolMetadata>, default_kind: MemberKind) -> MemberKind {
    match metadata.and_then(|metadata| metadata.declaration_kind.as_deref()) {
        Some("field") => MemberKind::Field,
        Some("method" | "fn" | "function") => MemberKind::Method,
        _ => default_kind,
    }
}

impl MemberCatalog {
    fn by_kind(&self, kind: MemberKind) -> Vec<&MemberSummary> {
        self.members
            .iter()
            .filter(|member| member.kind == kind)
            .collect()
    }
}

impl MemberKind {
    fn article_label(self) -> &'static str {
        match self {
            Self::Field => "a field",
            Self::Method => "a method",
        }
    }
}

fn cross_type_suggestions(
    manifest: &Manifest,
    member_name: &str,
    class_name: &str,
    file: &str,
) -> Vec<CrossTypeSuggestion> {
    let mut suggestions = Vec::new();
    let mut seen = HashSet::new();

    for (entry_file, entry) in &manifest.files {
        for (dotted_name, metadata) in &entry.method_metadata {
            if metadata.declaration_kind.as_deref() != Some("field") {
                continue;
            }

            let Some((owner, member)) = split_member_name(dotted_name) else {
                continue;
            };
            if owner == class_name || member != member_name {
                continue;
            }

            let Some(signature) = metadata.signature.as_deref() else {
                continue;
            };
            if !signature_mentions_type(signature, class_name) {
                continue;
            }

            let key = format!("{entry_file}:{owner}.{member}");
            if seen.insert(key) {
                suggestions.push(CrossTypeSuggestion {
                    owner: owner.to_string(),
                    member: member.to_string(),
                    kind: MemberKind::Field,
                    relationship_hint: format!("type {class_name}"),
                    owner_in_query_file: entry_file == file,
                });
            }
        }
    }

    // Prefer owner fields in the queried type's file, then use stable names.
    suggestions.sort_by(|a, b| {
        b.owner_in_query_file
            .cmp(&a.owner_in_query_file)
            .then(a.owner.cmp(&b.owner))
            .then(a.member.cmp(&b.member))
            .then(a.relationship_hint.cmp(&b.relationship_hint))
    });
    suggestions.truncate(CROSS_TYPE_CAP);
    suggestions
}

fn split_member_name(dotted_name: &str) -> Option<(&str, &str)> {
    dotted_name.rsplit_once('.')
}

fn signature_mentions_type(signature: &str, class_name: &str) -> bool {
    let type_name = class_name
        .rsplit(['.', ':'])
        .find(|part| !part.is_empty())
        .unwrap_or(class_name);

    signature
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '$'))
        .any(|token| token == type_name)
}

fn format_cross_type_suggestions(suggestions: &[CrossTypeSuggestion]) -> Option<String> {
    if suggestions.is_empty() {
        return None;
    }

    let notes = suggestions
        .iter()
        .map(|suggestion| {
            format!(
                "'{}' is {} on {} ({})",
                suggestion.member,
                suggestion.kind.article_label(),
                suggestion.owner,
                suggestion.relationship_hint
            )
        })
        .collect::<Vec<_>>();

    Some(format!("Cross-type: {}.", notes.join("; ")))
}

fn suggest_members(member_name: &str, members: &[MemberSummary]) -> Vec<String> {
    let query = member_name.to_ascii_lowercase();
    if query.is_empty() {
        return Vec::new();
    }

    let mut substring_hits: Vec<&MemberSummary> = members
        .iter()
        .filter(|member| {
            let candidate = member.name.to_ascii_lowercase();
            candidate.contains(&query) || query.contains(&candidate)
        })
        .collect();

    if !substring_hits.is_empty() {
        substring_hits.sort_by(|a, b| {
            a.name
                .len()
                .cmp(&b.name.len())
                .then(a.start.cmp(&b.start))
                .then(a.name.cmp(&b.name))
        });
        return unique_names(substring_hits.into_iter());
    }

    let threshold = if query.chars().count() <= 8 { 2 } else { 3 };
    let mut distance_hits: Vec<(usize, &MemberSummary)> = members
        .iter()
        .filter_map(|member| {
            let distance = levenshtein(&query, &member.name.to_ascii_lowercase());
            (distance <= threshold).then_some((distance, member))
        })
        .collect();

    distance_hits.sort_by(|(left_distance, left), (right_distance, right)| {
        left_distance
            .cmp(right_distance)
            .then(left.name.len().cmp(&right.name.len()))
            .then(left.start.cmp(&right.start))
            .then(left.name.cmp(&right.name))
    });

    unique_names(distance_hits.into_iter().map(|(_, member)| member))
}

fn unique_names<'a>(members: impl Iterator<Item = &'a MemberSummary>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut names = Vec::new();
    for member in members {
        if seen.insert(member.name.as_str()) {
            names.push(member.name.clone());
        }
        if names.len() == SUGGESTION_CAP {
            break;
        }
    }
    names
}

fn levenshtein(left: &str, right: &str) -> usize {
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();
    if left.is_empty() {
        return right.len();
    }
    if right.is_empty() {
        return left.len();
    }

    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0; right.len() + 1];

    for (left_index, left_char) in left.iter().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_char) in right.iter().enumerate() {
            let substitution_cost = usize::from(left_char != right_char);
            current[right_index + 1] = (previous[right_index + 1] + 1)
                .min(current[right_index] + 1)
                .min(previous[right_index] + substitution_cost);
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous[right.len()]
}

fn format_member_group(label: &str, members: Vec<&MemberSummary>) -> Option<String> {
    if members.is_empty() {
        return None;
    }

    let mut names: Vec<String> = members
        .iter()
        .take(DISPLAY_CAP)
        .map(|member| member.name.clone())
        .collect();
    let hidden = members.len().saturating_sub(DISPLAY_CAP);
    if hidden > 0 {
        names.push(format!("... +{hidden} more"));
    }

    Some(wrap_items(&format!("{label}: "), &names))
}

fn wrap_items(prefix: &str, items: &[String]) -> String {
    let indent = " ".repeat(prefix.len());
    let mut lines = Vec::new();
    let mut current = prefix.to_string();

    for item in items {
        let separator = if current == prefix || current == indent {
            ""
        } else {
            ", "
        };
        if current.len() + separator.len() + item.len() > WRAP_WIDTH && current.len() > prefix.len()
        {
            current.push(',');
            lines.push(current);
            current = format!("{indent}{item}");
        } else {
            current.push_str(separator);
            current.push_str(item);
        }
    }

    lines.push(current);
    lines.join("\n")
}

fn full_list_hint(guidance: ReadSymbolGuidance, file: &str) -> String {
    match guidance {
        ReadSymbolGuidance::Cli => {
            format!("use fmm outline {file} --include-private for full list")
        }
        ReadSymbolGuidance::Mcp => {
            format!("use fmm_file_outline(file: \"{file}\", include_private: true) for full list")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(name: &str, kind: MemberKind, start: usize) -> MemberSummary {
        MemberSummary {
            name: name.to_string(),
            kind,
            start,
            end: start,
        }
    }

    #[test]
    fn substring_suggestions_prefer_short_source_ordered_matches() {
        let members = vec![
            member("validate_spawn_target", MemberKind::Method, 55),
            member("begin_spawn", MemberKind::Method, 28),
            member("cancel_spawn", MemberKind::Method, 104),
            member("pending_ready", MemberKind::Field, 17),
        ];

        assert_eq!(
            suggest_members("spawn", &members),
            vec!["begin_spawn", "cancel_spawn", "validate_spawn_target"]
        );
    }

    #[test]
    fn levenshtein_suggestions_run_when_substring_has_no_hits() {
        let members = vec![
            member("symbol", MemberKind::Field, 15),
            member("file", MemberKind::Field, 16),
            member("format_text", MemberKind::Method, 156),
        ];

        assert_eq!(suggest_members("symbl", &members), vec!["symbol"]);
    }

    #[test]
    fn missing_member_falls_back_without_catalog() {
        let manifest = Manifest::new();
        let text = format_missing_member_with_catalog(
            &manifest,
            ReadSymbolGuidance::Mcp,
            "Pool.clien",
            "clien",
            "Pool",
            "src/db/pool.ts",
            None,
        );

        assert_eq!(
            text,
            "Member 'Pool.clien' not found. 'clien' is not a member of 'Pool'. Use fmm_file_outline(file: \"src/db/pool.ts\", include_private: true) to see all members."
        );
    }

    #[test]
    fn signature_type_matching_uses_token_boundaries() {
        assert!(signature_mentions_type(
            "spawn: SpawnCoordinator",
            "SpawnCoordinator"
        ));
        assert!(signature_mentions_type(
            "spawn: Arc<SpawnCoordinator>",
            "SpawnCoordinator",
        ));
        assert!(signature_mentions_type(
            "spawn: Option<SpawnCoordinator>",
            "SpawnCoordinator",
        ));
        assert!(signature_mentions_type(
            "spawn: Vec<SpawnCoordinator>",
            "SpawnCoordinator",
        ));
        assert!(signature_mentions_type(
            "spawn: SpawnCoordinator<T>",
            "SpawnCoordinator",
        ));
        assert!(signature_mentions_type(
            "spawn: crate::runtime::SpawnCoordinator",
            "runtime.SpawnCoordinator",
        ));
        assert!(!signature_mentions_type(
            "spawn: SpawnCoordinatorHandle",
            "SpawnCoordinator",
        ));
        assert!(!signature_mentions_type(
            "spawn: SpawnCoordinatorFactory",
            "SpawnCoordinator",
        ));
    }

    #[test]
    fn member_group_caps_displayed_names() {
        let members: Vec<MemberSummary> = (0..22)
            .map(|i| member(&format!("doWork{i:03}"), MemberKind::Method, i))
            .collect();
        let text = format_member_group("Methods", members.iter().collect()).unwrap();

        assert!(text.contains("doWork000"), "got: {text}");
        assert!(text.contains("doWork019"), "got: {text}");
        assert!(!text.contains("doWork020"), "got: {text}");
        assert!(text.contains("... +2 more"), "got: {text}");
    }
}
