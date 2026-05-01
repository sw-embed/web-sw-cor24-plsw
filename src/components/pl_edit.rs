//! PL/EDIT template expansion helpers.

use web_sys::HtmlTextAreaElement;
use yew::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PlEditLanguage {
    Source,
    Macro,
}

#[derive(Clone, Copy)]
pub struct PlEditTemplate {
    pub trigger: &'static str,
    pub label: &'static str,
    pub body: &'static str,
}

pub struct PlEditExpansion {
    pub source: String,
    pub cursor: u32,
    pub fields: Vec<u32>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct PlEditSession {
    pub fields: Vec<u32>,
    pub active: usize,
}

const SOURCE_TEMPLATES: &[PlEditTemplate] = &[
    PlEditTemplate {
        trigger: "IF",
        label: "IF/ELSE block",
        body: "IF ($0) THEN DO;\n    $1\nEND;\nELSE DO;\n    $2\nEND;",
    },
    PlEditTemplate {
        trigger: "IFS",
        label: "Single-statement IF",
        body: "IF ($0) THEN\n    $1",
    },
    PlEditTemplate {
        trigger: "DW",
        label: "DO WHILE block",
        body: "DO WHILE ($0);\n    $1\nEND;",
    },
    PlEditTemplate {
        trigger: "DO",
        label: "Counted DO block",
        body: "DO $0 = $1 TO $2;\n    $3\nEND;",
    },
    PlEditTemplate {
        trigger: "SEL",
        label: "SELECT/WHEN dispatch",
        body: "SELECT;\n    WHEN ($0) DO;\n        $1\n    END;\n    OTHERWISE DO;\n        $2\n    END;\nEND;",
    },
    PlEditTemplate {
        trigger: "WHEN",
        label: "WHEN branch",
        body: "WHEN ($0) DO;\n    $1\nEND;",
    },
    PlEditTemplate {
        trigger: "DCL",
        label: "Scalar DCL",
        body: "DCL $0 $1;",
    },
    PlEditTemplate {
        trigger: "REC",
        label: "Level DCL record",
        body: "DCL 1 $0,\n    3 $1 INT(24),\n        5 $2 INT(24),\n        5 $3(8) CHAR;",
    },
    PlEditTemplate {
        trigger: "BASED",
        label: "BASED record DCL",
        body: "DCL 1 $0 BASED,\n    3 $1 INT(24),\n        5 $2 PTR;",
    },
    PlEditTemplate {
        trigger: "P",
        label: "PROC block",
        body: "$0: PROC;\n    $1\nEND;",
    },
    PlEditTemplate {
        trigger: "PR",
        label: "PROC with return",
        body: "$0: PROC($1) RETURNS($2);\n    RETURN($3);\nEND;",
    },
    PlEditTemplate {
        trigger: "NAK",
        label: "NAKED PROC",
        body: "$0: PROC OPTIONS(NAKED);\n    ASM DO;\n        '$1';\n    END;\nEND;",
    },
    PlEditTemplate {
        trigger: "ASM",
        label: "ASM DO block",
        body: "ASM DO;\n    '$0';\n    $1\nEND;",
    },
    PlEditTemplate {
        trigger: "CALL",
        label: "CALL statement",
        body: "CALL $0($1);",
    },
    PlEditTemplate {
        trigger: "RET",
        label: "RETURN statement",
        body: "RETURN($0);",
    },
    PlEditTemplate {
        trigger: "RETV",
        label: "Void RETURN",
        body: "RETURN;",
    },
    PlEditTemplate {
        trigger: "G",
        label: "GOTO statement",
        body: "GOTO $0;",
    },
];

const MACRO_TEMPLATES: &[PlEditTemplate] = &[
    PlEditTemplate {
        trigger: "MD",
        label: "MACRODEF block",
        body: "MACRODEF $0;\n    REQUIRED $1(expr);\n    GEN DO;\n        '$2';\n    END;\nEND;",
    },
    PlEditTemplate {
        trigger: "REQ",
        label: "Required parameter",
        body: "REQUIRED $0($1);",
    },
    PlEditTemplate {
        trigger: "OPT",
        label: "Optional parameter",
        body: "OPTIONAL $0($1);",
    },
    PlEditTemplate {
        trigger: "GEN",
        label: "GEN block",
        body: "GEN DO;\n    '$0';\n    $1\nEND;",
    },
    PlEditTemplate {
        trigger: "INC",
        label: "%INCLUDE directive",
        body: "%INCLUDE '$0';",
    },
    PlEditTemplate {
        trigger: "INV",
        label: "Macro invocation",
        body: "?$0($1($2));",
    },
];

pub fn templates_for(language: PlEditLanguage) -> &'static [PlEditTemplate] {
    match language {
        PlEditLanguage::Source => SOURCE_TEMPLATES,
        PlEditLanguage::Macro => MACRO_TEMPLATES,
    }
}

