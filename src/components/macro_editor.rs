//! Macro file editor component for .msw files.
//!
//! Each .msw file gets its own collapsible notebook cell with a filename header,
//! syntax highlighting for macro-specific keywords (MACRODEF, GEN, REQUIRED, etc.),
//! and add/remove controls.

use wasm_bindgen::JsCast;
use web_sys::HtmlTextAreaElement;
use yew::prelude::*;

/// A single macro file with a name and source content.
#[derive(Clone, PartialEq)]
pub struct MacroFile {
    pub name: String,
    pub source: String,
    pub collapsed: bool,
}

impl MacroFile {
    pub fn new(name: String, source: String) -> Self {
        Self {
            name,
            source,
            collapsed: false,
        }
    }
}

/// Macro-specific keywords for syntax highlighting.
const MACRO_KEYWORDS: &[&str] = &[
    "MACRODEF", "GEN", "REQUIRED", "OPTIONAL", "END", "IF", "THEN", "ELSE", "DO", "WHILE",
];

/// PL/SW type keywords that may appear in macro bodies.
const TYPE_KEYWORDS: &[&str] = &[
    "DCL", "PROC", "BYTE", "WORD", "INT", "CHAR", "PTR", "BIT", "FIXED", "BASED", "DEFINED",
    "STATIC", "AUTO", "ENTRY", "LABEL", "BUILTIN", "ADDR", "LENGTH", "SUBSTR", "NULL", "RETURN",
    "CALL", "GOTO", "BEGIN", "ON", "REVERT", "SIGNAL", "INIT", "TO", "BY",
];

/// Directive keyword.
const INCLUDE_DIRECTIVE: &str = "%INCLUDE";

#[derive(Properties, PartialEq)]
pub struct MacroEditorProps {
    pub files: Vec<MacroFile>,
    pub on_change: Callback<(usize, String)>,
    pub on_add: Callback<()>,
    pub on_remove: Callback<usize>,
    pub on_rename: Callback<(usize, String)>,
    pub on_toggle_collapse: Callback<usize>,
    pub on_upload: Callback<(String, String)>,
}

#[function_component(MacroEditor)]
pub fn macro_editor(props: &MacroEditorProps) -> Html {
    let file_upload_ref = use_node_ref();

    let on_upload_click = {
        let file_upload_ref = file_upload_ref.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(input) = file_upload_ref.cast::<web_sys::HtmlInputElement>() {
                input.click();
            }
        })
    };

    let on_file_selected = {
        let on_upload = props.on_upload.clone();
        let file_reader = use_state(|| None::<gloo::file::callbacks::FileReader>);
        Callback::from(move |e: Event| {
            if let Some(target) = e.target()
                && let Some(input) = target.dyn_ref::<web_sys::HtmlInputElement>()
                && let Some(files) = input.files()
                && let Some(file) = files.get(0)
            {
                let name = file.name();
                let file = gloo::file::File::from(file);
                let on_upload = on_upload.clone();
                let reader = gloo::file::callbacks::read_as_text(&file, move |result| {
                    if let Ok(text) = result {
                        on_upload.emit((name, text));
                    }
                });
                file_reader.set(Some(reader));
                // Reset input so the same file can be re-uploaded
                input.set_value("");
            }
        })
    };

    html! {
        <div class="notebook-cell" id="cell-macros">
            <div class="cell-header">
                <span>{"Macro Files (.msw)"}</span>
                <div class="macro-header-actions">
                    <button class="macro-action-btn" onclick={on_upload_click}
                        title="Upload .msw file">
                        {"\u{1F4C2}"}
                    </button>
                    <button class="macro-action-btn" onclick={
                        let cb = props.on_add.clone();
                        Callback::from(move |_: MouseEvent| cb.emit(()))
                    } title="Add new macro file">
                        {"+"}
                    </button>
                </div>
            </div>
            <div class="cell-content">
                <input type="file" ref={file_upload_ref}
                    class="file-upload-input" accept=".msw,.txt"
                    onchange={on_file_selected} />

                if props.files.is_empty() {
                    <div class="macro-empty-state">
                        <span>{"No macro files"}</span>
                        <span class="macro-empty-hint">
                            {"Click + to add or upload a .msw file"}
                        </span>
                    </div>
                } else {
                    { for props.files.iter().enumerate().map(|(idx, file)| {
                        render_macro_file(
                            idx,
                            file,
                            &props.on_change,
                            &props.on_remove,
                            &props.on_rename,
                            &props.on_toggle_collapse,
                        )
                    })}
                }
            </div>
        </div>
    }
}

