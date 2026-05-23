use anyhow::{Result, anyhow};
use tree_sitter::{InputEdit, Node, Parser, Point, Tree};

#[derive(Debug, Clone)]
pub(crate) struct TextEdit {
    pub start_byte:   usize,
    pub old_end_byte: usize,
    pub new_text:     String,
}

/// Apply a text edit to the source and update the parse tree in-place.
///
/// # Arguments
///
/// * `source` - Source text to edit in-place.
/// * `tree` - Parse tree to edit and reparse.
/// * `parser` - Parser used for incremental reparsing.
/// * `edit` - The byte range and replacement text to apply.
///
/// # Returns
///
/// Returns `Ok(())` after the edit is applied and the tree is reparsed.
///
/// # Errors
///
/// Returns an error if incremental reparsing fails.
pub(crate) fn apply_edit(
    source: &mut String,
    tree: &mut Tree,
    parser: &mut Parser,
    edit: &TextEdit,
) -> Result<()> {
    let start_byte = edit.start_byte;
    let old_end_byte = edit.old_end_byte;
    let new_end_byte = start_byte + edit.new_text.len();

    let start_position = byte_to_point(source, start_byte);
    let old_end_position = byte_to_point(source, old_end_byte);
    let new_end_position =
        point_after_insertion(start_position, &edit.new_text);

    source.replace_range(start_byte..old_end_byte, &edit.new_text);

    tree.edit(&InputEdit {
        start_byte,
        old_end_byte,
        new_end_byte,
        start_position,
        old_end_position,
        new_end_position,
    });

    *tree = parser
        .parse(source.as_str(), Some(tree))
        .ok_or_else(|| anyhow!("incremental reparse failed"))?;

    Ok(())
}

/// Find the first attrset node in a depth-first traversal.
///
/// # Arguments
///
/// * `node` - Node to search from.
///
/// # Returns
///
/// Returns the first attrset node if present.
pub(crate) fn find_first_attrset(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() == "attrset_expression" || node.kind() == "attr_set" {
        return Some(node);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if let Some(found) = find_first_attrset(child) {
            return Some(found);
        }
    }
    None
}

/// Collect all binding nodes within an attrset.
///
/// # Arguments
///
/// * `attrset` - Attrset node to scan.
///
/// # Returns
///
/// Returns a list of binding-like nodes found in the attrset.
pub(crate) fn bindings_in_attrset(attrset: Node<'_>) -> Vec<Node<'_>> {
    let mut out = Vec::new();
    collect(attrset, &mut out);
    out
}

/// Recursively collect binding nodes from nested binding sets.
///
/// # Arguments
///
/// * `node` - Node to traverse.
/// * `out` - Accumulator for found bindings.
///
/// # Returns
///
/// Returns `()` after updating `out`.
fn collect<'a>(node: Node<'a>, out: &mut Vec<Node<'a>>) {
    let mut cursor = node.walk();

    for child in node.named_children(&mut cursor) {
        if child.kind() == "binding_set" {
            collect(child, out);
        } else if looks_like_binding(child) {
            out.push(child);
        }
    }
}

/// Find the top-level `inputs` binding in the first attrset.
///
/// # Arguments
///
/// * `root` - Root node of the parsed file.
/// * `source` - Full source text for extracting names.
///
/// # Returns
///
/// Returns the binding node for `inputs` if found.
pub(crate) fn find_top_level_inputs_binding<'a>(
    root: Node<'a>,
    source: &str,
) -> Option<Node<'a>> {
    let top_attrset = find_first_attrset(root)?;
    bindings_in_attrset(top_attrset)
        .into_iter()
        .find(|&binding| is_binding_named(binding, source, "inputs"))
}

/// Find a direct binding by name within a given attrset.
///
/// # Arguments
///
/// * `attrset` - Attrset node to scan.
/// * `source` - Full source text for extracting names.
/// * `wanted` - Binding name to match.
///
/// # Returns
///
/// Returns the matching binding node if present.
pub(crate) fn find_attrset_binding_by_name<'a>(
    attrset: Node<'a>,
    source: &str,
    wanted: &str,
) -> Option<Node<'a>> {
    for binding in bindings_in_attrset(attrset) {
        let path = binding_name_path(binding, source)?;
        if path.len() == 1 && path[0] == wanted {
            return Some(binding);
        }
    }

    None
}

/// Find a binding with a two-part attrpath like `base.leaf`.
///
/// # Arguments
///
/// * `attrset` - Attrset node to scan.
/// * `source` - Full source text for extracting names.
/// * `base` - First segment of the attrpath.
/// * `leaf` - Second segment of the attrpath.
///
/// # Returns
///
/// Returns the matching binding node if present.
pub(crate) fn find_flat_attrpath_binding<'a>(
    attrset: Node<'a>,
    source: &str,
    base: &str,
    leaf: &str,
) -> Option<Node<'a>> {
    for binding in bindings_in_attrset(attrset) {
        let path = binding_name_path(binding, source)?;
        if path.len() == 2 && path[0] == base && path[1] == leaf {
            return Some(binding);
        }
    }

    None
}