pub fn expansion_templates_for(language: PlEditLanguage) -> Vec<&'static PlEditTemplate> {
    match language {
        PlEditLanguage::Source => SOURCE_TEMPLATES.iter().collect(),
        PlEditLanguage::Macro => MACRO_TEMPLATES
            .iter()
            .chain(SOURCE_TEMPLATES.iter())
            .collect(),
    }
}

pub fn format_source(source: &str) -> String {
    let mut out = Vec::new();
    let mut indent = 0usize;
    let mut dcl_continuation: Option<usize> = None;
    let mut record_dcl: Option<(usize, u32)> = None;

    for raw_line in source.lines() {
        let line = normalize_declaration_line(&normalize_code_line(raw_line));
        if line.is_empty() {
            out.push(String::new());
            continue;
        }

        let upper = line.to_ascii_uppercase();
        let mut line_indent = record_dcl
            .and_then(|(base_indent, base_level)| {
                leading_level(&line).map(|level| {
                    let depth = level.saturating_sub(base_level) / 2;
                    base_indent + depth as usize
                })
            })
            .or(dcl_continuation)
            .unwrap_or(indent);

        if upper.starts_with("END") {
            indent = indent.saturating_sub(1);
            line_indent = indent;
            dcl_continuation = None;
            record_dcl = None;
        }

        out.push(format!("{}{}", "    ".repeat(line_indent), line));

        if let Some(level) = record_dcl_start(&line)
            && line.ends_with(',')
        {
            record_dcl = Some((indent, level));
            dcl_continuation = Some(indent + 1);
        } else if upper.starts_with("DCL ") && line.ends_with(',') {
            dcl_continuation = Some(indent + 1);
        } else if dcl_continuation.is_some() {
            if line.ends_with(';') {
                dcl_continuation = None;
                record_dcl = None;
            }
        } else if opens_block(&upper) {
            indent += 1;
        }
    }

    let mut formatted = out.join("\n");
    if source.ends_with('\n') {
        formatted.push('\n');
    }
    formatted
}

fn normalize_code_line(raw_line: &str) -> String {
    let collapsed = collapse_spacing_outside_strings(raw_line.trim());
    if collapsed.is_empty() {
        return collapsed;
    }

    let chars: Vec<char> = collapsed.chars().collect();
    let mut out = String::with_capacity(collapsed.len());
    let mut in_string = false;
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];
        if ch == '\'' {
            in_string = !in_string;
            out.push(ch);
            i += 1;
            continue;
        }

        if !in_string && ch == ' ' {
            let next = chars.get(i + 1).copied();
            if matches!(next, Some(',') | Some(';') | Some(')')) {
                i += 1;
                continue;
            }
            if next == Some('(') && remove_space_before_paren(&out) {
                i += 1;
                continue;
            }
        }

        if !in_string && ch == '(' {
            out.push(ch);
            i += 1;
            while chars.get(i) == Some(&' ') {
                i += 1;
            }
            continue;
        }

        if !in_string && ch == ',' {
            out.push(ch);
            i += 1;
            while chars.get(i) == Some(&' ') {
                i += 1;
            }
            if i < chars.len() && !matches!(chars[i], ')' | ';') {
                out.push(' ');
            }
            continue;
        }

        out.push(ch);
        i += 1;
    }

    out
}

