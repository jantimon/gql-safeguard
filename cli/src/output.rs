use crate::args::OutputFormat;
use gql_safeguard_lib::types::violation::Violation;

pub fn format_violations(
    violations: &[Violation],
    format: &OutputFormat,
    show_trees: bool,
) -> String {
    match format {
        OutputFormat::Text => format_text(violations, show_trees),
        OutputFormat::Json => format_json(violations),
    }
}

fn format_text(violations: &[Violation], show_trees: bool) -> String {
    if violations.is_empty() {
        return "✅ No violations found! All @throwOnFieldError directives are properly protected."
            .to_string();
    }

    let mut output = format!(
        "Found {} violation{}:\n\n",
        violations.len(),
        if violations.len() == 1 { "" } else { "s" }
    );

    for (i, violation) in violations.iter().enumerate() {
        if i > 0 {
            output.push_str("\n");
        }
        output.push_str(&format_single_violation(violation, show_trees));
    }

    output
}

fn format_single_violation(violation: &Violation, show_trees: bool) -> String {
    let mut output = String::new();

    output.push_str(&format!(
        "Query: {} ({}:{})\n",
        violation.query_name,
        violation.file_location.path.display(),
        violation.file_location.line
    ));

    output.push_str(&format!("├── {}\n", violation.message));

    if show_trees {
        output.push_str("\nDependency Tree:\n");
        output.push_str("(Tree visualization would be shown here)\n");
    }

    output.push_str(&format!(
        "\nFix: Add @catch directive to protect @throwOnFieldError usage\n"
    ));

    output
}

fn format_json(violations: &[Violation]) -> String {
    serde_json::to_string_pretty(violations).unwrap_or_else(|_| "[]".to_string())
}
