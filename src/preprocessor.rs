//! PL/SW macro preprocessor.
//!
//! Parses MACRODEF definitions from .msw files and expands `?MACRO(...)`
//! invocations in .plsw source code.  Returns structured expansion data
//! for the UI to render with collapsible annotations.

use std::collections::HashMap;

/// A parsed macro definition from a .msw file. Bodies come in two flavors:
/// the legacy `GEN DO; '...'; END;` block of quoted assembly strings, and
/// the newer source-template body — direct PL/SW lines between the
/// `REQUIRED` clauses and the closing `END;`. The web preprocessor treats
/// both uniformly as `{PARAM}`-substituted line templates.
#[derive(Clone, Debug)]
struct MacroDef {
    /// Macro name (e.g. "GETMAIN").
    name: String,
    /// Parameter names in order.
    params: Vec<String>,
    /// Body template lines (with `{PARAM}` placeholders).
    body_lines: Vec<String>,
}

/// A single macro expansion in the preprocessed output.
#[derive(Clone, Debug, PartialEq)]
pub struct MacroExpansion {
    /// Original source line number (0-based).
    pub line_number: usize,
    /// The original invocation text (e.g. `?UART_INIT(PORT=0xFF0100);`).
    pub invocation: String,
    /// Name of the macro being invoked.
    pub macro_name: String,
    /// Expanded assembly lines from the GEN block.
    pub expanded_lines: Vec<String>,
}

/// Result of preprocessing a PL/SW source file.
#[derive(Clone, Debug, PartialEq)]
pub struct PreprocessResult {
    /// The preprocessed source with macro invocations replaced by comments.
    pub output: String,
    /// Individual macro expansions with their original locations.
    pub expansions: Vec<MacroExpansion>,
    /// Lines from the original source (for display).
    pub source_lines: Vec<SourceLine>,
}

/// A line in the preprocessed view -- either plain source or a macro invocation.
#[derive(Clone, Debug, PartialEq)]
pub enum SourceLine {
    /// Plain source line (not a macro invocation).
    Plain(String),
    /// A macro invocation line with its expansion index.
    Invocation { text: String, expansion_idx: usize },
    /// An %INCLUDE directive.
    Include(String),
}

/// Parse all MACRODEF blocks from macro file content. Bodies may be either:
///   * `GEN DO; 'asm string'; ... END;` — legacy assembler-emitting macros
///     like `?LED_SET`. Quotes are stripped; everything inside the GEN
///     block becomes a body template line.
///   * Direct source-template lines after the REQUIRED/OPTIONAL clauses,
///     terminated by the closing `END;` — used by `?GETMAIN`/`?FREEMAIN`
///     which expand to PL/SW source rather than assembly.
fn parse_macro_defs(source: &str) -> Vec<MacroDef> {
    let mut defs = Vec::new();
    let lines: Vec<&str> = source.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        // Look for MACRODEF <name>;
        if let Some(rest) = trimmed.strip_prefix("MACRODEF") {
            let rest = rest.trim().trim_end_matches(';').trim();
            let name = rest.to_string();
            let mut params = Vec::new();
            let mut body_lines = Vec::new();
            // None = haven't decided body flavor yet, Some(true) = inside
            // GEN DO block, Some(false) = source-template body started.
            let mut in_gen: Option<bool> = None;

            i += 1;
            while i < lines.len() {
                let raw = lines[i];
                let line = raw.trim();

                // End of MACRODEF (or end of GEN DO; if we're inside one)
                if line == "END;" {
                    if matches!(in_gen, Some(true)) {
                        in_gen = Some(false); // exit GEN block, keep scanning for outer END;
                        i += 1;
                        continue;
                    }
                    break;
                }

                // Parameter declarations: REQUIRED NAME(type); or
                // OPTIONAL NAME(type); — only outside GEN/source bodies.
                if (line.starts_with("REQUIRED") || line.starts_with("OPTIONAL"))
                    && !matches!(in_gen, Some(true))
                    && body_lines.is_empty()
                {
                    let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
                    if parts.len() == 2 {
                        let param = parts[1].trim();
                        if let Some(paren) = param.find('(') {
                            params.push(param[..paren].trim().to_string());
                        } else {
                            params.push(param.trim_end_matches(';').trim().to_string());
                        }
                    }
                    i += 1;
                    continue;
                }

                // GEN DO; opens a legacy assembler block.
                if line == "GEN DO;" {
                    in_gen = Some(true);
                    i += 1;
                    continue;
                }

                if matches!(in_gen, Some(true)) {
                    // Strip trailing ; and outer single/double quotes, if
                    // any, to recover the assembly template line.
                    let s = line.trim_end_matches(';').trim();
                    let stripped = strip_quotes(s);
                    body_lines.push(stripped.to_string());
                } else if !line.is_empty() {
                    // Source-template body line: keep verbatim (no quote
                    // stripping; these lines are real PL/SW source).
                    body_lines.push(line.to_string());
                }

                i += 1;
            }

            defs.push(MacroDef {
                name,
                params,
                body_lines,
            });
        }

        i += 1;
    }

    defs
}

