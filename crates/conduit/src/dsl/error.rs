use crate::dsl::parser::Rule;
use std::fmt;

/// Renders a Rust-style code snippet with a pointer to the error location.
///
/// Example output:
/// ```text
///   --> line 2, column 24
///    |
///  2 |         store::path <<-
///    |                        ^ expected value after assignment operator
///    |
/// ```
fn render_code_snippet(source: &str, line: usize, column: usize, label: &str) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let line_number_width = format!("{}", line).len().max(2);
    let mut output = String::new();

    output.push_str(&format!(
        "{:>width$}--> line {}, column {}\n",
        "",
        line,
        column,
        width = line_number_width
    ));

    output.push_str(&format!("{:>width$} |\n", "", width = line_number_width));

    let context_start = if line >= 3 { line - 2 } else { 1 };
    let context_end = (line + 1).min(lines.len());

    for line_index in context_start..=context_end {
        if line_index == 0 || line_index > lines.len() {
            continue;
        }

        let line_content = lines[line_index - 1];

        output.push_str(&format!(
            "{:>width$} | {}\n",
            line_index,
            line_content,
            width = line_number_width
        ));

        if line_index == line {
            let caret_offset = if column > 0 { column - 1 } else { 0 };

            output.push_str(&format!(
                "{:>width$} | {:>offset$}^ {}\n",
                "",
                "",
                label,
                width = line_number_width,
                offset = caret_offset
            ));
        }
    }

    output.push_str(&format!("{:>width$} |\n", "", width = line_number_width));

    output
}

/// Extracts line and column from a pest parse error.
fn extract_pest_location(error: &pest::error::Error<Rule>) -> (usize, usize) {
    match error.line_col {
        pest::error::LineColLocation::Pos((line, column)) => (line, column),
        pest::error::LineColLocation::Span((line, column), _) => (line, column),
    }
}

/// Formats the "expected" tokens from a pest error into human-readable text.
fn format_pest_expected(error: &pest::error::Error<Rule>) -> String {
    match &error.variant {
        pest::error::ErrorVariant::ParsingError { positives, negatives } => {
            let mut parts = Vec::new();

            if !positives.is_empty() {
                let names: Vec<String> = positives.iter().map(|rule| format!("`{:?}`", rule)).collect();

                parts.push(format!("expected {}", names.join(" or ")));
            }

            if !negatives.is_empty() {
                let names: Vec<String> = negatives.iter().map(|rule| format!("`{:?}`", rule)).collect();

                parts.push(format!("unexpected {}", names.join(" or ")));
            }

            if parts.is_empty() {
                "unexpected token".to_string()
            } else {
                parts.join(", ")
            }
        }
        pest::error::ErrorVariant::CustomError { message } => message.clone(),
    }
}

/// Holds optional source context for attaching code snippets to errors.
#[derive(Debug, Clone, Default)]
pub struct SourceContext {
    pub source: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

impl SourceContext {
    pub fn new(source: &str, line: usize, column: usize) -> Self {
        Self {
            source: Some(source.to_string()),
            line: Some(line),
            column: Some(column),
        }
    }

