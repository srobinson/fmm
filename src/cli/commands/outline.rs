use anyhow::Result;
use colored::Colorize;

use super::{load_manifest, warn_no_sidecars};

#[derive(serde::Serialize)]
struct OutlineExportJson {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    lines: Option<[usize; 2]>,
}

#[derive(serde::Serialize)]
struct OutlineJson {
    file: String,
    exports: Vec<OutlineExportJson>,
    loc: usize,
}

pub fn outline(file: &str, json_output: bool) -> Result<()> {
    let (root, manifest) = load_manifest()?;

    if manifest.files.is_empty() {
        warn_no_sidecars();
        return Ok(());
    }

    if file.ends_with('/') || root.join(file).is_dir() {
        anyhow::bail!(
            "'{}' is a directory. Use {} to list files.",
            file,
            format!("fmm ls {}", file).bold()
        );
    }

    let entry = manifest.files.get(file).ok_or_else(|| {
        anyhow::anyhow!(
            "File '{}' not found in manifest. Run 'fmm generate' to index it.",
            file
        )
    })?;

    if json_output {
        let exports: Vec<OutlineExportJson> = entry
            .exports
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let lines = entry
                    .export_lines
                    .as_ref()
                    .and_then(|el| el.get(i))
                    .filter(|l| l.start > 0)
                    .map(|l| [l.start, l.end]);
                OutlineExportJson {
                    name: name.clone(),
                    lines,
                }
            })
            .collect();
        let json = OutlineJson {
            file: file.to_string(),
            exports,
            loc: entry.loc,
        };
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        println!("{}", crate::format::format_file_outline(file, entry, None));
    }

    Ok(())
}
