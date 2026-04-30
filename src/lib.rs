pub mod components;
pub mod demos;
pub mod pipeline;
pub mod preprocessor;

use components::{
    MacroEditor, MacroFile, MassCompileDialog, SourceEditor, WizardSidebar, WizardStep,
};
use demos::DEMOS;
use gloo::file::File;
use gloo::file::callbacks::FileReader;
use pipeline::{CompileResult, EmulatorDump, RunResult};
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

/// Add line numbers to source text.
fn number_lines(source: &str) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let width = lines.len().to_string().len().max(3);
    lines
        .iter()
        .enumerate()
        .map(|(i, line)| format!("{:>width$}  {line}", i + 1))
        .collect::<Vec<_>>()
        .join("\n")
}

/// COR24 register names: r0, r1, r2, fp, sp, z/c, iv, ir
const REG_NAMES: [&str; 8] = ["r0", "r1", "r2", "fp", "sp", "z/c", "iv", "ir"];

/// Format an emulator dump for display.
fn format_dump(dump: &EmulatorDump) -> String {
    let mut out = String::new();

    // Registers
    out.push_str("── Registers ──\n");
    out.push_str(&format!(
        "  PC  = 0x{:06X}    C = {}\n",
        dump.pc, dump.condition_flag as u8
    ));
    for (i, &val) in dump.registers.iter().enumerate() {
        out.push_str(&format!("  {:<3} = 0x{:06X}", REG_NAMES[i], val));
        if i == 2 || i == 4 || i == 7 {
            out.push('\n');
        } else {
            out.push_str("    ");
        }
    }

    // I/O
    out.push_str(&format!("\n── I/O ──\n  LED = 0x{:02X}\n", dump.led));

    // Memory regions
    if !dump.memory_regions.is_empty() {
        out.push_str("\n── Memory (non-zero) ──\n");
        for (addr, bytes) in &dump.memory_regions {
            // Display in 16-byte rows with ASCII sidebar
            for chunk_start in (0..bytes.len()).step_by(16) {
                let chunk_end = (chunk_start + 16).min(bytes.len());
                let chunk = &bytes[chunk_start..chunk_end];
                out.push_str(&format!("  0x{:06X}:", *addr + chunk_start as u32));
                for b in chunk {
                    out.push_str(&format!(" {:02X}", b));
                }
                // Pad if short row
                for _ in chunk.len()..16 {
                    out.push_str("   ");
                }
                out.push_str("  ");
                for b in chunk {
                    if b.is_ascii_graphic() || *b == b' ' {
                        out.push(*b as char);
                    } else {
                        out.push('.');
                    }
                }
                out.push('\n');
            }
        }
    }

    out
}