/// Strip a single layer of matching outer quotes (single or double).
fn strip_quotes(s: &str) -> &str {
    if s.len() >= 2 {
        let bytes = s.as_bytes();
        let first = bytes[0];
        let last = bytes[s.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return &s[1..s.len() - 1];
        }
    }
    s
}

/// Parse a macro invocation. Two forms are accepted:
///   * PL/I-flavored (current upstream): `?NAME KEY(val) KEY(val);` —
///     space-separated parenthetical clauses, no outer parens, statement-
///     terminated. Used by every macro in the current `_plsw_storage.msw`,
///     `system.msw`, and `greet.msw`.
///   * Legacy: `?NAME(val)` or `?NAME(KEY(val), KEY(val))` — parenthesized
///     argument list with optional comma-separated keyword clauses. Kept
///     so old user programs keep parsing in the preview.
///
/// Returns (name, positional_args, named_args).
fn parse_invocation(text: &str) -> Option<(String, Vec<String>, HashMap<String, String>)> {
    let trimmed = text.trim().trim_end_matches(';').trim();
    let rest = trimmed.strip_prefix('?')?;

    // Read the macro name: identifier characters until whitespace or '('.
    let mut name_end = 0;
    for (idx, ch) in rest.char_indices() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            name_end = idx + ch.len_utf8();
        } else {
            break;
        }
    }
    if name_end == 0 {
        return None;
    }
    let name = rest[..name_end].to_string();
    let after_name = rest[name_end..].trim_start();

    let mut named = HashMap::new();
    let mut positional = Vec::new();

    if let Some(open_idx) = after_name.find('(') {
        // Legacy `?NAME(...)` — consume only when the opening paren comes
        // first (i.e. not preceded by an identifier clause name).
        if after_name[..open_idx].trim().is_empty() {
            let args_str = after_name[open_idx + 1..]
                .strip_suffix(')')
                .map(str::trim)
                .unwrap_or_else(|| after_name[open_idx + 1..].trim_end_matches(')').trim());
            if !args_str.is_empty() {
                for arg in split_macro_args(args_str, ',') {
                    let arg = arg.trim();
                    if let Some(eq) = arg.find('=') {
                        let key = arg[..eq].trim().to_string();
                        let val = arg[eq + 1..].trim().to_string();
                        named.insert(key, val);
                    } else if let Some((key, val)) = parse_keyword_arg(arg) {
                        named.insert(key, val);
                    } else {
                        positional.push(arg.to_string());
                    }
                }
            }
            return Some((name, positional, named));
        }
    }

    // PL/I-flavored: walk space-separated KEY(val) clauses. Splitting on
    // top-level whitespace means `KEY(a b)` keeps "a b" intact since the
    // parens raise depth.
    for clause in split_macro_args(after_name, ' ') {
        let clause = clause.trim().trim_end_matches(';').trim();
        if clause.is_empty() {
            continue;
        }
        if let Some((key, val)) = parse_keyword_arg(clause) {
            named.insert(key, val);
        } else {
            // Bare-token clause without parens: treat as positional so
            // unusual demos still expand something rather than nothing.
            positional.push(clause.to_string());
        }
    }

    Some((name, positional, named))
}

