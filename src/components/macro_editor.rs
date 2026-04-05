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
                    <div class="macro-file-strip">
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
                    </div>
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
                <textarea
                    class="macro-file-textarea"
                    spellcheck="false"
                    autocomplete="off"
                    value={file.source.clone()}
                    {oninput}
                />
            }
        </div>
    }
}