#[function_component(App)]
pub fn app() -> Html {
    // Run emulator smoke test on mount
    use_effect_with((), |_| match emulator_smoke_test() {
        Ok(msg) => web_sys::console::log_1(&msg.into()),
        Err(msg) => web_sys::console::error_1(&msg.into()),
    });

    // Source editor state
    // Default to the minimal no-op demo.
    let default_demo = 0usize;
    let source = use_state(|| DEMOS[default_demo].source.to_string());
    let selected_demo = use_state(|| Some(default_demo));
    let editing_demo = use_state(|| default_demo);
    let edited_sources = use_state(|| vec![None::<String>; DEMOS.len()]);
    let current_step = use_state(|| WizardStep::Source);
    let mass_compile_open = use_state(|| false);

    // Macro files state
    let macro_files = use_state(|| {
        DEMOS[default_demo]
            .macros
            .iter()
            .map(|m| MacroFile::new(m.name.to_string(), m.source.to_string()))
            .collect::<Vec<_>>()
    });
    let edited_macros = use_state(|| vec![None::<Vec<MacroFile>>; DEMOS.len()]);

    // Preprocessor result state
    let preprocess_result = use_state(|| None::<PreprocessResult>);

    // Compilation result state
    let compile_result = use_state(|| None::<CompileResult>);
    let run_result = use_state(|| None::<RunResult>);
    let compiling = use_state(|| false);

    // File reader state (must keep alive during async read)
    let _file_reader = use_state(|| None::<FileReader>);

    // Demo selection handler
    let on_demo_select = {
        let source = source.clone();
        let selected_demo = selected_demo.clone();
        let editing_demo = editing_demo.clone();
        let edited_sources = edited_sources.clone();
        let current_step = current_step.clone();
        let macro_files = macro_files.clone();
        let edited_macros = edited_macros.clone();
        let compile_result = compile_result.clone();
        let run_result = run_result.clone();
        let preprocess_result = preprocess_result.clone();
        Callback::from(move |e: Event| {
            if let Some(target) = e.target()
                && let Some(select) = target.dyn_ref::<HtmlSelectElement>()
            {
                let idx: usize = select.value().parse().unwrap_or(0);
                let next_source = edited_sources
                    .get(idx)
                    .and_then(|source| source.clone())
                    .unwrap_or_else(|| DEMOS[idx].source.to_string());
                let next_macros = edited_macros
                    .get(idx)
                    .and_then(|macros| macros.clone())
                    .unwrap_or_else(|| {
                        DEMOS[idx]
                            .macros
                            .iter()
                            .map(|m| MacroFile::new(m.name.to_string(), m.source.to_string()))
                            .collect()
                    });
                source.set(next_source);
                macro_files.set(next_macros);
                selected_demo.set(Some(idx));
                editing_demo.set(idx);
                current_step.set(WizardStep::Source);
                compile_result.set(None);
                run_result.set(None);
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
        let editing_demo = editing_demo.clone();
        let edited_sources = edited_sources.clone();
        Callback::from(move |new_source: String| {
            let mut drafts = (*edited_sources).clone();
            if let Some(slot) = drafts.get_mut(*editing_demo) {
                *slot = Some(new_source.clone());
            }
            edited_sources.set(drafts);
            source.set(new_source);
            selected_demo.set(None);
        })
    };

    // File upload handler
    let on_file_upload = {
        let source = source.clone();
        let selected_demo = selected_demo.clone();
        let editing_demo = editing_demo.clone();
        let edited_sources = edited_sources.clone();
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
                let editing_demo = editing_demo.clone();
                let edited_sources = edited_sources.clone();
                let current_step = current_step.clone();
                let reader = gloo::file::callbacks::read_as_text(&file, move |result| {
                    if let Ok(text) = result {
                        let mut drafts = (*edited_sources).clone();
                        if let Some(slot) = drafts.get_mut(*editing_demo) {
                            *slot = Some(text.clone());
                        }
                        edited_sources.set(drafts);
                        source.set(text);
                        selected_demo.set(None);
                        current_step.set(WizardStep::Source);
                    }
                });
                file_reader.set(Some(reader));
            }
        })
    };

    // Wizard step click -- navigate back to any completed step
    let on_step_click = {
        let current_step = current_step.clone();
        Callback::from(move |step: WizardStep| {
            if step <= *current_step {
                current_step.set(step);
                let cell_id = step.cell_id().to_string();
                gloo::timers::callback::Timeout::new(50, move || {
                    if let Some(window) = web_sys::window()
                        && let Some(document) = window.document()
                        && let Some(element) = document.get_element_by_id(&cell_id)
                    {
                        element.scroll_into_view();
                    }
                })
                .forget();
            }
        })
    };

    // Wizard advance
    let on_advance = {
        let current_step = current_step.clone();
        let source = source.clone();
        let macro_files = macro_files.clone();
        let compile_result = compile_result.clone();
        let run_result = run_result.clone();
        let compiling = compiling.clone();
        let preprocess_result = preprocess_result.clone();
        Callback::from(move |()| {
            let current = *current_step;
            if let Some(next) = current.next() {
                // Preprocess step: just run the preprocessor (fast, synchronous)
                if next == WizardStep::Preprocess {
                    let macro_pairs: Vec<(String, String)> = macro_files
                        .iter()
                        .map(|m| (m.name.clone(), m.source.clone()))
                        .collect();
                    let pp_result = preprocessor::preprocess(&source, &macro_pairs);
                    preprocess_result.set(Some(pp_result));
                    current_step.set(WizardStep::Preprocess);

                    gloo::timers::callback::Timeout::new(50, || {
                        if let Some(window) = web_sys::window()
                            && let Some(document) = window.document()
                            && let Some(el) = document.get_element_by_id("cell-preprocess")
                        {
                            el.scroll_into_view();
                        }
                    })
                    .forget();
                    return;
                }

                // Compile step: run compiler on emulator only
                if next == WizardStep::Compile && !*compiling {
                    compiling.set(true);
                    let src = (*source).clone();
                    let macros: Vec<(String, String)> = macro_files
                        .iter()
                        .map(|m| (m.name.clone(), m.source.clone()))
                        .collect();

                    let compile_result = compile_result.clone();
                    let inner_step = current_step.clone();
                    let compiling = compiling.clone();

                    current_step.set(WizardStep::Compile);

                    gloo::timers::callback::Timeout::new(100, move || {
                        let result = pipeline::run_compiler(&src, &macros);
                        compile_result.set(Some(result));
                        compiling.set(false);
                        // Stay at Compile step — user clicks Assemble next
                        inner_step.set(WizardStep::Compile);

                        gloo::timers::callback::Timeout::new(50, || {
                            if let Some(window) = web_sys::window()
                                && let Some(document) = window.document()
                                && let Some(el) = document.get_element_by_id("cell-compile")
                            {
                                el.scroll_into_view();
                            }
                        })
                        .forget();
                    })
                    .forget();
                    return;
                }

                // Assemble step: just advance (assembly listing is already extracted)
                if next == WizardStep::Assemble {
                    current_step.set(WizardStep::Assemble);
                    gloo::timers::callback::Timeout::new(50, || {
                        if let Some(window) = web_sys::window()
                            && let Some(document) = window.document()
                            && let Some(el) = document.get_element_by_id("cell-assemble")
                        {
                            el.scroll_into_view();
                        }
                    })
                    .forget();
                    return;
                }

                // Run step: assemble and execute the program
                if next == WizardStep::Run
                    && !*compiling
                    && let Some(ref cr) = *compile_result
                    && let Some(ref asm_source) = cr.assembly
                {
                    compiling.set(true);
                    let asm = asm_source.clone();
                    let run_result = run_result.clone();
                    let compiling = compiling.clone();

                    current_step.set(WizardStep::Run);

                    gloo::timers::callback::Timeout::new(100, move || {
                        let rr = pipeline::run_program(&asm);
                        run_result.set(Some(rr));
                        compiling.set(false);

                        gloo::timers::callback::Timeout::new(50, || {
                            if let Some(window) = web_sys::window()
                                && let Some(document) = window.document()
                                && let Some(el) = document.get_element_by_id("cell-run")
                            {
                                el.scroll_into_view();
                            }
                        })
                        .forget();
                    })
                    .forget();
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
        let editing_demo = editing_demo.clone();
        let edited_macros = edited_macros.clone();
        Callback::from(move |(idx, new_source): (usize, String)| {
            let mut files = (*macro_files).clone();
            if let Some(f) = files.get_mut(idx) {
                f.source = new_source;
            }
            let mut drafts = (*edited_macros).clone();
            if let Some(slot) = drafts.get_mut(*editing_demo) {
                *slot = Some(files.clone());
            }
            edited_macros.set(drafts);
            macro_files.set(files);
        })
    };

    let on_macro_add = {
        let macro_files = macro_files.clone();
        let editing_demo = editing_demo.clone();
        let edited_macros = edited_macros.clone();
        Callback::from(move |()| {
            let mut files = (*macro_files).clone();
            let n = files.len() + 1;
            files.push(MacroFile::new(format!("MACRO{n}.msw"), String::new()));
            let mut drafts = (*edited_macros).clone();
            if let Some(slot) = drafts.get_mut(*editing_demo) {
                *slot = Some(files.clone());
            }
            edited_macros.set(drafts);
            macro_files.set(files);
        })
    };

    let on_macro_remove = {
        let macro_files = macro_files.clone();
        let editing_demo = editing_demo.clone();
        let edited_macros = edited_macros.clone();
        Callback::from(move |idx: usize| {
            let mut files = (*macro_files).clone();
            if idx < files.len() {
                files.remove(idx);
            }
            let mut drafts = (*edited_macros).clone();
            if let Some(slot) = drafts.get_mut(*editing_demo) {
                *slot = Some(files.clone());
            }
            edited_macros.set(drafts);
            macro_files.set(files);
        })
    };

    let on_macro_rename = {
        let macro_files = macro_files.clone();
        let editing_demo = editing_demo.clone();
        let edited_macros = edited_macros.clone();
        Callback::from(move |(idx, new_name): (usize, String)| {
            let mut files = (*macro_files).clone();
            if let Some(f) = files.get_mut(idx) {
                f.name = new_name;
            }
            let mut drafts = (*edited_macros).clone();
            if let Some(slot) = drafts.get_mut(*editing_demo) {
                *slot = Some(files.clone());
            }
            edited_macros.set(drafts);
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
        let editing_demo = editing_demo.clone();
        let edited_macros = edited_macros.clone();
        Callback::from(move |(name, source): (String, String)| {
            let mut files = (*macro_files).clone();
            files.push(MacroFile::new(name, source));
            let mut drafts = (*edited_macros).clone();
            if let Some(slot) = drafts.get_mut(*editing_demo) {
                *slot = Some(files.clone());
            }
            edited_macros.set(drafts);
            macro_files.set(files);
        })
    };

    let example_name = selected_demo.as_ref().map(|&idx| DEMOS[idx].name);
    let open_mass_compile = {
        let mass_compile_open = mass_compile_open.clone();
        Callback::from(move |_: MouseEvent| mass_compile_open.set(true))
    };
    let close_mass_compile = {
        let mass_compile_open = mass_compile_open.clone();
        Callback::from(move |()| mass_compile_open.set(false))
    };

    html! {
        <>
            // GitHub corner triangle
            <a href="https://github.com/sw-embed/web-sw-cor24-plsw"
               class="github-corner" aria-label="View source on GitHub" target="_blank">
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
            <MassCompileDialog
                open={*mass_compile_open}
                on_close={close_mass_compile}
                edited_sources={(*edited_sources).clone()}
                edited_macros={(*edited_macros).clone()}
            />
            // Main 2-column layout: sidebar | notebook
            <div id="app" class="plsw-layout">
                // Left panel: controls + wizard steps
                <div class="side-panel">
                    <div class="sidebar-section">
                        <span class="sidebar-section-label">{"Demos"}</span>
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
                        if let Some(idx) = *selected_demo {
                            <span class="sidebar-demo-desc">{DEMOS[idx].description}</span>
                        }
                    </div>

                    <div class="sidebar-section">
                        <input type="file" id="file-upload" class="file-upload-input"
                            accept=".plsw,.msw,.txt" onchange={on_file_upload} />
                        <label for="file-upload" class="file-upload-label">{"Upload .plsw"}</label>
                    </div>

                    <div class="sidebar-divider"></div>

                    // Wizard steps
                    <WizardSidebar
                        current_step={*current_step}
                        {on_step_click}
                        {on_advance}
                        has_source={!source.is_empty()}
                    />

                    <div class="sidebar-spacer"></div>
                </div>

                // Right: Notebook cells
                <div class="notebook-cells" id="notebook-scroll">
                    // Cell: Source editor (always visible)
                    <SourceEditor
                        source={(*source).clone()}
                        on_change={on_source_change}
                        title="PL/SW Source"
                        example_name={example_name.map(AttrValue::from)}
                        on_mass_compile={Some(open_mass_compile)}
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
                        // Consolidated preprocessed source
                        <div class="notebook-cell" id="cell-preprocess">
                            <div class="cell-header">
                                <span>{"Preprocessed Source"}</span>
                                if let Some(ref pp) = *preprocess_result {
                                    <span class="cell-header-stats">
                                        {format!("{} lines",
                                            pp.output.lines().count())}
                                    </span>
                                }
                            </div>
                            <div class="cell-content">
                                if let Some(ref pp) = *preprocess_result {
                                    <pre class="pipeline-output">{number_lines(&pp.output)}</pre>
                                } else {
                                    <div class="notebook-placeholder">
                                        <span>{"Click Preprocess to expand macros"}</span>
                                    </div>
                                }
                            </div>
                        </div>

                        // Compiler output
                        <div class="notebook-cell" id="cell-compile">
                            <div class="cell-header">
                                <span>{"Compiler Output"}</span>
                                if let Some(ref result) = *compile_result {
                                    if result.error.is_some() {
                                        <span class="cell-status-error">{"\u{2717} Error"}</span>
                                    } else {
                                        <span class="cell-status-ok">{"\u{2713} Compiled"}</span>
                                    }
                                    <span class="cell-header-stats">
                                        {format!("{} instructions", format_count(result.instructions))}
                                    </span>
                                }
                            </div>
                            <div class="cell-content">
                                if *compiling {
                                    <div class="compile-status">
                                        <span class="compile-spinner">{"\u{23F3}"}</span>
                                        <span>{"Compiling PL/SW on COR24 emulator..."}</span>
                                    </div>
                                } else if let Some(ref result) = *compile_result {
                                    if let Some(ref err) = result.error {
                                        <div class="compile-error">
                                            <span class="error-label">{"Error: "}</span>
                                            <span>{err}</span>
                                        </div>
                                    }
                                    if !result.compiler_output.is_empty() {
                                        <pre class="pipeline-output">{&result.compiler_output}</pre>
                                    }
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
                                if let Some(ref result) = *compile_result {
                                    if result.assembly.is_some() {
                                        <span class="cell-status-ok">{"\u{2713} Generated"}</span>
                                    }
                                }
                            </div>
                            <div class="cell-content">
                                if let Some(ref result) = *compile_result {
                                    if let Some(ref asm) = result.assembly {
                                        <pre class="pipeline-output assembly-listing">{asm}</pre>
                                    } else {
                                        <div class="compile-error">
                                            <span>{"No assembly generated"}</span>
                                        </div>
                                    }
                                }
                            </div>
                        </div>
                    }

                    if *current_step >= WizardStep::Run {
                        <div class="notebook-cell" id="cell-run">
                            <div class="cell-header">
                                <span>{"Program Output"}</span>
                                if let Some(ref rr) = *run_result {
                                    if rr.error.is_some() {
                                        <span class="cell-status-error">{"\u{2717} Error"}</span>
                                    } else if rr.halted {
                                        <span class="cell-status-ok">{"\u{2713} Halted"}</span>
                                    }
                                    <span class="cell-header-stats">
                                        {format!("{} instructions", format_count(rr.instructions))}
                                    </span>
                                }
                            </div>
                            <div class="cell-content">
                                if let Some(ref rr) = *run_result {
                                    if let Some(ref err) = rr.error {
                                        <div class="compile-error">
                                            <span class="error-label">{"Error: "}</span>
                                            <span>{err}</span>
                                        </div>
                                    }
                                    if !rr.output.is_empty() {
                                        <pre class="pipeline-output run-output">{&rr.output}</pre>
                                    } else if rr.error.is_none() {
                                        <div class="compile-status">
                                            <span>{"Program produced no UART output"}</span>
                                        </div>
                                    }
                                } else {
                                    <div class="notebook-placeholder">
                                        <span>{"Waiting for execution..."}</span>
                                    </div>
                                }
                            </div>
                        </div>
                    }

                    // Emulator state dump (after run)
                    if *current_step >= WizardStep::Run {
                        if let Some(ref rr) = *run_result {
                            if let Some(ref dump) = rr.dump {
                                <div class="notebook-cell" id="cell-dump">
                                    <div class="cell-header">
                                        <span>{"Emulator State"}</span>
                                        <span class="cell-header-stats">
                                            {format!("{} cycles", format_count(dump.cycles))}
                                        </span>
                                    </div>
                                    <div class="cell-content">
                                        <pre class="pipeline-output dump-output">{format_dump(dump)}</pre>
                                    </div>
                                </div>
                            }
                        }
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
                <a href="https://software-wrighter-lab.github.io/" target="_blank">{"Blog"}</a>
                <span class="footer-sep">{"\u{00b7}"}</span>
                <a href="https://discord.com/invite/Ctzk5uHggZ" target="_blank">{"Discord"}</a>
                <span class="footer-sep">{"\u{00b7}"}</span>
                <a href="https://www.youtube.com/@SoftwareWrighter" target="_blank">{"YouTube"}</a>
                <span class="footer-sep">{"\u{00b7}"}</span>
                <a href="/web-sw-cor24-plsw/CHANGES" target="_blank">{"CHANGES"}</a>
                <span class="footer-sep">{"\u{00b7}"}</span>
                <span>{ format!("{} \u{00b7} {} \u{00b7} {}",
                    env!("BUILD_HOST"),
                    env!("BUILD_SHA"),
                    env!("BUILD_TIMESTAMP"),
                ) }</span>
            </footer>
        </>
    }
}