/// Split `args` on top-level occurrences of `delim` (depth-aware on parens).
fn split_macro_args(args: &str, delim: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut depth = 0usize;

    for (idx, ch) in args.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            c if c == delim && depth == 0 => {
                let segment = &args[start..idx];
                if !segment.is_empty() {
                    parts.push(segment);
                }
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }
    let tail = &args[start..];
    if !tail.is_empty() {
        parts.push(tail);
    }
    parts
}

fn parse_keyword_arg(arg: &str) -> Option<(String, String)> {
    let open = arg.find('(')?;
    if !arg.ends_with(')') {
        return None;
    }
    let key = arg[..open].trim();
    if key.is_empty()
        || !key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return None;
    }
    let val = arg[open + 1..arg.len() - 1].trim();
    Some((key.to_string(), val.to_string()))
}

/// Expand a macro invocation given a definition, positional args, and named args.
/// Positional args are matched to params in declaration order; named args override.
fn expand_macro(
    def: &MacroDef,
    positional: &[String],
    named: &HashMap<String, String>,
) -> Vec<String> {
    // Build resolved args: positional first, then named overrides
    let mut resolved: HashMap<&str, &str> = HashMap::new();
    for (i, param) in def.params.iter().enumerate() {
        if let Some(val) = positional.get(i) {
            resolved.insert(param, val);
        }
    }
    for (key, val) in named {
        resolved.insert(key, val);
    }

    def.body_lines
        .iter()
        .map(|line| {
            let mut expanded = line.clone();
            for param in &def.params {
                let placeholder = format!("{{{param}}}");
                if let Some(val) = resolved.get(param.as_str()) {
                    expanded = expanded.replace(&placeholder, val);
                }
            }
            expanded
        })
        .collect()
}

