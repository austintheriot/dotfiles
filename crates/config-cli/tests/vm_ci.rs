//! `tools/vm` is its own Bun package, so no other workflow runs its tests or
//! its typecheck. `.github/workflows/vm.yml` runs them when a push touches
//! the package.

use dotfiles_test_support::repo::{self, root as repo_root};

fn workflow() -> yaml_serde::Value {
    repo::read_workflow(&repo_root().join(".github/workflows/vm.yml"))
}

fn package_scripts() -> Vec<String> {
    let path = repo_root().join("tools/vm/package.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()));
    let package: yaml_serde::Value = yaml_serde::from_str(&text)
        .unwrap_or_else(|error| panic!("{} parses: {error}", path.display()));
    package
        .get("scripts")
        .and_then(yaml_serde::Value::as_mapping)
        .expect("tools/vm/package.json has a scripts table")
        .keys()
        .filter_map(|name| name.as_str().map(str::to_string))
        .collect()
}

fn path_filters(event: &str) -> Vec<String> {
    let workflow = workflow();
    repo::triggers_of(&workflow)
        .and_then(|triggers| triggers.get(event))
        .and_then(|trigger| trigger.get("paths"))
        .and_then(yaml_serde::Value::as_sequence)
        .map(|filters| {
            filters
                .iter()
                .filter_map(|filter| filter.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn the_workflow_runs_every_package_script() {
    let scripts = repo::run_scripts(&workflow());
    assert!(
        scripts.contains("bun install --frozen-lockfile"),
        "vm.yml does not install from the lockfile"
    );
    let missing: Vec<String> = package_scripts()
        .into_iter()
        .filter(|name| !scripts.contains(&format!("bun run {name}")))
        .collect();
    assert!(
        missing.is_empty(),
        "vm.yml does not run the tools/vm package scripts {missing:?}"
    );
}

#[test]
fn the_workflow_runs_on_vm_changes_only() {
    for event in ["push", "pull_request"] {
        let mut filters = path_filters(event);
        filters.sort();
        assert_eq!(
            filters,
            [".github/workflows/vm.yml", "tools/vm/**"],
            "vm.yml `{event}.paths`"
        );
    }
}