fn collapse_spacing_outside_strings(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_string = false;
    let mut pending_space = false;

    for ch in line.chars() {
        if ch == '\'' {
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            pending_space = false;
            in_string = !in_string;
            out.push(ch);
        } else if !in_string && ch.is_whitespace() {
            pending_space = true;
        } else {
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            pending_space = false;
            out.push(ch);
        }
    }

    out
}

fn normalize_declaration_line(line: &str) -> String {
    if line.contains('\'') {
        return line.to_string();
    }

    let parts: Vec<&str> = line.split(' ').collect();
    if parts.len() < 3 {
        return line.to_string();
    }

    let (name_idx, type_idx) = if parts[0].eq_ignore_ascii_case("DCL") {
        if parts.get(1).is_some_and(|part| part.parse::<u32>().is_ok()) {
            return line.to_string();
        }
        (1, 2)
    } else if parts[0].parse::<u32>().is_ok() {
        (1, 2)
    } else {
        return line.to_string();
    };

    let Some(type_part) = parts.get(type_idx) else {
        return line.to_string();
    };
    let upper_type = type_part.to_ascii_uppercase();
    let Some(dim_start) = upper_type.strip_prefix("CHAR(") else {
        return line.to_string();
    };
    let Some(close_idx) = dim_start.find(')') else {
        return line.to_string();
    };
    let dim = &dim_start[..close_idx];
    if dim.is_empty() || !dim.chars().all(|ch| ch.is_ascii_digit()) {
        return line.to_string();
    }
    let suffix = &type_part["CHAR(".len() + close_idx + 1..];
    if !matches!(suffix, "" | ";" | ",") {
        return line.to_string();
    }

    let mut normalized = parts
        .iter()
        .map(|part| (*part).to_string())
        .collect::<Vec<_>>();
    let name = normalized[name_idx].clone();
    if name.contains('(') {
        return line.to_string();
    }
    normalized[name_idx] = format!("{name}({dim})");
    normalized[type_idx] = format!("CHAR{suffix}");
    normalized.join(" ")
}

fn record_dcl_start(line: &str) -> Option<u32> {
    let mut parts = line.split_whitespace();
    if !parts.next()?.eq_ignore_ascii_case("DCL") {
        return None;
    }
    parts.next()?.parse().ok()
}

fn leading_level(line: &str) -> Option<u32> {
    line.split_whitespace().next()?.parse().ok()
}

fn remove_space_before_paren(prefix: &str) -> bool {
    let word = prefix
        .chars()
        .rev()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '?')
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>()
        .to_ascii_uppercase();

    !matches!(word.as_str(), "IF" | "WHILE" | "WHEN" | "SELECT")
}

fn opens_block(upper: &str) -> bool {
    upper.starts_with("SELECT;")
        || upper.starts_with("MACRODEF ")
        || upper.starts_with("GEN DO;")
        || upper.starts_with("ASM DO;")
        || (upper.contains(" PROC") && upper.ends_with(';'))
        || upper.starts_with("PROC ")
        || upper.ends_with(" THEN DO;")
        || upper.starts_with("ELSE DO;")
        || upper.starts_with("DO WHILE ")
        || (upper.starts_with("DO ") && upper.ends_with(';'))
        || upper.starts_with("WHEN ") && upper.ends_with(" DO;")
        || upper.starts_with("OTHERWISE DO;")
}

pub fn expand_at_cursor(
    textarea: &HtmlTextAreaElement,
    source: &str,
    language: PlEditLanguage,
) -> Option<PlEditExpansion> {
    let cursor = textarea.selection_start().ok().flatten()? as usize;
    let selection_end = textarea.selection_end().ok().flatten()? as usize;
    expand_source_at(source, cursor, selection_end, language)
}

pub fn expand_source_at(
    source: &str,
    cursor: usize,
    selection_end: usize,
    language: PlEditLanguage,
) -> Option<PlEditExpansion> {
    let start = previous_token_start(source, cursor);
    let trigger = source.get(start..cursor)?.trim().to_ascii_uppercase();
    if trigger.is_empty() {
        return None;
    }

    let template = expansion_templates_for(language)
        .into_iter()
        .find(|template| template.trigger == trigger)?;

    let indent = line_indent_before(source, start);
    let (body, fields) = render_body(template.body, &indent);
    let mut next = String::with_capacity(source.len() + body.len());
    next.push_str(source.get(..start)?);
    next.push_str(&body);
    next.push_str(source.get(selection_end..)?);

    let fields: Vec<u32> = fields
        .into_iter()
        .map(|field| (start + field) as u32)
        .collect();
    let cursor = fields
        .first()
        .copied()
        .unwrap_or((start + body.len()) as u32);

    Some(PlEditExpansion {
        source: next,
        cursor,
        fields,
    })
}