fn render_macro_file(
    idx: usize,
    file: &MacroFile,
    on_change: &Callback<(usize, String)>,
    on_remove: &Callback<usize>,
    on_rename: &Callback<(usize, String)>,
    on_toggle_collapse: &Callback<usize>,
) -> Html {
    let highlighted = highlight_msw(&file.source);
    let collapse_icon = if file.collapsed {
        "\u{25B6}"
    } else {
        "\u{25BC}"
    };

    let oninput = {
        let on_change = on_change.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(target) = e.target()
                && let Some(ta) = target.dyn_ref::<HtmlTextAreaElement>()
            {
                on_change.emit((idx, ta.value()));
            }
        })
    };

    let on_name_input = {
        let on_rename = on_rename.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(target) = e.target()
                && let Some(input) = target.dyn_ref::<web_sys::HtmlInputElement>()
            {
                on_rename.emit((idx, input.value()));
            }
        })
    };

    let on_toggle = {
        let on_toggle_collapse = on_toggle_collapse.clone();
        Callback::from(move |_: MouseEvent| on_toggle_collapse.emit(idx))
    };

    let on_remove_click = {
        let on_remove = on_remove.clone();
        Callback::from(move |_: MouseEvent| on_remove.emit(idx))
    };

    html! {
        <div class="macro-file-cell">
            <div class="macro-file-header">
                <button class="macro-collapse-btn" onclick={on_toggle}>
                    {collapse_icon}
                </button>
                <input class="macro-name-input" type="text"
                    value={file.name.clone()}
                    oninput={on_name_input}
                    spellcheck="false"
                    placeholder="filename.msw" />
                <button class="macro-remove-btn" onclick={on_remove_click}
                    title="Remove this macro file">
                    {"\u{00D7}"}
                </button>
            </div>
            if !file.collapsed {
                <div class="macro-file-editor editor-container">
                    <pre class="editor-highlight">
                        <code>{ Html::from_html_unchecked(AttrValue::from(highlighted)) }</code>
                    </pre>
                    <textarea
                        class="editor-textarea"
                        spellcheck="false"
                        autocomplete="off"
                        value={file.source.clone()}
                        {oninput}
                    />
                </div>
            }
        </div>
    }
}

/// Syntax highlighter for .msw macro files.
fn highlight_msw(source: &str) -> String {
    let mut out = String::with_capacity(source.len() * 2);
    let chars: Vec<char> = source.chars().collect();
    let len = chars.len();
    let mut i = 0;

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

        // String literals: '...' (used in GEN blocks)
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

        // String literals: "..." (GEN assembly strings)
        if chars[i] == '"' {
            out.push_str("<span class=\"hl-gen-string\">");
            out.push_str(&escape_char(chars[i]));
            i += 1;
            while i < len && chars[i] != '"' {
                // Highlight {PARAM} template substitutions inside strings
                if chars[i] == '{' {
                    out.push_str("<span class=\"hl-gen-param\">");
                    out.push_str(&escape_char(chars[i]));
                    i += 1;
                    while i < len && chars[i] != '}' && chars[i] != '"' {
                        out.push_str(&escape_char(chars[i]));
                        i += 1;
                    }
                    if i < len && chars[i] == '}' {
                        out.push_str(&escape_char(chars[i]));
                        i += 1;
                    }
                    out.push_str("</span>");
                } else {
                    out.push_str(&escape_char(chars[i]));
                    i += 1;
                }
            }
            if i < len {
                out.push_str(&escape_char(chars[i]));
                i += 1;
            }
            out.push_str("</span>");
            continue;
        }

        // %INCLUDE directive
        if chars[i] == '%' && i + 1 < len && chars[i + 1].is_ascii_alphabetic() {
            let start = i;
            i += 1;
            while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let upper = word.to_ascii_uppercase();
            if upper == INCLUDE_DIRECTIVE {
                out.push_str("<span class=\"hl-directive\">");
                out.push_str(&escape(&word));
                out.push_str("</span>");
            } else {
                out.push_str(&escape(&word));
            }
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

        // Numbers
        if chars[i].is_ascii_digit() {
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

            if MACRO_KEYWORDS.contains(&upper.as_str()) {
                out.push_str("<span class=\"hl-macro-keyword\">");
                out.push_str(&escape(&word));
                out.push_str("</span>");
            } else if upper == "ASM" || upper == "ASM_EXPR" {
                out.push_str("<span class=\"hl-asm\">");
                out.push_str(&escape(&word));
                out.push_str("</span>");
            } else if TYPE_KEYWORDS.contains(&upper.as_str()) {
                out.push_str("<span class=\"hl-keyword\">");
                out.push_str(&escape(&word));
                out.push_str("</span>");
            } else if upper == "EXPR" || upper == "LVALUE" {
                out.push_str("<span class=\"hl-type-hint\">");
                out.push_str(&escape(&word));
                out.push_str("</span>");
            } else {
                out.push_str(&escape(&word));
            }
            continue;
        }

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