    pub fn from_source(source: &str) -> Self {
        Self {
            source: Some(source.to_string()),
            line: None,
            column: None,
        }
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn render_snippet(&self, label: &str) -> Option<String> {
        match (&self.source, self.line, self.column) {
            (Some(source), Some(line), Some(column)) => Some(render_code_snippet(source, line, column, label)),
            _ => None,
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ParserError {
    #[error("{}", format_module_not_defined_error(identifier, context))]
    ModuleNotDefined {
        identifier: String,
        #[source]
        context: SourceContext,
    },

    #[error("{}", format_property_conflict_error(identifier, property, context))]
    PropertyConflict {
        identifier: String,
        property: String,
        #[source]
        context: SourceContext,
    },

    #[error("{}", format_duplicated_node_error(identifier, context))]
    DuplicatedNode {
        identifier: String,
        #[source]
        context: SourceContext,
    },

    #[error("{}", format_invalid_number_error(context))]
    InvalidNumber {
        #[source]
        context: SourceContext,
    },

    #[error("{}", format_non_constant_expression_error(context))]
    NonConstantExpression {
        #[source]
        context: SourceContext,
    },

    #[error("{}", format_constant_not_found_error(name, context))]
    ConstantNotFound {
        name: String,
        #[source]
        context: SourceContext,
    },

    #[error("{}", format_non_numeric_value_error(context))]
    NonNumericValue {
        #[source]
        context: SourceContext,
    },

    #[error("{}", format_division_by_zero_error(context))]
    DivisionByZero {
        #[source]
        context: SourceContext,
    },

    #[error("{}", format_negative_exponent_error(context))]
    NegativeExponent {
        #[source]
        context: SourceContext,
    },

    #[error("{}", format_mixed_types_in_array_error(context))]
    MixedTypesInArray {
        #[source]
        context: SourceContext,
    },

    #[error("{}", format_append_to_non_array_error(identifier, property, context))]
    AppendToNonArray {
        identifier: String,
        property: String,
        #[source]
        context: SourceContext,
    },

    #[error("{}", format_syntax_error(source, raw_source))]
    SyntaxError {
        #[source]
        source: pest::error::Error<Rule>,
        raw_source: Option<String>,
    },
}

impl From<pest::error::Error<Rule>> for ParserError {
    fn from(error: pest::error::Error<Rule>) -> Self {
        ParserError::SyntaxError {
            source: error,
            raw_source: None,
        }
    }
}

/// Allows `SourceContext` to be used with `#[source]` in thiserror.
impl fmt::Display for SourceContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "")
    }
}

impl std::error::Error for SourceContext {}

// --- Formatting functions for each error variant ---

/// Describes a likely root cause found by analyzing the source code.
struct DiagnosedCause {
    line: usize,
    column: usize,
    description: String,
    help: String,
}

/// Scans the source for common structural issues that cause pest to report
/// errors at a misleading position (usually EOF). Returns a more helpful
/// diagnosis when one is found.
fn diagnose_source(source: &str, expected_rules: &[Rule]) -> Option<DiagnosedCause> {
    // Check for wrong keyword in for loop (e.g. `for x of` instead of `for x in`)
    if let Some(cause) = diagnose_wrong_for_loop_keyword(source) {
        return Some(cause);
    }

    // Check for unclosed parentheses inside string interpolation
    if let Some(cause) = diagnose_unclosed_paren_in_interpolation(source) {
        return Some(cause);
    }

    // Check for unclosed string interpolation braces inside strings
    if let Some(cause) = diagnose_unclosed_interpolation(source, expected_rules) {
        return Some(cause);
    }

    // Check for unclosed strings
    if let Some(cause) = diagnose_unclosed_string(source) {
        return Some(cause);
    }

    // Check for unclosed braces
    if let Some(cause) = diagnose_unclosed_braces(source) {
        return Some(cause);
    }

    // Check for missing value after assignment operator
    if let Some(cause) = diagnose_missing_value_after_operator(source) {
        return Some(cause);
    }

    None
}

/// Detects wrong keywords in for loops, such as `for x of [...]` instead of `for x in [...]`.
fn diagnose_wrong_for_loop_keyword(source: &str) -> Option<DiagnosedCause> {
    let lines: Vec<&str> = source.lines().collect();
    let wrong_keywords = ["of", "from", "each", "over", "through", "across"];

    for (line_index, line_content) in lines.iter().enumerate() {
        let trimmed = line_content.trim();

        if !trimmed.starts_with("for ") {
            continue;
        }

        // Parse: "for <identifier> <keyword> ..."
        let after_for = trimmed.strip_prefix("for ").unwrap().trim_start();
        // Find the identifier (first word after "for")
        let identifier_end = after_for
            .find(|character: char| character.is_whitespace())
            .unwrap_or(after_for.len());
        let identifier = &after_for[..identifier_end];

        if identifier.is_empty() {
            continue;
        }

        let after_identifier = after_for[identifier_end..].trim_start();

        for wrong_keyword in &wrong_keywords {
            if after_identifier.starts_with(wrong_keyword)
                && after_identifier[wrong_keyword.len()..]
                    .starts_with(|character: char| character.is_whitespace() || character == '[' || character == '{')
            {
                // Find the column where the wrong keyword starts
                let keyword_position = line_content.find(wrong_keyword).unwrap_or(0) + 1;

                return Some(DiagnosedCause {
                    line: line_index + 1,
                    column: keyword_position,
                    description: format!(
                        "unexpected keyword `{}` in for loop — did you mean `in`?",
                        wrong_keyword
                    ),
                    help: format!(
                        "The for loop on line {} uses `{}` but the correct keyword is `in`. \
                         Change `for {} {} ...` to `for {} in ...`.",
                        line_index + 1,
                        wrong_keyword,
                        identifier,
                        wrong_keyword,
                        identifier,
                    ),
                });
            }
        }
    }

    None
}

/// Detects unclosed parentheses inside string interpolation blocks.
/// E.g. `"{ (size + 1 }.png"` — the `(` is never closed with `)` before `}`.
fn diagnose_unclosed_paren_in_interpolation(source: &str) -> Option<DiagnosedCause> {
    let lines: Vec<&str> = source.lines().collect();

    for (line_index, line_content) in lines.iter().enumerate() {
        let mut in_string = false;
        let mut in_interpolation = false;
        let mut interpolation_brace_depth: i32 = 0;
        let mut paren_depth: i32 = 0;
        let mut unclosed_paren_column = None;
        let mut chars = line_content.chars().enumerate().peekable();

        while let Some((column_index, character)) = chars.next() {
            if character == '\\' && in_string {
                chars.next();
                continue;
            }

            if character == '"' {
                in_string = !in_string;
                if !in_string {
                    in_interpolation = false;
                    interpolation_brace_depth = 0;
                    paren_depth = 0;
                    unclosed_paren_column = None;
                }
                continue;
            }

            if in_string {
                if character == '{' {
                    interpolation_brace_depth += 1;
                    if interpolation_brace_depth == 1 {
                        in_interpolation = true;
                        paren_depth = 0;
                        unclosed_paren_column = None;
                    }
                } else if character == '}' {
                    if in_interpolation && paren_depth > 0 {
                        return Some(DiagnosedCause {
                            line: line_index + 1,
                            column: unclosed_paren_column.unwrap_or(column_index) + 1,
                            description: "unclosed `(` inside string interpolation".to_string(),
                            help: format!(
                                "The interpolation expression on line {} has an opening `(` that is \
                                 never closed with `)`. The `}}` closes the interpolation block before \
                                 the parenthesis is matched. Add the missing `)` before `}}`.",
                                line_index + 1
                            ),
                        });
                    }
                    interpolation_brace_depth -= 1;
                    if interpolation_brace_depth <= 0 {
                        in_interpolation = false;
                        interpolation_brace_depth = 0;
                    }
                } else if in_interpolation {
                    if character == '(' {
                        if paren_depth == 0 {
                            unclosed_paren_column = Some(column_index);
                        }
                        paren_depth += 1;
                    } else if character == ')' {
                        paren_depth -= 1;
                        if paren_depth == 0 {
                            unclosed_paren_column = None;
                        }
                    }
                }
            }
        }
    }

    None
}

/// Detects unclosed `{` inside a string interpolation.
/// E.g. `"{ prefix }.{ size.png"` — the second `{` is never closed with `}`.
/// Also detects unclosed string literals (missing closing `"`).
fn diagnose_unclosed_interpolation(source: &str, expected_rules: &[Rule]) -> Option<DiagnosedCause> {
    let is_interpolation_related = expected_rules
        .iter()
        .any(|rule| matches!(rule, Rule::string_content | Rule::interpolation));

    if !is_interpolation_related {
        return None;
    }

    let lines: Vec<&str> = source.lines().collect();

    for (line_index, line_content) in lines.iter().enumerate() {
        let mut in_string = false;
        let mut brace_depth: i32 = 0;
        let mut unclosed_brace_column = None;
        let mut string_start_column = None;
        let mut chars = line_content.chars().enumerate().peekable();

        while let Some((column_index, character)) = chars.next() {
            if character == '\\' {
                chars.next();
                continue;
            }

            if character == '"' {
                if in_string {
                    if brace_depth > 0 {
                        return Some(DiagnosedCause {
                            line: line_index + 1,
                            column: unclosed_brace_column.unwrap_or(column_index) + 1,
                            description: "unclosed interpolation brace `{` inside string".to_string(),
                            help: format!(
                                "The string on line {} has an opening `{{` for interpolation that is never \
                                 closed with `}}`. Each `{{` inside a string must have a matching `}}`. \
                                 Check that all interpolation expressions are properly closed.",
                                line_index + 1
                            ),
                        });
                    }
                    in_string = false;
                    brace_depth = 0;
                    string_start_column = None;
                } else {
                    in_string = true;
                    brace_depth = 0;
                    unclosed_brace_column = None;
                    string_start_column = Some(column_index);
                }
                continue;
            }

            if in_string {
                if character == '{' {
                    if brace_depth == 0 {
                        unclosed_brace_column = Some(column_index);
                    }
                    brace_depth += 1;
                } else if character == '}' {
                    brace_depth -= 1;
                }
            }
        }

        // If we reach end of line still inside a string, the string is unclosed
        if in_string {
            let opening_column = string_start_column.unwrap_or(0) + 1;
            return Some(DiagnosedCause {
                line: line_index + 1,
                column: opening_column,
                description: "unclosed string literal — missing closing `\"`".to_string(),
                help: format!(
                    "The string starting at line {}, column {} is never closed. \
                     Add a closing `\"` at the end of the string value.",
                    line_index + 1,
                    opening_column,
                ),
            });
        }
    }

    None
}

/// Detects unclosed string literals (odd number of unescaped quotes on a line).
fn diagnose_unclosed_string(source: &str) -> Option<DiagnosedCause> {
    let lines: Vec<&str> = source.lines().collect();

    for (line_index, line_content) in lines.iter().enumerate() {
        let trimmed = line_content.trim();

        if trimmed.starts_with('#') {
            continue;
        }

        let mut quote_count = 0;
        let mut chars = trimmed.chars().peekable();

        while let Some(character) = chars.next() {
            if character == '\\' {
                chars.next();
                continue;
            }

            if character == '"' {
                quote_count += 1;
            }
        }

        if quote_count % 2 != 0 {
            let quote_column = trimmed.find('"').unwrap_or(0);
            let leading_spaces = line_content.len() - line_content.trim_start().len();

            return Some(DiagnosedCause {
                line: line_index + 1,
                column: leading_spaces + quote_column + 1,
                description: "unclosed string literal".to_string(),
                help: format!(
                    "The string on line {} has an opening `\"` but no matching closing `\"`. \
                     Ensure every string is properly closed.",
                    line_index + 1
                ),
            });
        }
    }

    None
}

/// Detects unclosed braces `{` / `}` at the block level.
fn diagnose_unclosed_braces(source: &str) -> Option<DiagnosedCause> {
    let lines: Vec<&str> = source.lines().collect();
    let mut brace_stack: Vec<(usize, usize)> = Vec::new();
    let mut in_string = false;

    for (line_index, line_content) in lines.iter().enumerate() {
        let mut chars = line_content.chars().enumerate().peekable();

        while let Some((column_index, character)) = chars.next() {
            if character == '\\' && in_string {
                chars.next();
                continue;
            }

            if character == '"' {
                in_string = !in_string;
                continue;
            }

            if in_string {
                continue;
            }

            if character == '#' {
                break;
            }

            if character == '{' {
                brace_stack.push((line_index + 1, column_index + 1));
            } else if character == '}' {
                if brace_stack.is_empty() {
                    return Some(DiagnosedCause {
                        line: line_index + 1,
                        column: column_index + 1,
                        description: "unexpected closing brace `}`".to_string(),
                        help: format!(
                            "Found a closing `}}` on line {} without a matching opening `{{`.",
                            line_index + 1
                        ),
                    });
                }
                brace_stack.pop();
            }
        }
    }

    if let Some((line, column)) = brace_stack.last() {
        return Some(DiagnosedCause {
            line: *line,
            column: *column,
            description: "unclosed brace `{`".to_string(),
            help: format!(
                "The opening `{{` on line {} is never closed with a matching `}}`. \
                 Check that all blocks are properly closed.",
                line
            ),
        });
    }

    None
}

/// Detects missing values after `<-` or `->` at the end of a line.
fn diagnose_missing_value_after_operator(source: &str) -> Option<DiagnosedCause> {
    let lines: Vec<&str> = source.lines().collect();

    for (line_index, line_content) in lines.iter().enumerate() {
        let trimmed = line_content.trim();

        if trimmed.starts_with('#') {
            continue;
        }

        if trimmed.ends_with("<-") || trimmed.ends_with("<<-") || trimmed.ends_with("->") {
            let operator_position = line_content.rfind("<-").or_else(|| line_content.rfind("->"));

            if let Some(position) = operator_position {
                return Some(DiagnosedCause {
                    line: line_index + 1,
                    column: position + 1,
                    description: "missing value after assignment operator".to_string(),
                    help: format!(
                        "Line {} ends with an assignment operator (`<-`, `<<-`, or `->`) but no value follows it. \
                         Provide a value after the operator, e.g. `property <- 42` or `property <- \"text\"`.",
                        line_index + 1
                    ),
                });
            }
        }
    }

    None
}

fn format_syntax_error(pest_error: &pest::error::Error<Rule>, raw_source: &Option<String>) -> String {
    let (pest_line, pest_column) = extract_pest_location(pest_error);
    let expected = format_pest_expected(pest_error);

    let expected_rules: Vec<Rule> = match &pest_error.variant {
        pest::error::ErrorVariant::ParsingError { positives, .. } => positives.clone(),
        _ => Vec::new(),
    };

    let source_for_analysis = raw_source.as_deref().unwrap_or("");

    let diagnosis = diagnose_source(source_for_analysis, &expected_rules);

    let (error_line, error_column, root_cause_description, help_text) = if let Some(cause) = &diagnosis {
        (
            cause.line,
            cause.column,
            cause.description.as_str(),
            cause.help.as_str(),
        )
    } else {
        (pest_line, pest_column, "", "")
    };

    let source_to_render = if raw_source.is_some() {
        raw_source.as_deref().unwrap_or("")
    } else {
        ""
    };

    let lines: Vec<&str> = source_to_render.lines().collect();
    let has_source_lines = !lines.is_empty() && !source_to_render.is_empty();

    let display_description = if !root_cause_description.is_empty() {
        root_cause_description.to_string()
    } else {
        expected.clone()
    };

    let mut output = format!("Syntax Error: {}\n\n", display_description);

    let line_number_width = format!("{}", error_line).len().max(2);

    output.push_str(&format!(
        "{:>width$}--> line {}, column {}\n",
        "",
        error_line,
        error_column,
        width = line_number_width
    ));

    output.push_str(&format!("{:>width$} |\n", "", width = line_number_width));

    if has_source_lines && error_line <= lines.len() {
        let context_start = if error_line >= 3 { error_line - 2 } else { 1 };
        let context_end = (error_line + 1).min(lines.len());

        for display_line in context_start..=context_end {
            if display_line == 0 || display_line > lines.len() {
                continue;
            }

            let line_content = lines[display_line - 1];

            output.push_str(&format!(
                "{:>width$} | {}\n",
                display_line,
                line_content,
                width = line_number_width
            ));

            if display_line == error_line {
                let caret_offset = if error_column > 0 { error_column - 1 } else { 0 };

                output.push_str(&format!(
                    "{:>width$} | {:>offset$}^ {}\n",
                    "",
                    "",
                    display_description,
                    width = line_number_width,
                    offset = caret_offset
                ));
            }
        }
    } else {
        let line_content = pest_error.line().to_string();

        output.push_str(&format!(
            "{:>width$} | {}\n",
            error_line,
            line_content,
            width = line_number_width
        ));

        let caret_offset = if error_column > 0 { error_column - 1 } else { 0 };

        output.push_str(&format!(
            "{:>width$} | {:>offset$}^ {}\n",
            "",
            "",
            display_description,
            width = line_number_width,
            offset = caret_offset
        ));
    }

    output.push_str(&format!("{:>width$} |\n", "", width = line_number_width));

    if !help_text.is_empty() {
        output.push_str(&format!("Help: {}", help_text));
    } else {
        output.push_str(&format!(
            "Help: The parser encountered an unexpected token at line {}, column {}. ",
            pest_line, pest_column
        ));
        output.push_str("Check that the syntax follows the Conduit DSL grammar. ");
        output.push_str("Common causes include missing values after `<-` or `->`, ");
        output.push_str("unclosed braces `{}`, or invalid identifiers.");
    }

    output
}

fn format_module_not_defined_error(identifier: &str, context: &SourceContext) -> String {
    let mut output = format!(
        "Module Not Defined: '{}' is not defined in the current scope\n\n",
        identifier
    );

    if let Some(snippet) = context.render_snippet(&format!("module '{}' is not defined", identifier)) {
        output.push_str(&snippet);
        output.push('\n');
    }

    output.push_str(&format!(
        "Help: The identifier '{}' was referenced but no module with that name exists. ",
        identifier
    ));
    output.push_str("Ensure the module is declared before it is used. ");
    output.push_str(&format!(
        "Example: `{} module_name {{ property <- value }}`",
        identifier
    ));

    output
}

fn format_property_conflict_error(identifier: &str, property: &str, context: &SourceContext) -> String {
    let mut output = format!(
        "Property Conflict: property '{}' on module '{}' has conflicting assignments\n\n",
        property, identifier
    );

    if let Some(snippet) = context.render_snippet(&format!("conflicting property '{}'", property)) {
        output.push_str(&snippet);
        output.push('\n');
    }

    output.push_str(&format!(
        "Help: The property '{}' on '{}' is assigned more than once with conflicting directions or values. ",
        property, identifier
    ));
    output.push_str("Each property should have a single assignment direction (`<-` for input, `->` for output).");

    output
}

fn format_duplicated_node_error(identifier: &str, context: &SourceContext) -> String {
    let mut output = format!("Duplicated Node: '{}' is already defined\n\n", identifier);

    if let Some(snippet) = context.render_snippet(&format!("'{}' is already defined above", identifier)) {
        output.push_str(&snippet);
        output.push('\n');
    }

    output.push_str(&format!(
        "Help: A node named '{}' was declared more than once. ",
        identifier
    ));
    output.push_str("Each node must have a unique name within the pipeline. ");
    output.push_str("Either rename one of the nodes or remove the duplicate declaration.");

    output
}

fn format_invalid_number_error(context: &SourceContext) -> String {
    let mut output = "Invalid Number: the value could not be parsed as a valid number\n\n".to_string();

    if let Some(snippet) = context.render_snippet("not a valid number") {
        output.push_str(&snippet);
        output.push('\n');
    }

    output.push_str("Help: Numbers must be valid integers or decimals (e.g. `42`, `-3`, `3.14`). ");
    output.push_str("Ensure there are no extra characters, leading zeros (except `0.x`), or misplaced decimal points.");

    output
}

fn format_non_constant_expression_error(context: &SourceContext) -> String {
    let mut output = "Non-Constant Expression: this expression cannot be evaluated at parse time\n\n".to_string();

    if let Some(snippet) = context.render_snippet("cannot evaluate at parse time") {
        output.push_str(&snippet);
        output.push('\n');
    }

    output.push_str("Help: Expressions used in range bounds or compile-time contexts must be constant. ");
    output.push_str("Only literal numbers, identifiers with known constant values, and basic arithmetic are allowed.");

    output
}

fn format_constant_not_found_error(name: &str, context: &SourceContext) -> String {
    let mut output = format!("Constant Not Found: '{}' could not be resolved\n\n", name);

    if let Some(snippet) = context.render_snippet(&format!("'{}' is not defined or not constant", name)) {
        output.push_str(&snippet);
        output.push('\n');
    }

    output.push_str(&format!(
        "Help: The reference '{}' was expected to resolve to a constant value but was not found. ",
        name
    ));
    output.push_str("Ensure the identifier is defined before use and that it holds a constant (numeric) value.");

    output
}

fn format_non_numeric_value_error(context: &SourceContext) -> String {
    let mut output = "Non-Numeric Value: expected a numeric value but found a different type\n\n".to_string();

    if let Some(snippet) = context.render_snippet("expected numeric value") {
        output.push_str(&snippet);
        output.push('\n');
    }

    output.push_str("Help: This context requires a numeric value (integer or decimal). ");
    output.push_str("Strings, booleans, and other types cannot be used here. ");
    output.push_str("If you intended to use a number, remove any quotes around the value.");

    output
}

fn format_division_by_zero_error(context: &SourceContext) -> String {
    let mut output = "Division By Zero: cannot divide by zero in a constant expression\n\n".to_string();

    if let Some(snippet) = context.render_snippet("division by zero") {
        output.push_str(&snippet);
        output.push('\n');
    }

    output.push_str("Help: The divisor in this expression evaluates to zero, which is undefined. ");
    output.push_str("Change the divisor to a non-zero value.");

    output
}

fn format_negative_exponent_error(context: &SourceContext) -> String {
    let mut output = "Negative Exponent: negative exponents are not supported in constant expressions\n\n".to_string();

    if let Some(snippet) = context.render_snippet("negative exponent") {
        output.push_str(&snippet);
        output.push('\n');
    }

    output.push_str("Help: Only non-negative integer exponents are supported in compile-time expressions. ");
    output.push_str("Use a non-negative exponent value (e.g. `2 ^ 3` instead of `2 ^ -1`).");

    output
}

fn format_mixed_types_in_array_error(context: &SourceContext) -> String {
    let mut output = "Mixed Types In Array: all elements in an array must be the same type\n\n".to_string();

    if let Some(snippet) = context.render_snippet("mixed types in array") {
        output.push_str(&snippet);
        output.push('\n');
    }

    output.push_str("Help: Arrays in the DSL must contain elements of a single type. ");
    output.push_str("For example, `[1 2 3]` or `[\"a\" \"b\"]` are valid, but `[1 \"a\"]` is not. ");
    output.push_str("Ensure all elements have the same type.");

    output
}

fn format_append_to_non_array_error(identifier: &str, property: &str, context: &SourceContext) -> String {
    let mut output = format!(
        "Append To Non-Array: cannot append to '{}::{}' because it is not an array\n\n",
        identifier, property
    );

    if let Some(snippet) = context.render_snippet(&format!("'{}::{}' is not an array", identifier, property)) {
        output.push_str(&snippet);
        output.push('\n');
    }

    output.push_str(&format!(
        "Help: The `<<-` operator appends a value to an array, but '{}::{}' is not an array type. ",
        identifier, property
    ));
    output.push_str("Use `<-` to assign a single value, or change the property to an array type ");
    output.push_str(&format!(
        "(e.g. `{} _ {{ {} <- [] }}`) before appending.",
        identifier, property
    ));

    output
}
