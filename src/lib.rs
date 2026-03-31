pub mod components;
pub mod demos;
pub mod pipeline;
pub mod preprocessor;

use components::{
    MacroEditor, MacroExpansionView, MacroFile, SourceEditor, WizardSidebar, WizardStep,
};
use demos::DEMOS;
use gloo::file::File;
use gloo::file::callbacks::FileReader;
use pipeline::CompileResult;
use preprocessor::PreprocessResult;
use wasm_bindgen::JsCast;
use web_sys::{HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;

use cor24_emulator::{Assembler, EmulatorCore, StopReason};

/// Smoke test: assemble a trivial COR24 program, run it, verify register value.
fn emulator_smoke_test() -> Result<String, String> {
    let mut asm = Assembler::new();
    let result = asm.assemble("  lc r0, 42\nhalt:\n  bra halt\n");
    if !result.errors.is_empty() {
        return Err(format!("Assembly errors: {:?}", result.errors));
    }

    let mut emu = EmulatorCore::new();
    emu.load_program(0, &result.bytes);
    emu.set_pc(0);
    emu.resume();
    let batch = emu.run_batch(100);

    if !matches!(batch.reason, StopReason::Halted) {
        return Err(format!("Expected Halted, got {:?}", batch.reason));
    }

    let r0 = emu.get_reg(0);
    if r0 == 42 {
        Ok(format!(
            "WASM smoke test PASSED: r0={r0}, {} instructions, halted OK",
            batch.instructions_run
        ))
    } else {
        Err(format!("Expected r0=42, got r0={r0}"))
    }
}

/// Format a large number with K/M suffix for display.
fn format_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

#[function_component(App)]
pub fn app() -> Html {
    // Run emulator smoke test on mount
    use_effect_with((), |_| match emulator_smoke_test() {
        Ok(msg) => web_sys::console::log_1(&msg.into()),
        Err(msg) => web_sys::console::error_1(&msg.into()),
    });

    // Source editor state
    let source = use_state(|| DEMOS[0].source.to_string());
    let selected_demo = use_state(|| Some(0usize));
    let current_step = use_state(|| WizardStep::Source);

    // Macro files state
    let macro_files = use_state(|| {
        DEMOS[0]
            .macros
            .iter()
            .map(|m| MacroFile::new(m.name.to_string(), m.source.to_string()))
            .collect::<Vec<_>>()
    });

    // Preprocessor result state
    let preprocess_result = use_state(|| None::<PreprocessResult>);

    // Compilation result state
    let compile_result = use_state(|| None::<CompileResult>);
    let compiling = use_state(|| false);

    // File reader state (must keep alive during async read)
    let _file_reader = use_state(|| None::<FileReader>);

    // Demo selection handler
    let on_demo_select = {
        let source = source.clone();
        let selected_demo = selected_demo.clone();
        let current_step = current_step.clone();
        let macro_files = macro_files.clone();
        let compile_result = compile_result.clone();
        let preprocess_result = preprocess_result.clone();
        Callback::from(move |e: Event| {
            if let Some(target) = e.target()
                && let Some(select) = target.dyn_ref::<HtmlSelectElement>()
            {
                let idx: usize = select.value().parse().unwrap_or(0);
                source.set(DEMOS[idx].source.to_string());
                macro_files.set(
                    DEMOS[idx]
                        .macros
                        .iter()
                        .map(|m| MacroFile::new(m.name.to_string(), m.source.to_string()))
                        .collect(),
                );
                selected_demo.set(Some(idx));
                current_step.set(WizardStep::Source);
                compile_result.set(None);
                preprocess_result.set(None);
                // Scroll notebook to top
                if let Some(window) = web_sys::window()
                    && let Some(document) = window.document()
                    && let Some(container) = document.get_element_by_id("notebook-scroll")
                {
                    container.set_scroll_top(0);
                }
            }
        })
    };

    // Source change handler
    let on_source_change = {
        let source = source.clone();
        let selected_demo = selected_demo.clone();
        Callback::from(move |new_source: String| {
            source.set(new_source);
            selected_demo.set(None);
        })
    };

    // File upload handler
    let on_file_upload = {
        let source = source.clone();
        let selected_demo = selected_demo.clone();
        let current_step = current_step.clone();
        let file_reader = _file_reader.clone();
        Callback::from(move |e: Event| {
            if let Some(target) = e.target()
                && let Some(input) = target.dyn_ref::<HtmlInputElement>()
                && let Some(files) = input.files()
                && let Some(file) = files.get(0)
            {
                let file = File::from(file);
                let source = source.clone();
                let selected_demo = selected_demo.clone();
                let current_step = current_step.clone();
                let reader = gloo::file::callbacks::read_as_text(&file, move |result| {
                    if let Ok(text) = result {
                        source.set(text);
                        selected_demo.set(None);
                        current_step.set(WizardStep::Source);
                    }
                });
                file_reader.set(Some(reader));
            }
        })
    };

    // Wizard step click
    let on_step_click = {
        let current_step = current_step.clone();
        Callback::from(move |step: WizardStep| {
            if step <= *current_step {
                let cell_id = step.cell_id().to_string();
                if let Some(window) = web_sys::window()
                    && let Some(document) = window.document()
                    && let Some(element) = document.get_element_by_id(&cell_id)
                {
                    element.scroll_into_view();
                }
            }
        })
    };

    // Wizard advance
    let on_advance = {
        let current_step = current_step.clone();
        let source = source.clone();
        let macro_files = macro_files.clone();
        let compile_result = compile_result.clone();
        let compiling = compiling.clone();
        let preprocess_result = preprocess_result.clone();
        Callback::from(move |()| {
            let current = *current_step;
            if let Some(next) = current.next() {
                // Trigger compilation when advancing to Preprocess (skips to Compile)
                if next == WizardStep::Preprocess && !*compiling {
                    // Run preprocessor first (synchronous, fast)
                    let macro_pairs: Vec<(String, String)> = macro_files
                        .iter()
                        .map(|m| (m.name.clone(), m.source.clone()))
                        .collect();
                    let pp_result = preprocessor::preprocess(&source, &macro_pairs);
                    preprocess_result.set(Some(pp_result));

                    compiling.set(true);
                    let src = (*source).clone();
                    let macros: Vec<(String, String)> = macro_files
                        .iter()
                        .map(|m| (m.name.clone(), m.source.clone()))
                        .collect();

                    let compile_result = compile_result.clone();
                    let inner_step = current_step.clone();
                    let compiling = compiling.clone();

                    // Run compilation asynchronously via timeout to allow UI to update
                    gloo::timers::callback::Timeout::new(50, move || {
                        let result = pipeline::run_compiler(&src, &macros);
                        compile_result.set(Some(result));
                        compiling.set(false);
                        // Advance to Compile step (skip Preprocess since we show both)
                        inner_step.set(WizardStep::Compile);

                        // Scroll to output
                        gloo::timers::callback::Timeout::new(100, || {
                            if let Some(window) = web_sys::window()
                                && let Some(document) = window.document()
                                && let Some(element) = document.get_element_by_id("cell-compile")
                            {
                                element.scroll_into_view();
                            }
                        })
                        .forget();
                    })
                    .forget();

                    // Show preprocess cell immediately
                    current_step.set(WizardStep::Preprocess);
                    return;
                }

                current_step.set(next);
                let scroll_to = next.cell_id().to_string();
                gloo::timers::callback::Timeout::new(100, move || {
                    if let Some(window) = web_sys::window()
                        && let Some(document) = window.document()
                        && let Some(element) = document.get_element_by_id(&scroll_to)
                    {
                        element.scroll_into_view();
                    }
                })
                .forget();
            }
        })
    };

    // Macro editor callbacks
    let on_macro_change = {
        let macro_files = macro_files.clone();
        Callback::from(move |(idx, new_source): (usize, String)| {
            let mut files = (*macro_files).clone();
            if let Some(f) = files.get_mut(idx) {
                f.source = new_source;
            }
            macro_files.set(files);
        })
    };

    let on_macro_add = {
        let macro_files = macro_files.clone();
        Callback::from(move |()| {
            let mut files = (*macro_files).clone();
            let n = files.len() + 1;
            files.push(MacroFile::new(format!("MACRO{n}.msw"), String::new()));
            macro_files.set(files);
        })
    };

    let on_macro_remove = {
        let macro_files = macro_files.clone();
        Callback::from(move |idx: usize| {
            let mut files = (*macro_files).clone();
            if idx < files.len() {
                files.remove(idx);
            }
            macro_files.set(files);
        })
    };

    let on_macro_rename = {
        let macro_files = macro_files.clone();
        Callback::from(move |(idx, new_name): (usize, String)| {
            let mut files = (*macro_files).clone();
            if let Some(f) = files.get_mut(idx) {
                f.name = new_name;
            }
            macro_files.set(files);
        })
    };

    let on_macro_toggle = {
        let macro_files = macro_files.clone();
        Callback::from(move |idx: usize| {
            let mut files = (*macro_files).clone();
            if let Some(f) = files.get_mut(idx) {
                f.collapsed = !f.collapsed;
            }
            macro_files.set(files);
        })
    };

    let on_macro_upload = {
        let macro_files = macro_files.clone();
        Callback::from(move |(name, source): (String, String)| {
            let mut files = (*macro_files).clone();
            files.push(MacroFile::new(name, source));
            macro_files.set(files);
        })
    };

    let example_name = selected_demo.as_ref().map(|&idx| DEMOS[idx].name);

    html! {
        <>
            // GitHub corner
            <a href="https://github.com/softwarewrighter/web-sw-cor24-plsw" class="github-corner"
               aria-label="View source on GitHub" target="_blank">
                <svg width="80" height="80" viewBox="0 0 250 250" aria-hidden="true">
                    <path d="M0,0 L115,115 L130,115 L142,142 L250,250 L250,0 Z" />
                    <path d="M128.3,109.0 C113.8,99.7 119.0,89.6 119.0,89.6 C122.0,82.7 120.5,78.6 \
                        120.5,78.6 C119.2,72.0 123.4,76.3 123.4,76.3 C127.3,80.9 125.5,87.3 125.5,87.3 \
                        C122.9,97.6 130.6,101.9 134.4,103.2" fill="currentColor"
                        style="transform-origin:130px 106px;" class="octo-arm" />
                    <path d="M115.0,115.0 C114.9,115.1 118.7,116.5 119.8,115.4 L133.7,101.6 C136.9,99.2 \
                        139.9,98.4 142.2,98.6 C133.8,88.0 127.5,74.4 143.8,58.0 C148.5,53.4 154.0,51.2 \
                        159.7,51.0 C160.3,49.4 163.2,43.6 171.4,40.1 C171.4,40.1 176.1,42.5 178.8,56.2 \
                        C183.1,58.6 187.2,61.8 190.9,65.4 C194.5,69.0 197.7,73.2 200.1,77.6 C213.8,80.2 \
                        216.3,84.9 216.3,84.9 C212.7,93.1 206.9,96.0 205.4,96.6 C205.1,102.4 203.0,107.8 \
                        198.3,112.5 C181.9,128.9 168.3,122.5 157.7,114.1 C157.9,116.9 156.7,120.9 \
                        152.7,124.9 L141.0,136.5 C139.8,137.7 141.6,141.9 141.8,141.8 Z"
                        fill="currentColor" />
                </svg>
            </a>
            // Header
            <header>
                <h1>{"PL/SW"}</h1>
                <span>{"COR24 Dev"}</span>
            </header>
            // Main 3-column layout
            <div id="app" class="plsw-wizard-layout">
                // Column 1: Sidebar
                <div class="wizard-sidebar">
                    <div class="sidebar-section">
                        <span class="sidebar-section-label">{"Demo"}</span>
                        <select class="sidebar-select"
                            onchange={on_demo_select}
                            value={selected_demo.map_or(String::new(), |i| i.to_string())}>
                            { for DEMOS.iter().enumerate().map(|(i, demo)| {
                                html! {
                                    <option value={i.to_string()}
                                        selected={*selected_demo == Some(i)}>
                                        {demo.name}
                                    </option>
                                }
                            })}
                        </select>
                    </div>

                    <div class="sidebar-section">
                        <span class="sidebar-section-label">{"File"}</span>
                        <input type="file" id="file-upload" class="file-upload-input"
                            accept=".plsw,.msw,.txt" onchange={on_file_upload} />
                        <label for="file-upload" class="file-upload-label">{"Upload .plsw"}</label>
                    </div>

                    <div class="sidebar-spacer"></div>

                    <a href="https://github.com/softwarewrighter/web-sw-cor24-plsw"
                       target="_blank" rel="noopener" class="sidebar-link">
                        {"GitHub"}<span class="ext-icon">{" \u{2197}"}</span>
                    </a>
                </div>

                // Column 2: Wizard steps
                <WizardSidebar
                    current_step={*current_step}
                    {on_step_click}
                    {on_advance}
                    has_source={!source.is_empty()}
                />

                // Column 3: Notebook cells
                <div class="notebook-cells" id="notebook-scroll">
                    // Cell: Source editor (always visible)
                    <SourceEditor
                        source={(*source).clone()}
                        on_change={on_source_change}
                        title="PL/SW Source"
                        example_name={example_name.map(AttrValue::from)}
                    />

                    // Macro editor cell
                    if *current_step >= WizardStep::Macros {
                        <MacroEditor
                            files={(*macro_files).clone()}
                            on_change={on_macro_change.clone()}
                            on_add={on_macro_add.clone()}
                            on_remove={on_macro_remove.clone()}
                            on_rename={on_macro_rename.clone()}
                            on_toggle_collapse={on_macro_toggle.clone()}
                            on_upload={on_macro_upload.clone()}
                        />
                    }

                    if *current_step >= WizardStep::Preprocess {
                        // Macro expansion view
                        <div class="notebook-cell" id="cell-preprocess">
                            <div class="cell-header">
                                <span>{"Preprocessed Source"}</span>
                                if let Some(ref pp) = *preprocess_result {
                                    <span class="cell-header-stats">
                                        {format!("{} expansion{}", pp.expansions.len(),
                                            if pp.expansions.len() == 1 { "" } else { "s" })}
                                    </span>
                                }
                            </div>
                            <div class="cell-content">
                                if let Some(ref pp) = *preprocess_result {
                                    <MacroExpansionView result={pp.clone()} />
                                } else {
                                    <div class="notebook-placeholder">
                                        <span>{"Click Preprocess to expand macros"}</span>
                                    </div>
                                }
                            </div>
                        </div>

                        // Compiler boot output
                        <div class="notebook-cell" id="cell-boot">
                            <div class="cell-header">
                                <span>{"Compiler Boot (Self-Tests)"}</span>
                                if let Some(ref result) = *compile_result {
                                    <span class="cell-header-stats">
                                        {format!("{} instructions", format_count(result.instructions))}
                                    </span>
                                }
                            </div>
                            <div class="cell-content">
                                if *compiling {
                                    <div class="compile-status">
                                        <span class="compile-spinner">{"\u{23F3}"}</span>
                                        <span>{"Running PL/SW compiler on COR24 emulator..."}</span>
                                    </div>
                                } else if let Some(ref result) = *compile_result {
                                    <pre class="pipeline-output">{&result.boot_output}</pre>
                                } else {
                                    <div class="notebook-placeholder">
                                        <span>{"Waiting for compiler boot..."}</span>
                                    </div>
                                }
                            </div>
                        </div>
                    }

                    if *current_step >= WizardStep::Compile {
                        <div class="notebook-cell" id="cell-compile">
                            <div class="cell-header">
                                <span>{"Compiler Output (Lexer Tokens)"}</span>
                                if let Some(ref result) = *compile_result {
                                    if result.error.is_some() {
                                        <span class="cell-status-error">{"\u{2717} Error"}</span>
                                    } else {
                                        <span class="cell-status-ok">{"\u{2713} OK"}</span>
                                    }
                                }
                            </div>
                            <div class="cell-content">
                                if let Some(ref result) = *compile_result {
                                    if let Some(ref err) = result.error {
                                        <div class="compile-error">
                                            <span class="error-label">{"Error: "}</span>
                                            <span>{err}</span>
                                        </div>
                                    }
                                    if !result.compiler_output.is_empty() {
                                        <pre class="pipeline-output">{&result.compiler_output}</pre>
                                    } else if result.error.is_none() {
                                        <div class="compile-status">
                                            <span>{"No token output (source may be empty)"}</span>
                                        </div>
                                    }
                                    <div class="compile-note">
                                        <em>{"Note: The PL/SW compiler currently implements the lexer stage. \
                                              Full compilation (parser \u{2192} codegen \u{2192} assembly output) \
                                              is under development in the sw-cor24-plsw project."}</em>
                                    </div>
                                } else {
                                    <div class="notebook-placeholder">
                                        <span>{"Waiting for compilation..."}</span>
                                    </div>
                                }
                            </div>
                        </div>
                    }

                    if *current_step >= WizardStep::Assemble {
                        <div class="notebook-cell" id="cell-assemble">
                            <div class="cell-header">
                                <span>{"Assembly Listing"}</span>
                            </div>
                            <div class="cell-content">
                                <div class="notebook-placeholder">
                                    <span>{"Assembler -- coming soon"}</span>
                                </div>
                            </div>
                        </div>
                    }

                    if *current_step >= WizardStep::Run {
                        <div class="notebook-cell" id="cell-run">
                            <div class="cell-header">
                                <span>{"Execution / Debugger"}</span>
                            </div>
                            <div class="cell-content">
                                <div class="notebook-placeholder">
                                    <span>{"Debugger -- coming soon"}</span>
                                </div>
                            </div>
                        </div>
                    }

                    // Pipeline note
                    <div class="pipeline-note">
                        <em>{"Pipeline: .plsw + .msw \u{2192} Preprocess \u{2192} Compile (on COR24) \u{2192} .s \u{2192} Assemble \u{2192} Run"}</em>
                    </div>
                </div>
            </div>
            // Footer
            <footer>
                <span>{"MIT License"}</span>
                <span class="footer-sep">{"\u{00b7}"}</span>
                <span>{"\u{00a9} 2026 Michael A Wright"}</span>
                <span class="footer-sep">{"\u{00b7}"}</span>
                <a href="https://makerlisp.com" target="_blank">{"COR24-TB"}</a>
                <span class="footer-sep">{"\u{00b7}"}</span>
                <span>{env!("BUILD_SHA")}</span>
                <span class="footer-sep">{"\u{00b7}"}</span>
                <span>{env!("BUILD_HOST")}</span>
                <span class="footer-sep">{"\u{00b7}"}</span>
                <span>{env!("BUILD_TIMESTAMP")}</span>
            </footer>
        </>
    }
}
