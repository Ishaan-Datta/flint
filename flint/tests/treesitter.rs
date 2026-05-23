use flint::{
  ast::write::apply_flake_input_edits,
  errors::treesitter::TreesitterParseError,
};
use tracing_test::traced_test;

mod common;
use common::{
  INVALID_FLAKE_CONTENT,
  SHORT_FLAKE_CONTENT,
  VALID_FLAKE_CONTENT,
  assert_flake_eq,
  edits,
};

#[test]
#[traced_test]
fn inserts_follow_input_into_existing_attrset() {
  let input = r#"
{
  inputs = {
    foo = {
      url = "github:owner/foo";
    };
  };

  outputs = { self }: { };
}
"#;

  let expected = r#"
{
  inputs = {
    foo = {
      url = "github:owner/foo";
      inputs.systems.follows = "systems";
    };
  };

  outputs = { self }: { };
}
"#;

  let result = apply_flake_input_edits(
    input,
    &edits(&[("foo", &["inputs.systems.follows = \"systems\""])]),
  )
  .expect("flake input edits should apply");

  assert_flake_eq(expected, &result);
}

#[test]
#[traced_test]
fn inserts_multiple_follow_lines_into_existing_attrset_in_order() {
  let input = r#"
{
  inputs = {
    foo = {
      url = "github:owner/foo";
    };
  };

  outputs = { self }: { };
}
"#;

  let expected = r#"
{
  inputs = {
    foo = {
      url = "github:owner/foo";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.systems.follows = "systems";
      inputs.flake-utils.follows = "flake-utils";
    };
  };

  outputs = { self }: { };
}
"#;

  let result = apply_flake_input_edits(
    input,
    &edits(&[("foo", &[
      "inputs.nixpkgs.follows = \"nixpkgs\"",
      "inputs.systems.follows = \"systems\";",
      "inputs.flake-utils.follows = \"flake-utils\"",
    ])]),
  )
  .expect("flake input edits should apply");

  assert_flake_eq(expected, &result);
}

#[test]
#[traced_test]
fn preserves_existing_attrset_inner_indentation() {
  let input = r#"
{
    inputs = {
        foo = {
            url = "github:owner/foo";
        };
    };

    outputs = { self }: { };
}
"#;

  let expected = r#"
{
    inputs = {
        foo = {
            url = "github:owner/foo";
            inputs.nixpkgs.follows = "nixpkgs";
        };
    };

    outputs = { self }: { };
}
"#;

  let result = apply_flake_input_edits(
    input,
    &edits(&[("foo", &["inputs.nixpkgs.follows = \"nixpkgs\""])]),
  )
  .expect("flake input edits should apply");

  assert_flake_eq(expected, &result);
}

#[test]
#[traced_test]
fn trims_requested_lines_and_adds_missing_semicolons() {
  let input = r#"
{
  inputs = {
    foo = {
      url = "github:owner/foo";
    };
  };

  outputs = { self }: { };
}
"#;

  let expected = r#"
{
  inputs = {
    foo = {
      url = "github:owner/foo";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.systems.follows = "systems";
    };
  };

  outputs = { self }: { };
}
"#;

  let result = apply_flake_input_edits(
    input,
    &edits(&[("foo", &[
      "   inputs.nixpkgs.follows = \"nixpkgs\"   ",
      "   inputs.systems.follows = \"systems\";   ",
    ])]),
  )
  .expect("flake input edits should apply");

  assert_flake_eq(expected, &result);
}

#[test]
#[traced_test]
fn does_not_duplicate_existing_assignment_with_same_lhs() {
  let input = r#"
{
  inputs = {
    foo = {
      url = "github:owner/foo";
      inputs.nixpkgs.follows = "some-existing-pin";
    };
  };

  outputs = { self }: { };
}
"#;

  let result = apply_flake_input_edits(
    input,
    &edits(&[("foo", &["inputs.nixpkgs.follows = \"nixpkgs\""])]),
  )
  .expect("flake input edits should apply");

  assert_flake_eq(input, &result);
}

#[test]
#[traced_test]
fn rewrites_flat_url_binding_into_attrset_with_follow_inputs() {
  let input = r#"
{
  inputs = {
    foo.url = "github:owner/foo";
  };

  outputs = { self }: { };
}
"#;

  let expected = r#"
{
  inputs = {
    foo = {
      url = "github:owner/foo";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.systems.follows = "systems";
    };
  };

  outputs = { self }: { };
}
"#;

  let result = apply_flake_input_edits(
    input,
    &edits(&[("foo", &[
      "inputs.nixpkgs.follows = \"nixpkgs\"",
      "inputs.systems.follows = \"systems\"",
    ])]),
  )
  .expect("flake input edits should apply");

  assert_flake_eq(expected, &result);
}

