//! PL/SW macro preprocessor.
//!
//! Parses MACRODEF definitions from .msw files and expands `?MACRO(...)`
//! invocations in .plsw source code.  Returns structured expansion data
//! for the UI to render with collapsible annotations.

use std::collections::HashMap;

/// A parsed macro definition from a .msw file.
#[derive(Clone, Debug)]
struct MacroDef {
    /// Macro name (e.g. "UART_INIT").
    name: String,
    /// Parameter names in order.
    params: Vec<String>,
    /// GEN block lines (raw template strings with `{PARAM}` placeholders).
    gen_lines: Vec<String>,
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

/// Parse all MACRODEF blocks from macro file content.
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
            let mut gen_lines = Vec::new();
            let mut in_gen = false;

            i += 1;
            while i < lines.len() {
                let line = lines[i].trim();

                // End of MACRODEF
                if line == "END;" {
                    break;
                }

                // Parameter declarations: REQUIRED NAME(type); or OPTIONAL NAME(type);
                if (line.starts_with("REQUIRED") || line.starts_with("OPTIONAL")) && !in_gen {
                    // Extract parameter name
                    let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
                    if parts.len() == 2 {
                        let param = parts[1].trim();
                        // Name is everything before '('
                        if let Some(paren) = param.find('(') {
                            params.push(param[..paren].trim().to_string());
                        } else {
                            params.push(param.trim_end_matches(';').trim().to_string());
                        }
                    }
                }

                // GEN DO; ... END;
                if line == "GEN DO;" {
                    in_gen = true;
                    i += 1;
                    continue;
                }

                if in_gen {
                    if line == "END;" {
                        in_gen = false;
                    } else {
                        // GEN lines are quoted strings: "assembly text";
                        let s = line.trim_end_matches(';').trim();
                        if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
                            gen_lines.push(s[1..s.len() - 1].to_string());
                        } else {
                            gen_lines.push(s.to_string());
                        }
                    }
                }

                i += 1;
            }

            defs.push(MacroDef {
                name,
                params,
                gen_lines,
            });
        }

        i += 1;
    }

    defs
}

/// Parse a macro invocation: `?NAME(PARAM=value, PARAM2=value2)`
/// Returns (name, args_map) or None.
fn parse_invocation(text: &str) -> Option<(String, HashMap<String, String>)> {
    let trimmed = text.trim().trim_end_matches(';').trim();
    if !trimmed.starts_with('?') {
        return None;
    }

    let rest = &trimmed[1..];
    let paren = rest.find('(')?;
    let name = rest[..paren].trim().to_string();
    let args_str = rest[paren + 1..].trim_end_matches(')').trim();

    let mut args = HashMap::new();
    if !args_str.is_empty() {
        for arg in args_str.split(',') {
            let arg = arg.trim();
            if let Some(eq) = arg.find('=') {
                let key = arg[..eq].trim().to_string();
                let val = arg[eq + 1..].trim().to_string();
                args.insert(key, val);
            }
        }
    }

    Some((name, args))
}

/// Expand a macro invocation given a definition and arguments.
fn expand_macro(def: &MacroDef, args: &HashMap<String, String>) -> Vec<String> {
    def.gen_lines
        .iter()
        .map(|line| {
            let mut expanded = line.clone();
            for param in &def.params {
                let placeholder = format!("{{{param}}}");
                if let Some(val) = args.get(param) {
                    expanded = expanded.replace(&placeholder, val);
                }
            }
            expanded
        })
        .collect()
}

/// Preprocess PL/SW source with the given macro files.
pub fn preprocess(source: &str, macro_sources: &[(String, String)]) -> PreprocessResult {
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

        // Check for %INCLUDE directives
        if trimmed.starts_with("%INCLUDE") {
            source_lines.push(SourceLine::Include(line.to_string()));
            output_lines.push(format!("/* {line} */"));
            continue;
        }

        // Check for ?MACRO(...) invocations
        if trimmed.starts_with('?') {
            if let Some((name, args)) = parse_invocation(trimmed)
                && let Some(def) = macro_defs.get(&name)
            {
                let expanded = expand_macro(def, &args);
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
    fn test_parse_macro_defs() {
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
        assert_eq!(defs[0].gen_lines.len(), 2);
        assert_eq!(defs[0].gen_lines[0], "lc r0, {PORT}");
    }

    #[test]
    fn test_expand_macro() {
        let def = MacroDef {
            name: "TEST".into(),
            params: vec!["X".into()],
            gen_lines: vec!["lc r0, {X}".into()],
        };
        let mut args = HashMap::new();
        args.insert("X".into(), "42".into());
        let result = expand_macro(&def, &args);
        assert_eq!(result, vec!["lc r0, 42"]);
    }

    #[test]
    fn test_preprocess_with_macros() {
        let source = "?UART_INIT(PORT=0xFF0100);\nMOVE X, Y;\n";
        let macros = vec![(
            "UART.msw".into(),
            r#"MACRODEF UART_INIT;
  REQUIRED PORT(expr);
  GEN DO;
    "lc r0, {PORT}";
  END;
END;"#
                .into(),
        )];
        let result = preprocess(source, &macros);
        assert_eq!(result.expansions.len(), 1);
        assert_eq!(result.expansions[0].macro_name, "UART_INIT");
        assert_eq!(result.expansions[0].expanded_lines, vec!["lc r0, 0xFF0100"]);
        assert!(result.output.contains("lc r0, 0xFF0100"));
    }
}
