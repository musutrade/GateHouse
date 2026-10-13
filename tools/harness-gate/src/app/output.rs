pub(super) fn print_scope(scope: &crate::scope::ScopeResult) {
    println!("Scope: {}", scope.mode);
    println!("Changed files: {}", scope.changed_files.len());
    for file in &scope.changed_files {
        println!("  {file}");
    }
    if !scope.unmatched_files.is_empty() {
        println!("Unmatched files: {}", scope.unmatched_files.len());
        for file in &scope.unmatched_files {
            println!("  {file}");
        }
    }
    let components = scope
        .components
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    println!(
        "Components: {}",
        if components.is_empty() {
            "none"
        } else {
            &components
        }
    );
}