pub fn update_session_after_input(
    session: &PlEditSession,
    old_source: &str,
    new_source: &str,
    cursor: usize,
) -> PlEditSession {
    let delta = new_source.len() as isize - old_source.len() as isize;
    let edit_start = if delta > 0 {
        cursor.saturating_sub(delta as usize)
    } else {
        cursor
    };

    let mut fields = session.fields.clone();
    for (idx, field) in fields.iter_mut().enumerate() {
        if idx == session.active {
            *field = cursor as u32;
        } else if (*field as usize) > edit_start {
            *field = (*field as isize + delta).max(0) as u32;
        }
    }

    PlEditSession {
        fields,
        active: session.active,
    }
}

pub fn advance_session(session: &PlEditSession, backwards: bool) -> Option<PlEditSession> {
    if session.fields.is_empty() {
        return None;
    }

    let active = if backwards {
        session.active.checked_sub(1)?
    } else {
        let next = session.active + 1;
        if next >= session.fields.len() {
            return None;
        }
        next
    };

    Some(PlEditSession {
        fields: session.fields.clone(),
        active,
    })
}

fn previous_token_start(source: &str, cursor: usize) -> usize {
    let prefix = source.get(..cursor).unwrap_or(source);
    prefix
        .char_indices()
        .rev()
        .find(|(_, ch)| !(ch.is_ascii_alphanumeric() || *ch == '_'))
        .map_or(0, |(idx, ch)| idx + ch.len_utf8())
}

fn line_indent_before(source: &str, start: usize) -> String {
    let line_start = source
        .get(..start)
        .and_then(|s| s.rfind('\n'))
        .map_or(0, |i| i + 1);
    source
        .get(line_start..start)
        .unwrap_or_default()
        .chars()
        .take_while(|ch| *ch == ' ' || *ch == '\t')
        .collect()
}

fn render_body(template: &str, indent: &str) -> (String, Vec<usize>) {
    let mut out = String::with_capacity(template.len() + indent.len() * 4);
    let mut fields = Vec::new();
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '$'
            && let Some(next) = chars.peek()
            && next.is_ascii_digit()
        {
            fields.push(out.len());
            chars.next();
            continue;
        }

        out.push(ch);
        if ch == '\n' {
            out.push_str(indent);
        }
    }

    if fields.is_empty() {
        fields.push(out.len());
    }

    (out, fields)
}