/// Check whether a node kind matches a binding-like syntax node.
///
/// # Arguments
///
/// * `node` - Node to test.
///
/// # Returns
///
/// Returns `true` if the node looks like a binding.
pub(crate) fn looks_like_binding(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "binding"
            | "bind"
            | "attrpath_binding"
            | "attrpath_value"
            | "assignment"
    )
}

/// Check whether a binding node matches a single-name binding.
///
/// # Arguments
///
/// * `node` - Binding node to test.
/// * `source` - Full source text for extracting names.
/// * `wanted` - Binding name to match.
///
/// # Returns
///
/// Returns `true` if the binding path is exactly `wanted`.
pub(crate) fn is_binding_named(
    node: Node<'_>,
    source: &str,
    wanted: &str,
) -> bool {
    binding_name_path(node, source)
        .is_some_and(|p| p.len() == 1 && p[0] == wanted)
}

/// Parse the left-hand side of a binding into attrpath segments.
///
/// # Arguments
///
/// * `node` - Binding node to inspect.
/// * `source` - Full source text for extracting names.
///
/// # Returns
///
/// Returns the attrpath segments if a left-hand side exists.
pub(crate) fn binding_name_path(
    node: Node<'_>,
    source: &str,
) -> Option<Vec<String>> {
    let text = &source[node.byte_range()];
    let lhs = text.split('=').next()?.trim();

    if lhs.is_empty() {
        return None;
    }

    Some(parse_attrpath_text(lhs))
}

/// Return a named child by field name, or the last named child as a fallback.
///
/// # Arguments
///
/// * `node` - Node to query.
/// * `field` - Field name to look up.
///
/// # Returns
///
/// Returns the named child node if present.
pub(crate) fn child_by_field_name_or_last_named<'a>(
    node: Node<'a>,
    field: &str,
) -> Option<Node<'a>> {
    node.child_by_field_name(field).or_else(|| {
        let mut cursor = node.walk();
        node.named_children(&mut cursor).last()
    })
}