#[test]
#[traced_test]
fn rewrites_flat_url_binding_and_preserves_outer_indentation() {
  let input = r#"
{
    inputs = {
        foo.url = "github:owner/foo";
    };

    outputs = { self }: { };
}
"#;

  let expected = r#"
{
    inputs = {
        foo = {
          url = "github:owner/foo";
          inputs.nixpkgs.follows = "nixpkgs";
        };
    };

    outputs = { self }: { };
}
"#;

  let result = apply_flake_input_edits(
    input,
    &edits(&[("foo", &["inputs.nixpkgs.follows = \"nixpkgs\""])]),
  )
  .expect("flake input edits should apply");

  assert_flake_eq(expected, &result);
}

#[test]
#[traced_test]
fn rewrites_flat_url_binding_with_complex_rhs_unchanged() {
  let input = r#"
{
  inputs = {
    foo.url = "git+https://example.com/foo?ref=main&rev=abcdef";
  };

  outputs = { self }: { };
}
"#;

  let expected = r#"
{
  inputs = {
    foo = {
      url = "git+https://example.com/foo?ref=main&rev=abcdef";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self }: { };
}
"#;

  let result = apply_flake_input_edits(
    input,
    &edits(&[("foo", &["inputs.nixpkgs.follows = \"nixpkgs\""])]),
  )
  .expect("flake input edits should apply");

  assert_flake_eq(expected, &result);
}

#[test]
#[traced_test]
fn applies_multiple_edits_across_flat_and_nested_inputs() {
  let input = r#"
{
  inputs = {
    foo.url = "github:owner/foo";

    bar = {
      url = "github:owner/bar";
    };

    baz.url = "github:owner/baz";
  };

  outputs = { self }: { };
}
"#;

  let expected = r#"
{
  inputs = {
    foo = {
      url = "github:owner/foo";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    bar = {
      url = "github:owner/bar";
      inputs.systems.follows = "systems";
    };

    baz = {
      url = "github:owner/baz";
      inputs.flake-utils.follows = "flake-utils";
    };
  };

  outputs = { self }: { };
}
"#;

  let result = apply_flake_input_edits(
    input,
    &edits(&[
      ("foo", &["inputs.nixpkgs.follows = \"nixpkgs\""]),
      ("bar", &["inputs.systems.follows = \"systems\""]),
      ("baz", &["inputs.flake-utils.follows = \"flake-utils\""]),
    ]),
  )
  .expect("flake input edits should apply");

  assert_flake_eq(expected, &result);
}

#[test]
#[traced_test]
fn leaves_unknown_inputs_unchanged() {
  let input = r#"
{
  inputs = {
    foo.url = "github:owner/foo";
  };

  outputs = { self }: { };
}
"#;

  let result = apply_flake_input_edits(
    input,
    &edits(&[("missing-input", &["inputs.nixpkgs.follows = \"nixpkgs\""])]),
  )
  .expect("flake input edits should apply");

  assert_flake_eq(input, &result);
}

#[test]
#[traced_test]
fn leaves_non_attrset_non_flat_input_binding_unchanged() {
  let input = r#"
{
  inputs = {
    foo = "github:owner/foo";
  };

  outputs = { self }: { };
}
"#;

  let result = apply_flake_input_edits(
    input,
    &edits(&[("foo", &["inputs.nixpkgs.follows = \"nixpkgs\""])]),
  )
  .expect("flake input edits should apply");

  assert_flake_eq(input, &result);
}

#[test]
#[traced_test]
fn inserts_into_empty_multiline_attrset_using_default_inner_indent() {
  let input = r#"
{
  inputs = {
    foo = {
    };
  };

  outputs = { self }: { };
}
"#;

  let expected = r#"
{
  inputs = {
    foo = {
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self }: { };
}
"#;

  let result = apply_flake_input_edits(
    input,
    &edits(&[("foo", &["inputs.nixpkgs.follows = \"nixpkgs\""])]),
  )
  .expect("flake input edits should apply");

  assert_flake_eq(expected, &result);
}

#[test]
#[traced_test]
fn inserts_after_comments_inside_existing_attrset() {
  let input = r#"
{
  inputs = {
    foo = {
      # keep this comment
      url = "github:owner/foo";
      # keep this trailing comment
    };
  };

  outputs = { self }: { };
}
"#;

  let expected = r#"
{
  inputs = {
    foo = {
      # keep this comment
      url = "github:owner/foo";
      # keep this trailing comment
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self }: { };
}
"#;

  let result = apply_flake_input_edits(
    input,
    &edits(&[("foo", &["inputs.nixpkgs.follows = \"nixpkgs\""])]),
  )
  .expect("flake input edits should apply");

  assert_flake_eq(expected, &result);
}

#[test]
#[traced_test]
fn preserves_blank_lines_around_edited_input_blocks() {
  let input = r#"
{
  inputs = {
    foo = {
      url = "github:owner/foo";
    };

    bar.url = "github:owner/bar";
  };

  outputs = { self }: { };
}
"#;

  let expected = r#"
{
  inputs = {
    foo = {
      url = "github:owner/foo";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    bar = {
      url = "github:owner/bar";
      inputs.systems.follows = "systems";
    };
  };

  outputs = { self }: { };
}
"#;

  let result = apply_flake_input_edits(
    input,
    &edits(&[
      ("foo", &["inputs.nixpkgs.follows = \"nixpkgs\""]),
      ("bar", &["inputs.systems.follows = \"systems\""]),
    ]),
  )
  .expect("flake input edits should apply");

  assert_flake_eq(expected, &result);
}

#[test]
#[traced_test]
fn handles_real_fixture_without_duplicating_existing_follows() {
  let result = apply_flake_input_edits(
    VALID_FLAKE_CONTENT,
    &edits(&[("buongiorno", &[
      "inputs.nixpkgs.follows = \"nixpkgs\"",
      "inputs.flake-utils.follows = \"flake-utils\"",
      "inputs.systems.follows = \"systems\"",
    ])]),
  )
  .expect("flake input edits should apply");

  assert_flake_eq(VALID_FLAKE_CONTENT, &result);
}

#[test]
#[traced_test]
fn handles_real_fixture_when_rewriting_existing_flat_binding() {
  let result = apply_flake_input_edits(
    VALID_FLAKE_CONTENT,
    &edits(&[("yazi", &["inputs.nixpkgs.follows = \"nixpkgs\""])]),
  )
  .expect("flake input edits should apply");

  assert!(
    result.contains(
      r#"    yazi = {
      url = "github:sxyazi/yazi";
      inputs.nixpkgs.follows = "nixpkgs";
    };"#
    ),
    "rewritten yazi input block missing or malformed:\n{}",
    result,
  );

  assert!(
    !result.contains(r#"    yazi.url = "github:sxyazi/yazi";"#),
    "old flat yazi binding should have been replaced:\n{}",
    result,
  );
}

#[test]
#[traced_test]
fn handles_real_fixture_when_inserting_into_existing_nested_input() {
  let result = apply_flake_input_edits(
    VALID_FLAKE_CONTENT,
    &edits(&[("vicinae", &[
      "inputs.nixpkgs.follows = \"nixpkgs\"",
      "inputs.systems.follows = \"systems\"",
    ])]),
  )
  .expect("flake input edits should apply");

  assert!(
    result.contains(
      r#"    vicinae = {
      url = "github:vicinaehq/vicinae";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.systems.follows = "systems";
    };"#
    ),
    "updated vicinae input block missing or malformed:\n{}",
    result,
  );
}

#[test]
#[traced_test]
fn returns_error_for_invalid_nix_syntax() {
  let result = apply_flake_input_edits(
    INVALID_FLAKE_CONTENT,
    &edits(&[("foo", &["inputs.nixpkgs.follows = \"nixpkgs\""])]),
  );

  assert!(
    matches!(result, Err(TreesitterParseError::SyntaxError)),
    "expected syntax error, got {result:?}",
  );
}

#[test]
#[traced_test]
fn returns_error_when_top_level_inputs_is_missing() {
  let result = apply_flake_input_edits(
    SHORT_FLAKE_CONTENT,
    &edits(&[("foo", &["inputs.nixpkgs.follows = \"nixpkgs\""])]),
  );

  assert!(
    matches!(result, Err(TreesitterParseError::MissingTopLevelInputs)),
    "expected missing top-level inputs error, got {result:?}",
  );
}

#[test]
#[traced_test]
fn returns_error_when_inputs_rhs_is_not_attrset() {
  let input = r#"
{
  inputs = "not-an-attrset";

  outputs = { self }: { };
}
"#;

  let result = apply_flake_input_edits(
    input,
    &edits(&[("foo", &["inputs.nixpkgs.follows = \"nixpkgs\""])]),
  );

  assert!(
    matches!(result, Err(TreesitterParseError::InputsNotAttrset(_))),
    "expected inputs-not-attrset error, got {result:?}",
  );
}
