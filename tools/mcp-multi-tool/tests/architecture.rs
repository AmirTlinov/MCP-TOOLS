use std::{collections::HashMap, fs, path::PathBuf};

use once_cell::sync::Lazy;
use walkdir::WalkDir;

static RULES: Lazy<HashMap<&'static str, Vec<&'static str>>> = Lazy::new(|| {
    HashMap::from([
        ("domain", vec!["app", "adapters", "infra"]),
        ("app", vec!["adapters", "infra"]),
        ("adapters", vec!["app", "infra"]),
        ("infra", vec!["app"]),
        ("shared", vec!["app", "domain", "adapters", "infra"]),
    ])
});

#[test]
fn layering_contract_enforced() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src_root = manifest_dir.join("src");
    let mut violations = Vec::new();

    for entry in WalkDir::new(&src_root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        if entry.path().extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let relative = entry.path().strip_prefix(&src_root).unwrap();
        let mut components = relative.components();
        let Some(layer_component) = components.next() else {
            continue;
        };
        let Some(layer) = layer_component.as_os_str().to_str() else {
            continue;
        };
        let Some(forbidden_layers) = RULES.get(layer) else {
            continue;
        };

        let content = fs::read_to_string(entry.path()).expect("read source file");
        for forbidden in forbidden_layers {
            let needle = format!("crate::{forbidden}");
            if content.contains(&needle) {
                violations.push(format!(
                    "{} must not depend on '{}'",
                    relative.display(),
                    forbidden
                ));
            }
        }
    }

    if !violations.is_empty() {
        panic!("layering violations:\n{}", violations.join("\n"));
    }
}
