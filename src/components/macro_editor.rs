//! Macro file editor component for .msw files.
//!
//! Each .msw file gets its own collapsible notebook cell with a filename header,
//! PL/EDIT controls, formatting, and add/remove controls.

use crate::components::pl_edit::{
    PlEditLanguage, PlEditSession, advance_session, expand_at_cursor, format_source,
    render_pl_edit_help, update_session_after_input,
};
use gloo::timers::callback::Timeout;
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
                <span class="macro-header-actions">
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
                    <a href="https://github.com/sw-embed/web-sw-cor24-plsw"
                       class="header-action-repo-link" target="_blank" rel="noopener">
                        {"sw-embed/web-sw-cor24-plsw"}
                    </a>
                </span>
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
                    <div class="macro-file-strip">
                        { for props.files.iter().enumerate().map(|(idx, file)| {
                            html! {
                                <MacroFileEditor
                                    idx={idx}
                                    file={file.clone()}
                                    on_change={props.on_change.clone()}
                                    on_remove={props.on_remove.clone()}
                                    on_rename={props.on_rename.clone()}
                                    on_toggle_collapse={props.on_toggle_collapse.clone()}
                                />
                            }
                        })}
                    </div>
                }
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct MacroFileEditorProps {
    idx: usize,
    file: MacroFile,
    on_change: Callback<(usize, String)>,
    on_remove: Callback<usize>,
    on_rename: Callback<(usize, String)>,
    on_toggle_collapse: Callback<usize>,
}

#[function_component(MacroFileEditor)]
fn macro_file_editor(props: &MacroFileEditorProps) -> Html {
    let pl_edit_enabled = use_state(|| false);
    let help_open = use_state(|| false);
    let fullscreen = use_state(|| false);
    let edit_session = use_state(|| None::<PlEditSession>);
    let textarea_ref = use_node_ref();

    let idx = props.idx;
    let collapse_icon = if props.file.collapsed {
        "\u{25B6}"
    } else {
        "\u{25BC}"
    };

    let oninput = {
        let on_change = props.on_change.clone();
        let edit_session = edit_session.clone();
        let old_source = props.file.source.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(target) = e.target()
                && let Some(ta) = target.dyn_ref::<HtmlTextAreaElement>()
            {
                let next_source = ta.value();
                if let Some(session) = (*edit_session).clone() {
                    let cursor = ta.selection_start().ok().flatten().unwrap_or(0) as usize;
                    edit_session.set(Some(update_session_after_input(
                        &session,
                        &old_source,
                        &next_source,
                        cursor,
                    )));
                }
                on_change.emit((idx, next_source));
            }
        })
    };

    let on_name_input = {
        let on_rename = props.on_rename.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(target) = e.target()
                && let Some(input) = target.dyn_ref::<web_sys::HtmlInputElement>()
            {
                on_rename.emit((idx, input.value()));
            }
        })
    };

    let on_toggle = {
        let on_toggle_collapse = props.on_toggle_collapse.clone();
        Callback::from(move |_: MouseEvent| on_toggle_collapse.emit(idx))
    };

    let on_remove_click = {
        let on_remove = props.on_remove.clone();
        Callback::from(move |_: MouseEvent| on_remove.emit(idx))
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
                    expand_at_cursor(&textarea, &textarea.value(), PlEditLanguage::Macro)
            {
                e.prevent_default();
                edit_session.set(Some(PlEditSession {
                    fields: expansion.fields.clone(),
                    active: 0,
                }));
                textarea.set_value(&expansion.source);
                on_change.emit((idx, expansion.source));
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
        let source = props.file.source.clone();
        let textarea_ref = textarea_ref.clone();
        Callback::from(move |_: MouseEvent| {
            let textarea = textarea_ref.cast::<HtmlTextAreaElement>();
            let current = textarea
                .as_ref()
                .map(HtmlTextAreaElement::value)
                .unwrap_or_else(|| source.clone());
            let formatted = format_source(&current);
            if let Some(textarea) = textarea {
                textarea.set_value(&formatted);
            }
            edit_session.set(None);
            on_change.emit((idx, formatted));
        })
    };
    let toggle_fullscreen = {
        let fullscreen = fullscreen.clone();
        Callback::from(move |_: MouseEvent| fullscreen.set(!*fullscreen))
    };
    let mode_label = if *pl_edit_enabled { "EDIT" } else { "PL/EDIT" };
    let fullscreen_label = if *fullscreen {
        "Collapse"
    } else {
        "Fullscreen"
    };

    html! {
        <div class={classes!("macro-file-cell", (*fullscreen).then_some("editor-fullscreen"))}>
            <div class="macro-file-header">
                <button class="macro-collapse-btn" onclick={on_toggle}>
                    {collapse_icon}
                </button>
                <input class="macro-name-input" type="text"
                    value={props.file.name.clone()}
                    oninput={on_name_input}
                    spellcheck="false"
                    placeholder="filename.msw" />
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
                    title="Format PL/SW or .msw indentation">
                    {"Format"}
                </button>
                <button class="editor-action-btn" onclick={toggle_fullscreen}
                    title="Expand or collapse editor">
                    {fullscreen_label}
                </button>
                <button class="macro-remove-btn" onclick={on_remove_click}
                    title="Remove this macro file">
                    {"\u{00D7}"}
                </button>
            </div>
            if *help_open {
                {render_pl_edit_help(PlEditLanguage::Macro)}
            }
            if !props.file.collapsed {
                <textarea
                    class="macro-file-textarea"
                    spellcheck="false"
                    autocomplete="off"
                    ref={textarea_ref}
                    value={props.file.source.clone()}
                    {oninput}
                    {onkeydown}
                />
            }
        </div>
    }
}
