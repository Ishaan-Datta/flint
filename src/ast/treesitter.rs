use anyhow::{Result, anyhow};
use tree_sitter::{InputEdit, Node, Parser, Point, Tree};

#[derive(Debug, Clone)]
pub(crate) struct TextEdit {
    pub start_byte: usize,
    pub old_end_byte: usize,
    pub new_text: String,
}

pub(crate) fn apply_edit(
    source: &mut String,
    tree: &mut Tree,
    parser: &mut Parser,
    edit: TextEdit,
) -> Result<()> {
    let start_byte = edit.start_byte;
    let old_end_byte = edit.old_end_byte;
    let new_end_byte = start_byte + edit.new_text.len();

    let start_position = byte_to_point(source, start_byte);
    let old_end_position = byte_to_point(source, old_end_byte);
    let new_end_position = point_after_insertion(start_position, &edit.new_text);

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

pub(crate) fn find_first_attrset<'a>(node: Node<'a>) -> Option<Node<'a>> {
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

pub(crate) fn bindings_in_attrset<'a>(attrset: Node<'a>) -> Vec<Node<'a>> {
    let mut out = Vec::new();

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

    collect(attrset, &mut out);
    out
}

pub(crate) fn find_top_level_inputs_binding<'a>(root: Node<'a>, source: &str) -> Option<Node<'a>> {
    let top_attrset = find_first_attrset(root)?;

    for binding in bindings_in_attrset(top_attrset) {
        if is_binding_named(binding, source, "inputs") {
            return Some(binding);
        }
    }

    None
}

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

pub(crate) fn looks_like_binding(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "binding" | "bind" | "attrpath_binding" | "attrpath_value" | "assignment"
    )
}

pub(crate) fn is_binding_named(node: Node<'_>, source: &str, wanted: &str) -> bool {
    binding_name_path(node, source)
        .map(|p| p.len() == 1 && p[0] == wanted)
        .unwrap_or(false)
}

pub(crate) fn binding_name_path(node: Node<'_>, source: &str) -> Option<Vec<String>> {
    let text = &source[node.byte_range()];
    let lhs = text.split('=').next()?.trim();

    if lhs.is_empty() {
        return None;
    }

    Some(parse_attrpath_text(lhs))
}

pub(crate) fn child_by_field_name_or_last_named<'a>(
    node: Node<'a>,
    field: &str,
) -> Option<Node<'a>> {
    node.child_by_field_name(field).or_else(|| {
        let mut cursor = node.walk();
        node.named_children(&mut cursor).last()
    })
}

pub(crate) fn parse_attrpath_text(text: &str) -> Vec<String> {
    text.split('.')
        .map(str::trim)
        .map(|s| s.trim_matches('"'))
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

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

pub(crate) fn attrset_contains_assignment(attrset_rhs: Node<'_>, source: &str, line: &str) -> bool {
    let lhs = line.split('=').next().map(str::trim).unwrap_or("");
    let wanted_path = parse_attrpath_text(lhs).join(".");

    for binding in bindings_in_attrset(attrset_rhs) {
        if let Some(path) = binding_name_path(binding, source) {
            if path.join(".") == wanted_path {
                return true;
            }
        }
    }

    false
}

pub(crate) fn insert_into_existing_attrset(
    attrset_rhs: Node<'_>,
    source: &str,
    lines: &[String],
) -> Result<TextEdit> {
    let text = &source[attrset_rhs.byte_range()];
    let close_rel = text
        .rfind('}')
        .ok_or_else(|| anyhow!("attrset text had no closing brace"))?;
    let close_line_start_rel = text[..close_rel]
        .rfind('\n')
        .map(|i| i + 1)
        .unwrap_or(close_rel);
    let close_abs = attrset_rhs.start_byte() + close_line_start_rel;

    let base_indent = detect_attrset_inner_indent(text).unwrap_or("      ".to_string());
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
        start_byte: close_abs,
        old_end_byte: close_abs,
        new_text: inserted,
    })
}

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

pub(crate) fn detect_attrset_inner_indent(attrset_text: &str) -> Option<String> {
    for line in attrset_text.lines().skip(1) {
        let trimmed = line.trim();
        if !trimmed.is_empty() && trimmed != "}" {
            return Some(line.chars().take_while(|c| c.is_whitespace()).collect());
        }
    }
    None
}

pub(crate) fn line_indent_at(source: &str, byte: usize) -> String {
    let line_start = source[..byte].rfind('\n').map(|i| i + 1).unwrap_or(0);
    source[line_start..byte]
        .chars()
        .take_while(|c| c.is_whitespace())
        .collect()
}

pub(crate) fn line_start_byte_at(source: &str, byte: usize) -> usize {
    source[..byte].rfind('\n').map(|i| i + 1).unwrap_or(0)
}

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
