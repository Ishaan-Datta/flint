use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum TreesitterParseError {
    #[error("failed to load tree-sitter-nix language")]
    LanguageLoad,
    #[error("tree-sitter parse failed")]
    ParseFailed,
    #[error("input has syntax errors")]
    SyntaxError,
    #[error("could not find top-level `inputs = {{ ... }};`")]
    MissingTopLevelInputs,
    #[error("inputs binding had no RHS")]
    InputsMissingRhs,
    #[error("`inputs` is not an attrset; found {0}")]
    InputsNotAttrset(String),
    #[error("binding `{0}` missing RHS")]
    BindingMissingRhs(String),
    #[error("attrset text had no closing brace")]
    AttrsetMissingClosingBrace,
    #[error("flat binding missing RHS")]
    FlatBindingMissingRhs,
    #[error("incremental reparse failed")]
    IncrementalReparseFailed,
}