/// Parse a dotted attrpath string into a list of segments.
///
/// # Arguments
///
/// * `text` - Attrpath string, possibly containing quotes.
///
/// # Returns
///
/// Returns the non-empty attrpath segments.
pub(crate) fn parse_attrpath_text(text: &str) -> Vec<String> {
    text.split('.')
        .map(str::trim)
        .map(|s| s.trim_matches('"'))
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

/// Filter wanted lines to those missing from an attrset.
///
/// # Arguments
///
/// * `attrset_rhs` - Attrset RHS node to check.
/// * `source` - Full source text for extracting names.
/// * `wanted_lines` - Lines to test for missing assignments.
///
/// # Returns
///
/// Returns the subset of `wanted_lines` not present in the attrset.
pub(crate) fn filter_missing_insertions(
    attrset_rhs: Node<'_>,
    source: &str,
    wanted_lines: &[String],
) -> Vec<String> {
    wanted_lines
        .iter()
        .filter(|line| !attrset_contains_assignment(attrset_rhs, source, line))
        .cloned()
        .collect()
}

/// Check whether an attrset already contains an assignment for a given line.
///
/// # Arguments
///
/// * `attrset_rhs` - Attrset RHS node to check.
/// * `source` - Full source text for extracting names.
/// * `line` - Assignment line whose LHS is used for matching.
///
/// # Returns
///
/// Returns `true` if a binding with the same attrpath exists.
pub(crate) fn attrset_contains_assignment(
    attrset_rhs: Node<'_>,
    source: &str,
    line: &str,
) -> bool {
    let lhs = line.split('=').next().map_or("", str::trim);
    let wanted_path = parse_attrpath_text(lhs).join(".");

    for binding in bindings_in_attrset(attrset_rhs) {
        if let Some(path) = binding_name_path(binding, source)
            && path.join(".") == wanted_path
        {
            return true;
        }
    }

    false
}

/// Create a text edit that inserts lines before an attrset closing brace.
///
/// # Arguments
///
/// * `attrset_rhs` - Attrset RHS node to edit.
/// * `source` - Full source text for extracting indentation.
/// * `lines` - Assignment lines to insert.
///
/// # Returns
///
/// Returns a `TextEdit` that inserts the requested lines.
pub(crate) fn insert_into_existing_attrset(
    attrset_rhs: Node<'_>,
    source: &str,
    lines: &[String],
) -> Result<TextEdit> {
    let text = &source[attrset_rhs.byte_range()];
    let close_rel = text
        .rfind('}')
        .ok_or_else(|| anyhow!("attrset text had no closing brace"))?;
    let close_line_start_rel =
        text[..close_rel].rfind('\n').map_or(close_rel, |i| i + 1);
    let close_abs = attrset_rhs.start_byte() + close_line_start_rel;

    let base_indent = detect_attrset_inner_indent(text)
        .unwrap_or_else(|| "      ".to_string());
    let mut inserted = String::new();

    for line in lines {
        inserted.push_str(&base_indent);
        inserted.push_str(line.trim());
        if !line.trim_end().ends_with(';') {
            inserted.push(';');
        }
        inserted.push('\n');
    }

    Ok(TextEdit {
        start_byte:   close_abs,
        old_end_byte: close_abs,
        new_text:     inserted,
    })
}

/// Rewrite a flat `input.url` binding into an attrset with extra lines.
///
/// # Arguments
///
/// * `flat_binding` - Binding node for the flat `input.url` assignment.
/// * `source` - Full source text for extracting the RHS.
/// * `input_name` - Name of the input attrset to emit.
/// * `lines` - Additional assignment lines to include in the attrset.
///
/// # Returns
///
/// Returns the replacement text for the rewritten attrset.
pub(crate) fn rewrite_flat_url_binding_to_attrset(
    flat_binding: Node<'_>,
    source: &str,
    input_name: &str,
    lines: &[String],
) -> Result<String> {
    let rhs = child_by_field_name_or_last_named(flat_binding, "value")
        .ok_or_else(|| anyhow!("flat binding missing RHS"))?;
    let rhs_text = source[rhs.byte_range()].trim();

    let outer_indent = line_indent_at(source, flat_binding.start_byte());
    let inner_indent = format!("{outer_indent}  ");

    let mut out = String::new();
    out.push_str(&outer_indent);
    out.push_str(input_name);
    out.push_str(" = {\n");

    out.push_str(&inner_indent);
    out.push_str("url = ");
    out.push_str(rhs_text);
    out.push_str(";\n");

    for line in lines {
        out.push_str(&inner_indent);
        out.push_str(line.trim());
        if !line.trim_end().ends_with(';') {
            out.push(';');
        }
        out.push('\n');
    }

    out.push_str(&outer_indent);
    out.push_str("};");
    Ok(out)
}

/// Detect the indentation used for entries inside an attrset.
///
/// # Arguments
///
/// * `attrset_text` - Text of the attrset, including braces.
///
/// # Returns
///
/// Returns the indentation prefix if an inner line is found.
pub(crate) fn detect_attrset_inner_indent(
    attrset_text: &str,
) -> Option<String> {
    for line in attrset_text.lines().skip(1) {
        let trimmed = line.trim();
        if !trimmed.is_empty() && trimmed != "}" {
            return Some(
                line.chars().take_while(|c| c.is_whitespace()).collect(),
            );
        }
    }
    None
}

/// Compute the whitespace indentation for the line containing `byte`.
///
/// # Arguments
///
/// * `source` - Full source text to inspect.
/// * `byte` - Byte offset within `source`.
///
/// # Returns
///
/// Returns the indentation prefix for the line.
pub(crate) fn line_indent_at(source: &str, byte: usize) -> String {
    let line_start = source[..byte].rfind('\n').map_or(0, |i| i + 1);
    source[line_start..byte]
        .chars()
        .take_while(|c| c.is_whitespace())
        .collect()
}

/// Return the byte offset for the start of the line containing `byte`.
///
/// # Arguments
///
/// * `source` - Full source text to inspect.
/// * `byte` - Byte offset within `source`.
///
/// # Returns
///
/// Returns the byte index of the line start.
pub(crate) fn line_start_byte_at(source: &str, byte: usize) -> usize {
    source[..byte].rfind('\n').map_or(0, |i| i + 1)
}

/// Convert a byte offset into a Tree-sitter point (row and column).
///
/// # Arguments
///
/// * `source` - Full source text to inspect.
/// * `byte` - Byte offset within `source`.
///
/// # Returns
///
/// Returns a `Point` with zero-based row and column measured in bytes.
pub(crate) fn byte_to_point(source: &str, byte: usize) -> Point {
    let mut row = 0usize;
    let mut column = 0usize;
    for b in source[..byte].bytes() {
        if b == b'\n' {
            row += 1;
            column = 0;
        } else {
            column += 1;
        }
    }
    Point { row, column }
}

/// Compute the end point after inserting text at a given start point.
///
/// # Arguments
///
/// * `start` - Starting point before insertion.
/// * `inserted` - Inserted text.
///
/// # Returns
///
/// Returns the `Point` after the inserted text, measured in bytes.
pub(crate) fn point_after_insertion(start: Point, inserted: &str) -> Point {
    let mut row = start.row;
    let mut column = start.column;

    for b in inserted.bytes() {
        if b == b'\n' {
            row += 1;
            column = 0;
        } else {
            column += 1;
        }
    }

    Point { row, column }
}
