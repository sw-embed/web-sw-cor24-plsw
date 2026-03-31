//! Source editor component with PL/SW syntax highlighting.
//!
//! Uses the overlay technique: a transparent `<textarea>` sits on top of a
//! `<pre><code>` block that renders the highlighted source. The user types
//! in the textarea; the highlighted view updates in sync.

use wasm_bindgen::JsCast;
use web_sys::HtmlTextAreaElement;
use yew::prelude::*;

/// PL/SW keywords for syntax highlighting.
const KEYWORDS: &[&str] = &[
    "DCL", "PROC", "DO", "END", "IF", "THEN", "ELSE", "WHILE", "RETURN", "CALL", "GOTO", "BIT",
    "BYTE", "WORD", "INT", "CHAR", "PTR", "INIT", "TO", "BY", "FIXED", "BASED", "DEFINED",
    "STATIC", "AUTO", "ENTRY", "LABEL", "BUILTIN", "ADDR", "LENGTH", "SUBSTR", "NULL", "BEGIN",
    "ON", "REVERT", "SIGNAL",
];

/// Inline assembly block keyword.
const ASM_KEYWORD: &str = "ASM";

#[derive(Properties, PartialEq)]
pub struct SourceEditorProps {
    pub source: AttrValue,
    pub on_change: Callback<String>,
    #[prop_or("PL/SW Source".into())]
    pub title: AttrValue,
    #[prop_or_default]
    pub example_name: Option<AttrValue>,
    #[prop_or_default]
    pub readonly: bool,
}

#[function_component(SourceEditor)]
pub fn source_editor(props: &SourceEditorProps) -> Html {
    let source = props.source.clone();
    let highlighted = highlight_plsw(&source);

    let oninput = {
        let on_change = props.on_change.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(target) = e.target()
                && let Some(ta) = target.dyn_ref::<HtmlTextAreaElement>()
            {
                on_change.emit(ta.value());
            }
        })
    };

    // Sync scroll between textarea and highlighted pre
    let pre_ref = use_node_ref();
    let onscroll = {
        let pre_ref = pre_ref.clone();
        Callback::from(move |e: Event| {
            if let Some(target) = e.target()
                && let Some(ta) = target.dyn_ref::<HtmlTextAreaElement>()
                && let Some(pre) = pre_ref.cast::<web_sys::HtmlElement>()
            {
                pre.set_scroll_top(ta.scroll_top());
                pre.set_scroll_left(ta.scroll_left());
            }
        })
    };

    html! {
        <div class="notebook-cell" id="cell-source">
            <div class="cell-header">
                <span>{&props.title}</span>
                if let Some(name) = &props.example_name {
                    <span class="cell-header-example">{name}</span>
                }
            </div>
            <div class="cell-content editor-container">
                <pre class="editor-highlight" ref={pre_ref}>
                    <code>{ Html::from_html_unchecked(AttrValue::from(highlighted)) }</code>
                </pre>
                <textarea
                    class="editor-textarea"
                    spellcheck="false"
                    autocomplete="off"
                    value={source}
                    {oninput}
                    {onscroll}
                    readonly={props.readonly}
                />
            </div>
        </div>
    }
}

/// Simple PL/SW syntax highlighter producing HTML spans.
fn highlight_plsw(source: &str) -> String {
    let mut out = String::with_capacity(source.len() * 2);
    let chars: Vec<char> = source.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_asm = false;

    while i < len {
        // Block comments: /* ... */
        if i + 1 < len && chars[i] == '/' && chars[i + 1] == '*' {
            out.push_str("<span class=\"hl-comment\">");
            out.push_str(&escape("/*"));
            i += 2;
            while i < len {
                if i + 1 < len && chars[i] == '*' && chars[i + 1] == '/' {
                    out.push_str(&escape("*/"));
                    i += 2;
                    break;
                }
                out.push_str(&escape_char(chars[i]));
                i += 1;
            }
            out.push_str("</span>");
            continue;
        }

        // ASM inline: semicolon comments (only inside ASM DO blocks)
        if in_asm && chars[i] == ';' {
            out.push_str("<span class=\"hl-comment\">");
            while i < len && chars[i] != '\n' {
                out.push_str(&escape_char(chars[i]));
                i += 1;
            }
            out.push_str("</span>");
            continue;
        }

        // String literals: '...'
        if chars[i] == '\'' {
            out.push_str("<span class=\"hl-string\">");
            out.push_str(&escape_char(chars[i]));
            i += 1;
            while i < len && chars[i] != '\'' {
                out.push_str(&escape_char(chars[i]));
                i += 1;
            }
            if i < len {
                out.push_str(&escape_char(chars[i]));
                i += 1;
            }
            out.push_str("</span>");
            continue;
        }

        // Macro invocations: ?NAME
        if chars[i] == '?' && i + 1 < len && chars[i + 1].is_ascii_alphabetic() {
            out.push_str("<span class=\"hl-macro\">");
            out.push_str(&escape_char(chars[i]));
            i += 1;
            while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                out.push_str(&escape_char(chars[i]));
                i += 1;
            }
            out.push_str("</span>");
            continue;
        }

        // Numbers: 0x hex or decimal
        if chars[i].is_ascii_digit()
            || (chars[i] == '0' && i + 1 < len && (chars[i + 1] == 'x' || chars[i + 1] == 'X'))
        {
            out.push_str("<span class=\"hl-number\">");
            if chars[i] == '0' && i + 1 < len && (chars[i + 1] == 'x' || chars[i + 1] == 'X') {
                out.push_str(&escape_char(chars[i]));
                i += 1;
                out.push_str(&escape_char(chars[i]));
                i += 1;
                while i < len && chars[i].is_ascii_hexdigit() {
                    out.push_str(&escape_char(chars[i]));
                    i += 1;
                }
            } else {
                while i < len && chars[i].is_ascii_digit() {
                    out.push_str(&escape_char(chars[i]));
                    i += 1;
                }
            }
            out.push_str("</span>");
            continue;
        }

        // Identifiers and keywords
        if chars[i].is_ascii_alphabetic() || chars[i] == '_' {
            let start = i;
            while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let upper = word.to_ascii_uppercase();

            if upper == ASM_KEYWORD {
                in_asm = true;
                out.push_str("<span class=\"hl-asm\">");
                out.push_str(&escape(&word));
                out.push_str("</span>");
            } else if upper == "END" && in_asm {
                in_asm = false;
                out.push_str("<span class=\"hl-asm\">");
                out.push_str(&escape(&word));
                out.push_str("</span>");
            } else if KEYWORDS.iter().any(|k| *k == upper) {
                out.push_str("<span class=\"hl-keyword\">");
                out.push_str(&escape(&word));
                out.push_str("</span>");
            } else if in_asm {
                out.push_str("<span class=\"hl-asm-text\">");
                out.push_str(&escape(&word));
                out.push_str("</span>");
            } else {
                out.push_str(&escape(&word));
            }
            continue;
        }

        // Operators and punctuation
        out.push_str(&escape_char(chars[i]));
        i += 1;
    }

    out
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_char(c: char) -> String {
    match c {
        '&' => "&amp;".to_string(),
        '<' => "&lt;".to_string(),
        '>' => "&gt;".to_string(),
        _ => c.to_string(),
    }
}