/// Preprocess PL/SW source with the given macro files.
/// Inlines %INCLUDE content and expands ?MACRO() invocations.
pub fn preprocess(source: &str, macro_sources: &[(String, String)]) -> PreprocessResult {
    // Build include lookup: strip .msw extension, case-insensitive
    let mut include_map: HashMap<String, &str> = HashMap::new();
    for (name, content) in macro_sources {
        let key = name
            .strip_suffix(".msw")
            .unwrap_or(name)
            .to_ascii_lowercase();
        include_map.insert(key, content.as_str());
    }

    // Parse all macro definitions from .msw files
    let mut macro_defs: HashMap<String, MacroDef> = HashMap::new();
    for (_name, msw_source) in macro_sources {
        for def in parse_macro_defs(msw_source) {
            macro_defs.insert(def.name.clone(), def);
        }
    }

    let mut output_lines = Vec::new();
    let mut source_lines = Vec::new();
    let mut expansions = Vec::new();

    for (line_num, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        // Check for %INCLUDE directives -- inline the file content
        if trimmed.starts_with("%INCLUDE") {
            let include_name = trimmed
                .strip_prefix("%INCLUDE")
                .unwrap_or("")
                .trim()
                .trim_end_matches(';')
                .trim()
                .to_ascii_lowercase();

            source_lines.push(SourceLine::Include(line.to_string()));

            if let Some(content) = include_map.get(&include_name) {
                output_lines.push(format!("/* --- {include_name}.msw --- */"));
                for inc_line in content.lines() {
                    output_lines.push(inc_line.to_string());
                }
                output_lines.push(format!("/* --- end {include_name}.msw --- */"));
            } else {
                output_lines.push(format!("/* {line} -- not found */"));
            }
            continue;
        }

        // Check for ?MACRO(...) invocations
        if trimmed.starts_with('?') {
            if let Some((name, positional, named)) = parse_invocation(trimmed)
                && let Some(def) = macro_defs.get(&name)
            {
                let expanded = expand_macro(def, &positional, &named);
                let exp_idx = expansions.len();

                expansions.push(MacroExpansion {
                    line_number: line_num,
                    invocation: line.to_string(),
                    macro_name: name,
                    expanded_lines: expanded.clone(),
                });

                source_lines.push(SourceLine::Invocation {
                    text: line.to_string(),
                    expansion_idx: exp_idx,
                });

                // In the output, replace invocation with expanded assembly
                output_lines.push(format!("/* {trimmed} */"));
                for exp_line in &expanded {
                    output_lines.push(format!("  {exp_line}"));
                }
                continue;
            }
            // Unknown macro -- pass through as-is
            source_lines.push(SourceLine::Plain(line.to_string()));
            output_lines.push(line.to_string());
        } else {
            source_lines.push(SourceLine::Plain(line.to_string()));
            output_lines.push(line.to_string());
        }
    }

    PreprocessResult {
        output: output_lines.join("\n"),
        expansions,
        source_lines,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gen_macro_defs() {
        let source = r#"
MACRODEF UART_INIT;
  REQUIRED PORT(expr);
  GEN DO;
    "lc r0, {PORT}";
    "st r0, [0xFF0100]";
  END;
END;
"#;
        let defs = parse_macro_defs(source);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "UART_INIT");
        assert_eq!(defs[0].params, vec!["PORT"]);
        assert_eq!(defs[0].body_lines.len(), 2);
        assert_eq!(defs[0].body_lines[0], "lc r0, {PORT}");
    }

    #[test]
    fn test_parse_source_template_macro_def() {
        let source = r#"MACRODEF GETMAIN;
    REQUIRED SET(lvalue);
    REQUIRED LENGTH(expr);
    REQUIRED RC(lvalue);
    {SET} = _PLSW_GETMAIN({LENGTH});
    IF ({SET} = 0) THEN {RC} = 4;
    ELSE {RC} = 0;
END;
"#;
        let defs = parse_macro_defs(source);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "GETMAIN");
        assert_eq!(defs[0].params, vec!["SET", "LENGTH", "RC"]);
        assert_eq!(
            defs[0].body_lines,
            vec![
                "{SET} = _PLSW_GETMAIN({LENGTH});",
                "IF ({SET} = 0) THEN {RC} = 4;",
                "ELSE {RC} = 0;",
            ],
        );
    }

    #[test]
    fn test_expand_macro_named() {
        let def = MacroDef {
            name: "TEST".into(),
            params: vec!["X".into()],
            body_lines: vec!["lc r0, {X}".into()],
        };
        let mut named = HashMap::new();
        named.insert("X".into(), "42".into());
        let result = expand_macro(&def, &[], &named);
        assert_eq!(result, vec!["lc r0, 42"]);
    }

    #[test]
    fn test_expand_macro_positional() {
        let def = MacroDef {
            name: "TEST".into(),
            params: vec!["CH".into()],
            body_lines: vec!["ld r0, {CH}".into()],
        };
        let result = expand_macro(&def, &["myvar".into()], &HashMap::new());
        assert_eq!(result, vec!["ld r0, myvar"]);
    }

    #[test]
    fn test_preprocess_pli_invocation() {
        let source = "?GETMAIN LENGTH(12) SET(P) RC(RC);\n";
        let macros = vec![(
            "_plsw_storage.msw".into(),
            r#"MACRODEF GETMAIN;
    REQUIRED SET(lvalue);
    REQUIRED LENGTH(expr);
    REQUIRED RC(lvalue);
    {SET} = _PLSW_GETMAIN({LENGTH});
    IF ({SET} = 0) THEN {RC} = 4;
    ELSE {RC} = 0;
END;"#
                .into(),
        )];
        let result = preprocess(source, &macros);
        assert_eq!(result.expansions.len(), 1);
        assert_eq!(result.expansions[0].macro_name, "GETMAIN");
        assert_eq!(
            result.expansions[0].expanded_lines,
            vec![
                "P = _PLSW_GETMAIN(12);",
                "IF (P = 0) THEN RC = 4;",
                "ELSE RC = 0;",
            ],
        );
    }

    #[test]
    fn test_preprocess_pli_gen_invocation() {
        let source = "?LED_SET VAL(0);\n";
        let macros = vec![(
            "system.msw".into(),
            r#"MACRODEF LED_SET;
    REQUIRED VAL(expr);
    GEN DO;
        'la r2,16711680';
        'lc r0,{VAL}';
        'sb r0,0(r2)';
    END;
END;"#
                .into(),
        )];
        let result = preprocess(source, &macros);
        assert_eq!(result.expansions.len(), 1);
        assert_eq!(
            result.expansions[0].expanded_lines,
            vec!["la r2,16711680", "lc r0,0", "sb r0,0(r2)"],
        );
    }
}