pub fn render_pl_edit_help(language: PlEditLanguage) -> Html {
    html! {
        <div class="pl-edit-help">
            <div class="pl-edit-help-title">{"PL/EDIT Expansions"}</div>
            <div class="pl-edit-help-hotkeys">{"F4 expands. Ctrl+Space also expands. Tab or Enter advances fields. Ctrl+Enter inserts a line."}</div>
            <div class="pl-edit-help-grid">
                { for expansion_templates_for(language).into_iter().map(|template| html! {
                    <>
                        <span class="pl-edit-trigger">{template.trigger}</span>
                        <span>{template.label}</span>
                    </>
                })}
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_if_to_block_conditional() {
        let expansion = expand_source_at("    if", 6, 6, PlEditLanguage::Source).unwrap();

        assert_eq!(
            expansion.source,
            "    IF () THEN DO;\n        \n    END;\n    ELSE DO;\n        \n    END;"
        );
        assert_eq!(expansion.cursor, 8);
        assert_eq!(expansion.fields, vec![8, 27, 58]);
    }

    #[test]
    fn expands_select_dispatch() {
        let expansion = expand_source_at("SEL", 3, 3, PlEditLanguage::Source).unwrap();

        assert_eq!(
            expansion.source,
            "SELECT;\n    WHEN () DO;\n        \n    END;\n    OTHERWISE DO;\n        \n    END;\nEND;"
        );
        assert_eq!(expansion.cursor, 18);
    }

    #[test]
    fn expands_macrodef_with_gen_block() {
        let expansion = expand_source_at("MD", 2, 2, PlEditLanguage::Macro).unwrap();

        assert_eq!(
            expansion.source,
            "MACRODEF ;\n    REQUIRED (expr);\n    GEN DO;\n        '';\n    END;\nEND;"
        );
        assert_eq!(expansion.cursor, 9);
        assert_eq!(expansion.fields, vec![9, 24, 53]);
    }

    #[test]
    fn expands_source_control_flow_inside_macro_file() {
        let expansion = expand_source_at("IF", 2, 2, PlEditLanguage::Macro).unwrap();

        assert_eq!(
            expansion.source,
            "IF () THEN DO;\n    \nEND;\nELSE DO;\n    \nEND;"
        );
        assert_eq!(expansion.cursor, 4);
    }

    #[test]
    fn formats_nested_control_flow() {
        let source =
            "MAIN: PROC;\nIF (2 > 1) THEN DO;\nCALL A();\nEND;\nELSE DO;\nCALL B();\nEND;\nEND;\n";

        assert_eq!(
            format_source(source),
            "MAIN: PROC;\n    IF (2 > 1) THEN DO;\n        CALL A();\n    END;\n    ELSE DO;\n        CALL B();\n    END;\nEND;\n"
        );
    }

    #[test]
    fn formats_dcl_continuation() {
        let source = "DCL 1 POINT,\n3 X INT(24),\n5 Y CHAR(8);\n";

        assert_eq!(
            format_source(source),
            "DCL 1 POINT,\n    3 X INT(24),\n        5 Y(8) CHAR;\n"
        );
    }

    #[test]
    fn formats_record_dcl_inside_proc_by_level() {
        let source = "MAIN: PROC;\nDCL 1 FOO,\n3 BAR INT(24),\n5 BAT CHAR(8);\nEND;\n";

        assert_eq!(
            format_source(source),
            "MAIN: PROC;\n    DCL 1 FOO,\n        3 BAR INT(24),\n            5 BAT(8) CHAR;\nEND;\n"
        );
    }

    #[test]
    fn formats_macro_file_blocks() {
        let source = "ADD2: PROC(A INT(24), B INT(24)) RETURNS(INT(24));\nRETURN(A + B);\nEND;\nMACRODEF EMIT_NOP;\nREQUIRED COUNT(expr);\nGEN DO;\n'lc      r0,{COUNT}';\nEND;\nEND;\n";

        assert_eq!(
            format_source(source),
            "ADD2: PROC(A INT(24), B INT(24)) RETURNS(INT(24));\n    RETURN(A + B);\nEND;\nMACRODEF EMIT_NOP;\n    REQUIRED COUNT(expr);\n    GEN DO;\n        'lc      r0,{COUNT}';\n    END;\nEND;\n"
        );
    }

    #[test]
    fn normalizes_spacing_before_indent_decisions() {
        let source = "MACRODEF   EMIT_NOP ;\nREQUIRED   COUNT( expr ) ;\nGEN   DO ;\n'lc      r0,{COUNT}';\nEND ;\nEND ;\n";

        assert_eq!(
            format_source(source),
            "MACRODEF EMIT_NOP;\n    REQUIRED COUNT(expr);\n    GEN DO;\n        'lc      r0,{COUNT}';\n    END;\nEND;\n"
        );
    }

    #[test]
    fn normalizes_parameter_spacing_without_touching_strings() {
        let source = "ADD3:   PROC( A INT( 24 ) ,   B INT(24) , C INT(24) ) RETURNS( INT(24) ) ;\nRETURN( A + B + C ) ;\nCALL UART_PUTS( ADDR( APP_MSG ) ) ;\n'keep   generated   spaces';\nEND;\n";

        assert_eq!(
            format_source(source),
            "ADD3: PROC(A INT(24), B INT(24), C INT(24)) RETURNS(INT(24));\n    RETURN(A + B + C);\n    CALL UART_PUTS(ADDR(APP_MSG));\n    'keep   generated   spaces';\nEND;\n"
        );
    }
}
