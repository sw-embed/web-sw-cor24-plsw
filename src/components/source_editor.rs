//! Source editor component with PL/SW syntax highlighting.
//!
//! Uses the overlay technique: a transparent `<textarea>` sits on top of a
//! `<pre><code>` block that renders the highlighted source. The user types
//! in the textarea; the highlighted view updates in sync.

use crate::components::pl_edit::{
    PlEditLanguage, PlEditSession, advance_session, expand_at_cursor, format_source,
    render_pl_edit_help, update_session_after_input,
};
use gloo::timers::callback::Timeout;
use wasm_bindgen::JsCast;
use web_sys::HtmlTextAreaElement;
use yew::prelude::*;

/// PL/SW keywords for syntax highlighting.
const KEYWORDS: &[&str] = &[
    "DCL",
    "DECLARE",
    "PROC",
    "PROCEDURE",
    "OPTIONS",
    "NAKED",
    "RETURNS",
    "DO",
    "END",
    "IF",
    "THEN",
    "ELSE",
    "WHILE",
    "SELECT",
    "WHEN",
    "OTHERWISE",
    "RETURN",
    "CALL",
    "GOTO",
    "BIT",
    "BYTE",
    "WORD",
    "INT",
    "CHAR",
    "PTR",
    "INIT",
    "TO",
    "BY",
    "FIXED",
    "BASED",
    "DEFINED",
    "STATIC",
    "AUTO",
    "AUTOMATIC",
    "EXTERNAL",
    "REGISTER",
    "ENTRY",
    "LABEL",
    "BUILTIN",
    "ADDR",
    "LENGTH",
    "SUBSTR",
    "NULL",
    "BEGIN",
    "ON",
    "REVERT",
    "SIGNAL",
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
    #[prop_or_default]
    pub on_mass_compile: Option<Callback<MouseEvent>>,
}

#[function_component(SourceEditor)]
pub fn source_editor(props: &SourceEditorProps) -> Html {
    let source = props.source.clone();
    let highlighted = highlight_plsw(&source);
    let pl_edit_enabled = use_state(|| false);
    let help_open = use_state(|| false);
    let fullscreen = use_state(|| false);
    let edit_session = use_state(|| None::<PlEditSession>);
    let textarea_ref = use_node_ref();

    let oninput = {
        let on_change = props.on_change.clone();
        let edit_session = edit_session.clone();
        let source = source.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(target) = e.target()
                && let Some(ta) = target.dyn_ref::<HtmlTextAreaElement>()
            {
                let next_source = ta.value();
                if let Some(session) = (*edit_session).clone() {
                    let cursor = ta.selection_start().ok().flatten().unwrap_or(0) as usize;
                    edit_session.set(Some(update_session_after_input(
                        &session,
                        source.as_str(),
                        &next_source,
                        cursor,
                    )));
                }
                on_change.emit(next_source);
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
    let onkeydown = {
        let pl_edit_enabled = pl_edit_enabled.clone();
        let on_change = props.on_change.clone();
        let textarea_ref = textarea_ref.clone();
        let edit_session = edit_session.clone();
        Callback::from(move |e: KeyboardEvent| {
            if let Some(session) = (*edit_session).clone()
                && (e.key() == "Tab" || (e.key() == "Enter" && !e.ctrl_key()))
            {
                e.prevent_default();
                if let Some(next_session) = advance_session(&session, e.shift_key()) {
                    let field = next_session.fields[next_session.active];
                    edit_session.set(Some(next_session));
                    if let Some(textarea) = textarea_ref.cast::<HtmlTextAreaElement>() {
                        Timeout::new(0, move || {
                            let _ = textarea.focus();
                            let _ = textarea.set_selection_range(field, field);
                        })
                        .forget();
                    }
                } else {
                    edit_session.set(None);
                }
                return;
            }

            let expand_key = e.key() == "F4" || (e.ctrl_key() && e.key() == " ");
            if !expand_key || !*pl_edit_enabled {
                return;
            }
            if let Some(textarea) = textarea_ref.cast::<HtmlTextAreaElement>()
                && let Some(expansion) =
                    expand_at_cursor(&textarea, &textarea.value(), PlEditLanguage::Source)
            {
                e.prevent_default();
                edit_session.set(Some(PlEditSession {
                    fields: expansion.fields.clone(),
                    active: 0,
                }));
                textarea.set_value(&expansion.source);
                on_change.emit(expansion.source);
                Timeout::new(0, move || {
                    let _ = textarea.focus();
                    let _ = textarea.set_selection_range(expansion.cursor, expansion.cursor);
                })
                .forget();
            }
        })
    };

    let toggle_pl_edit = {
        let pl_edit_enabled = pl_edit_enabled.clone();
        Callback::from(move |_: MouseEvent| pl_edit_enabled.set(!*pl_edit_enabled))
    };
    let toggle_help = {
        let help_open = help_open.clone();
        Callback::from(move |_: MouseEvent| help_open.set(!*help_open))
    };
    let on_format = {
        let on_change = props.on_change.clone();
        let edit_session = edit_session.clone();
        let textarea_ref = textarea_ref.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(textarea) = textarea_ref.cast::<HtmlTextAreaElement>() {
                let formatted = format_source(&textarea.value());
                textarea.set_value(&formatted);
                edit_session.set(None);
                on_change.emit(formatted);
            }
        })
    };
    let toggle_fullscreen = {
        let fullscreen = fullscreen.clone();
        Callback::from(move |_: MouseEvent| fullscreen.set(!*fullscreen))
    };

    let root_class = classes!(
        "notebook-cell",
        (*fullscreen).then_some("editor-fullscreen")
    );
    let mode_label = if *pl_edit_enabled { "EDIT" } else { "PL/EDIT" };
    let fullscreen_label = if *fullscreen {
        "Collapse"
    } else {
        "Fullscreen"
    };

    html! {
        <div class={root_class} id="cell-source">
            <div class="cell-header">
                <span>{&props.title}</span>
                <span class="source-header-actions">
                    if let Some(name) = &props.example_name {
                        <span class="cell-header-example">{name}</span>
                    }
                    <button class={classes!("editor-action-btn", (*pl_edit_enabled).then_some("active"))}
                        onclick={toggle_pl_edit}
                        title="Toggle PL/EDIT hotkey expansion">
                        {mode_label}
                    </button>
                    <button class="editor-action-btn" onclick={toggle_help}
                        title="Show PL/EDIT expansion keys">
                        {"?"}
                    </button>
                    <button class="editor-action-btn" onclick={on_format}
                        title="Format PL/SW source indentation">
                        {"Format"}
                    </button>
                    <button class="editor-action-btn" onclick={toggle_fullscreen}
                        title="Expand or collapse editor">
                        {fullscreen_label}
                    </button>
                    if let Some(on_mass_compile) = props.on_mass_compile.clone() {
                        <button class="editor-action-btn" onclick={on_mass_compile}
                            title="Open Mass Compile">
                            {"Mass Compile"}
                        </button>
                    }
                </span>
            </div>
            if *help_open {
                {render_pl_edit_help(PlEditLanguage::Source)}
            }
            <div class="cell-content editor-container">
                <pre class="editor-highlight" ref={pre_ref}>
                    <code>{ Html::from_html_unchecked(AttrValue::from(highlighted)) }</code>
                </pre>
                <textarea
                    class="editor-textarea"
                    spellcheck="false"
                    autocomplete="off"
                    ref={textarea_ref}
                    value={source}
                    {oninput}
                    {onscroll}
                    {onkeydown}
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
