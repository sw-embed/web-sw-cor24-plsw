//! Mass Compile dialog for running several demo compile jobs sequentially.

use std::cell::RefCell;
use std::rc::Rc;

use crate::components::MacroFile;
use crate::demos::DEMOS;
use crate::pipeline;
use gloo::timers::callback::Timeout;
use yew::platform::spawn_local;
use yew::prelude::*;

const STAGE_DELAY_MS: u32 = 700;

#[derive(Clone, Copy, PartialEq, Eq)]
enum JobState {
    Queued,
    Waiting,
    Compiling,
    Assembling,
    Running,
    Complete,
    Failed,
}

impl JobState {
    fn label(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Waiting => "waiting",
            Self::Compiling => "compiling",
            Self::Assembling => "assembling",
            Self::Running => "running",
            Self::Complete => "complete",
            Self::Failed => "failed",
        }
    }

    fn class(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Waiting => "waiting",
            Self::Compiling | Self::Assembling | Self::Running => "active",
            Self::Complete => "complete",
            Self::Failed => "failed",
        }
    }

    fn is_active(self) -> bool {
        matches!(self, Self::Compiling | Self::Assembling | Self::Running)
    }

    fn is_editable(self) -> bool {
        matches!(
            self,
            Self::Queued | Self::Waiting | Self::Complete | Self::Failed
        )
    }

    fn can_resubmit(self) -> bool {
        matches!(self, Self::Complete | Self::Failed)
    }
}

#[derive(Clone, PartialEq)]
struct JobLog {
    compiler_input: String,
    compiler_output: String,
    assembly_source: String,
    assembler_listing: String,
    assembler_error: String,
    run_output: String,
    run_error: String,
}

impl JobLog {
    fn new() -> Self {
        Self {
            compiler_input: String::new(),
            compiler_output: String::new(),
            assembly_source: String::new(),
            assembler_listing: String::new(),
            assembler_error: String::new(),
            run_output: String::new(),
            run_error: String::new(),
        }
    }
}

#[derive(Clone, PartialEq)]
struct MassJob {
    demo_idx: usize,
    state: JobState,
    log: JobLog,
    summary: String,
    source_override: Option<String>,
    scratch_buffer: Option<String>,
    scratch_dirty: bool,
    compile_source: Option<String>,
    compile_macros: Option<Vec<(String, String)>>,
    worker_started: bool,
}

impl MassJob {
    fn new(demo_idx: usize) -> Self {
        Self {
            demo_idx,
            state: JobState::Queued,
            log: JobLog::new(),
            summary: String::new(),
            source_override: None,
            scratch_buffer: None,
            scratch_dirty: false,
            compile_source: None,
            compile_macros: None,
            worker_started: false,
        }
    }

    fn with_scratch(
        demo_idx: usize,
        source_override: Option<String>,
        scratch_buffer: Option<String>,
        scratch_dirty: bool,
    ) -> Self {
        let mut job = Self::new(demo_idx);
        job.source_override = source_override;
        job.scratch_buffer = scratch_buffer;
        job.scratch_dirty = scratch_dirty;
        if job.has_unsaved_scratch() {
            job.summary = "Unsaved scratch edit".into();
        }
        job
    }

    fn from_draft(row: &DraftRow) -> Self {
        Self::with_scratch(
            row.demo_idx,
            row.source_override.clone(),
            row.scratch_buffer.clone(),
            row.scratch_dirty,
        )
    }

    fn reset_for_submit(&mut self) {
        self.state = JobState::Queued;
        self.log = JobLog::new();
        self.compile_source = None;
        self.compile_macros = None;
        self.worker_started = false;
        self.summary = if self.has_unsaved_scratch() {
            "Unsaved scratch edit".into()
        } else {
            String::new()
        };
    }

    fn has_unsaved_scratch(&self) -> bool {
        self.scratch_dirty || self.scratch_buffer != self.source_override
    }
}

#[derive(Clone, PartialEq)]
struct DraftRow {
    demo_idx: usize,
    source_override: Option<String>,
    scratch_buffer: Option<String>,
    scratch_dirty: bool,
}

impl DraftRow {
    fn new() -> Self {
        Self {
            demo_idx: 0,
            source_override: None,
            scratch_buffer: None,
            scratch_dirty: false,
        }
    }

    fn has_unsaved_scratch(&self) -> bool {
        self.scratch_dirty || self.scratch_buffer != self.source_override
    }
}

struct JobControls<'a> {
    jobs: &'a UseStateHandle<Vec<MassJob>>,
    jobs_ref: &'a Rc<RefCell<Vec<MassJob>>>,
    running: &'a UseStateHandle<bool>,
    selected_job: &'a UseStateHandle<Option<usize>>,
}

#[derive(Properties, PartialEq)]
pub struct MassCompileDialogProps {
    pub open: bool,
    pub on_close: Callback<()>,
    pub edited_sources: Vec<Option<String>>,
    pub edited_macros: Vec<Option<Vec<MacroFile>>>,
}

#[function_component(MassCompileDialog)]
pub fn mass_compile_dialog(props: &MassCompileDialogProps) -> Html {
    let draft_rows = use_state(|| vec![DraftRow::new()]);
    let selected_draft = use_state(|| 0usize);
    let jobs = use_state(Vec::<MassJob>::new);
    let jobs_ref = use_mut_ref(Vec::<MassJob>::new);
    let running = use_state(|| false);
    let selected_job = use_state(|| None::<usize>);

    {
        let jobs = jobs.clone();
        let jobs_ref = jobs_ref.clone();
        let running = running.clone();
        let selected_job = selected_job.clone();
        use_effect_with(
            (
                (*jobs).clone(),
                *running,
                props.edited_sources.clone(),
                props.edited_macros.clone(),
            ),
            move |(snapshot, is_running, edited_sources, edited_macros)| {
                if *is_running {
                    if let Some((idx, job)) = snapshot
                        .iter()
                        .enumerate()
                        .find(|(_, job)| job.state.is_active())
                    {
                        let stage = job.state;
                        Timeout::new(STAGE_DELAY_MS, move || {
                            advance_job(idx, stage, &jobs, &jobs_ref, &running);
                        })
                        .forget();
                    } else if let Some(idx) = snapshot
                        .iter()
                        .position(|job| matches!(job.state, JobState::Queued | JobState::Waiting))
                    {
                        let mut next = snapshot.clone();
                        if next[idx].has_unsaved_scratch() {
                            if next[idx].state != JobState::Waiting {
                                next[idx].state = JobState::Waiting;
                                next[idx].summary = "Waiting for scratch save".into();
                                set_jobs(&jobs, &jobs_ref, next);
                            }
                            if selected_job.is_none() {
                                selected_job.set(Some(idx));
                            }
                            return;
                        }
                        let (mut source, macros) =
                            job_source(next[idx].demo_idx, edited_sources, edited_macros);
                        if let Some(override_source) = &next[idx].source_override {
                            source = override_source.clone();
                        }
                        next[idx].state = JobState::Compiling;
                        next[idx].log.compiler_input = compiler_input_log(&source, &macros);
                        next[idx].compile_source = Some(source);
                        next[idx].compile_macros = Some(macros);
                        next[idx].worker_started = false;
                        next[idx].summary = "Starting compiler".into();
                        set_jobs(&jobs, &jobs_ref, next);
                        if selected_job.is_none() {
                            selected_job.set(Some(idx));
                        }
                    } else {
                        running.set(false);
                    }
                }
            },
        );
    }

    let add_row = {
        let draft_rows = draft_rows.clone();
        let selected_draft = selected_draft.clone();
        Callback::from(move |_: MouseEvent| {
            let mut rows = (*draft_rows).clone();
            rows.push(DraftRow::new());
            selected_draft.set(rows.len() - 1);
            draft_rows.set(rows);
        })
    };

    let submit_all_drafts = {
        let draft_rows = draft_rows.clone();
        let jobs = jobs.clone();
        let jobs_ref = jobs_ref.clone();
        let running = running.clone();
        let selected_job = selected_job.clone();
        Callback::from(move |_: MouseEvent| {
            let next_jobs = draft_rows
                .iter()
                .map(MassJob::from_draft)
                .collect::<Vec<_>>();
            selected_job.set(if next_jobs.is_empty() { None } else { Some(0) });
            set_jobs(&jobs, &jobs_ref, next_jobs);
            running.set(true);
        })
    };

    let submit_all_jobs = {
        let jobs = jobs.clone();
        let jobs_ref = jobs_ref.clone();
        let running = running.clone();
        Callback::from(move |_: MouseEvent| {
            let mut next = jobs_ref.borrow().clone();
            for job in &mut next {
                job.reset_for_submit();
            }
            set_jobs(&jobs, &jobs_ref, next);
            running.set(true);
        })
    };

    let close = {
        let on_close = props.on_close.clone();
        Callback::from(move |_: MouseEvent| on_close.emit(()))
    };

    if !props.open {
        return html! {};
    }

    let selected_log = selected_job.and_then(|idx| jobs.get(idx).map(|job| (idx, job.clone())));
    let has_jobs = !jobs.is_empty();
    let can_submit_all_jobs = !jobs.is_empty()
        && jobs
            .iter()
            .all(|job| matches!(job.state, JobState::Complete | JobState::Failed));
    let job_controls = JobControls {
        jobs: &jobs,
        jobs_ref: &jobs_ref,
        running: &running,
        selected_job: &selected_job,
    };

    html! {
        <div class="modal-backdrop">
            <div class="mass-compile-dialog" role="dialog" aria-modal="true">
                <div class="mass-compile-header">
                    <span>{"Mass Compile"}</span>
                    <button class="editor-action-btn" onclick={close.clone()}>{"Close"}</button>
                </div>
                <div class="mass-compile-body">
                    <section class="mass-compile-left">
                        if !has_jobs {
                            <div class="mass-compile-form">
                                { for draft_rows.iter().enumerate().map(|(idx, row)| {
                                    render_draft_row(idx, row, &draft_rows, &selected_draft, &job_controls)
                                })}
                                <div class="mass-compile-actions">
                                    <button class="sidebar-btn" onclick={add_row}>{"Add Line"}</button>
                                    <button class="sidebar-btn" onclick={submit_all_drafts}>{"Submit All"}</button>
                                </div>
                                if let Some(row) = draft_rows.get(*selected_draft) {
                                    {render_draft_scratch(
                                        *selected_draft,
                                        row,
                                        &draft_rows,
                                        &props.edited_sources,
                                        &props.edited_macros,
                                    )}
                                }
                            </div>
                        } else {
                            <div class="mass-job-list">
                                { for jobs.iter().enumerate().map(|(idx, job)| {
                                    render_job_row(idx, job, &selected_job, &jobs, &jobs_ref, &running)
                                })}
                            </div>
                            if let Some(idx) = *selected_job
                                && let Some(job) = jobs.get(idx)
                            {
                                {render_job_scratch(idx, job, &jobs, &jobs_ref, &props.edited_sources, &props.edited_macros)}
                            }
                            if can_submit_all_jobs {
                                <div class="mass-compile-actions">
                                    <button class="sidebar-btn" onclick={submit_all_jobs}>{"Submit All"}</button>
                                </div>
                            }
                        }
                    </section>
                    <section class="mass-compile-log-panel">
                        if let Some((idx, job)) = selected_log {
                            {render_job_log(idx, &job)}
                        } else {
                            <div class="mass-log-empty">{"Select a submitted job to view its log."}</div>
                        }
                    </section>
                </div>
            </div>
        </div>
    }
}

fn render_draft_scratch(
    idx: usize,
    row: &DraftRow,
    draft_rows: &UseStateHandle<Vec<DraftRow>>,
    edited_sources: &[Option<String>],
    edited_macros: &[Option<Vec<MacroFile>>],
) -> Html {
    let (default_source, _) = job_source(row.demo_idx, edited_sources, edited_macros);
    let value = row
        .scratch_buffer
        .clone()
        .or_else(|| row.source_override.clone())
        .unwrap_or(default_source);

    let oninput = {
        let draft_rows = draft_rows.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(target) = e.target_dyn_into::<web_sys::HtmlTextAreaElement>() {
                let mut rows = (*draft_rows).clone();
                if let Some(row) = rows.get_mut(idx) {
                    row.scratch_buffer = Some(target.value());
                    row.scratch_dirty = true;
                }
                draft_rows.set(rows);
            }
        })
    };

    let save = {
        let draft_rows = draft_rows.clone();
        Callback::from(move |_: MouseEvent| {
            let mut rows = (*draft_rows).clone();
            if let Some(row) = rows.get_mut(idx)
                && let Some(buffer) = row.scratch_buffer.clone()
            {
                row.source_override = Some(buffer);
                row.scratch_dirty = false;
            }
            draft_rows.set(rows);
        })
    };

    html! {
        <div class="mass-job-scratch">
            <div class="mass-job-scratch-bar">
                <span class="mass-job-scratch-title">{"Draft Source Scratch"}</span>
                <button class="sidebar-btn" disabled={!row.has_unsaved_scratch()} onclick={save}>{"Save"}</button>
            </div>
            <textarea
                class="mass-job-scratch-editor"
                value={value}
                {oninput}
            />
        </div>
    }
}

fn render_job_scratch(
    idx: usize,
    job: &MassJob,
    jobs: &UseStateHandle<Vec<MassJob>>,
    jobs_ref: &Rc<RefCell<Vec<MassJob>>>,
    edited_sources: &[Option<String>],
    edited_macros: &[Option<Vec<MacroFile>>],
) -> Html {
    let editable = job.state.is_editable();
    let (default_source, _) = job_source(job.demo_idx, edited_sources, edited_macros);
    let value = job
        .scratch_buffer
        .clone()
        .or_else(|| job.source_override.clone())
        .or_else(|| job.compile_source.clone())
        .unwrap_or(default_source);
    let oninput = {
        let jobs = jobs.clone();
        let jobs_ref = jobs_ref.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(target) = e.target_dyn_into::<web_sys::HtmlTextAreaElement>() {
                let mut next = jobs_ref.borrow().clone();
                if let Some(job) = next.get_mut(idx)
                    && job.state.is_editable()
                {
                    job.scratch_buffer = Some(target.value());
                    job.scratch_dirty = true;
                    if job.state == JobState::Waiting {
                        job.summary = "Waiting for scratch save".into();
                    } else {
                        job.summary = "Unsaved scratch edit".into();
                    }
                    job.log.compiler_input.clear();
                }
                set_jobs(&jobs, &jobs_ref, next);
            }
        })
    };

    let save = {
        let jobs = jobs.clone();
        let jobs_ref = jobs_ref.clone();
        Callback::from(move |_: MouseEvent| {
            let mut next = jobs_ref.borrow().clone();
            if let Some(job) = next.get_mut(idx)
                && job.state.is_editable()
                && let Some(buffer) = job.scratch_buffer.clone()
            {
                job.source_override = Some(buffer);
                job.scratch_dirty = false;
                if job.state == JobState::Waiting {
                    job.state = JobState::Queued;
                    job.summary = String::new();
                } else if matches!(job.state, JobState::Queued) {
                    job.summary = String::new();
                } else if job.state.can_resubmit() {
                    job.summary = "Saved scratch edit; submit line to rerun".into();
                }
            }
            set_jobs(&jobs, &jobs_ref, next);
        })
    };

    html! {
        <div class="mass-job-scratch">
            <div class="mass-job-scratch-bar">
                <span class="mass-job-scratch-title">{"Queued Source Scratch"}</span>
                <button class="sidebar-btn" disabled={!job.has_unsaved_scratch()} onclick={save}>{"Save"}</button>
            </div>
            <textarea
                class="mass-job-scratch-editor"
                value={value}
                disabled={!editable}
                {oninput}
            />
        </div>
    }
}

fn render_draft_row(
    idx: usize,
    row: &DraftRow,
    draft_rows: &UseStateHandle<Vec<DraftRow>>,
    selected_draft: &UseStateHandle<usize>,
    controls: &JobControls<'_>,
) -> Html {
    let onchange = {
        let draft_rows = draft_rows.clone();
        let selected_draft = selected_draft.clone();
        Callback::from(move |e: Event| {
            if let Some(target) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                let mut rows = (*draft_rows).clone();
                if let Ok(next_idx) = target.value().parse::<usize>()
                    && let Some(row) = rows.get_mut(idx)
                {
                    row.demo_idx = next_idx;
                    row.source_override = None;
                    row.scratch_buffer = None;
                    row.scratch_dirty = false;
                    selected_draft.set(idx);
                    draft_rows.set(rows);
                }
            }
        })
    };

    let select_row = {
        let selected_draft = selected_draft.clone();
        Callback::from(move |_: MouseEvent| selected_draft.set(idx))
    };

    let remove = {
        let draft_rows = draft_rows.clone();
        let selected_draft = selected_draft.clone();
        Callback::from(move |_: MouseEvent| {
            let mut rows = (*draft_rows).clone();
            if rows.len() > 1 && idx < rows.len() {
                rows.remove(idx);
                selected_draft.set(idx.saturating_sub(1).min(rows.len() - 1));
                draft_rows.set(rows);
            }
        })
    };

    let submit_line = {
        let draft_rows = draft_rows.clone();
        let jobs = controls.jobs.clone();
        let jobs_ref = controls.jobs_ref.clone();
        let running = controls.running.clone();
        let selected_job = controls.selected_job.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(row) = draft_rows.get(idx) {
                let job = MassJob::from_draft(row);
                let mut next = jobs_ref.borrow().clone();
                next.push(job);
                let job_idx = next.len() - 1;
                set_jobs(&jobs, &jobs_ref, next);
                selected_job.set(Some(job_idx));
                running.set(true);
            }
        })
    };

    html! {
        <div class="mass-form-row">
            <span class="mass-row-num">{idx + 1}</span>
            <select class="sidebar-select" {onchange} onclick={select_row} value={row.demo_idx.to_string()}>
                { for DEMOS.iter().enumerate().map(|(i, demo)| html! {
                    <option value={i.to_string()} selected={i == row.demo_idx}>{demo.name}</option>
                })}
            </select>
            <button class="editor-action-btn" onclick={submit_line}>{"Submit"}</button>
            <button class="editor-action-btn" onclick={remove}>{"Remove"}</button>
        </div>
    }
}

fn render_job_row(
    idx: usize,
    job: &MassJob,
    selected_job: &UseStateHandle<Option<usize>>,
    jobs: &UseStateHandle<Vec<MassJob>>,
    jobs_ref: &Rc<RefCell<Vec<MassJob>>>,
    running: &UseStateHandle<bool>,
) -> Html {
    let onclick = {
        let selected_job = selected_job.clone();
        Callback::from(move |_: MouseEvent| selected_job.set(Some(idx)))
    };
    let is_selected = **selected_job == Some(idx);
    let submit_line = {
        let jobs = jobs.clone();
        let jobs_ref = jobs_ref.clone();
        let running = running.clone();
        Callback::from(move |_: MouseEvent| {
            let mut next = jobs_ref.borrow().clone();
            if let Some(job) = next.get_mut(idx)
                && job.state.can_resubmit()
            {
                job.reset_for_submit();
                set_jobs(&jobs, &jobs_ref, next);
                running.set(true);
            }
        })
    };
    let change_demo = {
        let jobs = jobs.clone();
        let jobs_ref = jobs_ref.clone();
        Callback::from(move |e: Event| {
            if let Some(target) = e.target_dyn_into::<web_sys::HtmlSelectElement>()
                && let Ok(demo_idx) = target.value().parse::<usize>()
            {
                let mut next = jobs_ref.borrow().clone();
                if let Some(job) = next.get_mut(idx)
                    && job.state.can_resubmit()
                {
                    job.demo_idx = demo_idx;
                    job.source_override = None;
                    job.scratch_buffer = None;
                    job.scratch_dirty = false;
                    job.compile_source = None;
                    job.compile_macros = None;
                    job.worker_started = false;
                    job.log = JobLog::new();
                    job.summary = "Changed demo; submit line to run".into();
                    set_jobs(&jobs, &jobs_ref, next);
                }
            }
        })
    };

    html! {
        <div class={classes!("mass-job-row", is_selected.then_some("selected"))} {onclick}>
            if job.state.can_resubmit() {
                <select class="sidebar-select mass-job-demo-select" onchange={change_demo} value={job.demo_idx.to_string()}>
                    { for DEMOS.iter().enumerate().map(|(i, demo)| html! {
                        <option value={i.to_string()} selected={i == job.demo_idx}>{demo.name}</option>
                    })}
                </select>
            } else {
                <span class="mass-job-demo">{DEMOS[job.demo_idx].name}</span>
            }
            <span class={classes!("mass-job-state", job.state.class())}>{job.state.label()}</span>
            if !job.summary.is_empty() {
                <span class="mass-job-summary">{&job.summary}</span>
            }
            if job.state.can_resubmit() {
                <button class="editor-action-btn mass-job-submit" onclick={submit_line}>{"Submit"}</button>
            }
        </div>
    }
}

fn render_job_log(idx: usize, job: &MassJob) -> Html {
    html! {
        <div class="mass-log">
            <div class="mass-log-title">
                {format!("Job {}: {} ({})", idx + 1, DEMOS[job.demo_idx].name, job.state.label())}
            </div>
            {log_block("Compiler Input", &job.log.compiler_input)}
            {log_block("Compiler Output", &job.log.compiler_output)}
            {log_block("Generated Assembly", &job.log.assembly_source)}
            {log_block("Assembler Object Listing", &job.log.assembler_listing)}
            {log_block("Assembler Errors", &job.log.assembler_error)}
            {log_block("Run Output", &job.log.run_output)}
            {log_block("Run Errors", &job.log.run_error)}
        </div>
    }
}

fn log_block(title: &str, content: &str) -> Html {
    let body = if content.is_empty() {
        "(not available yet)"
    } else {
        content
    };
    html! {
        <details class="mass-log-block" open=true>
            <summary>{title}</summary>
            <pre>{body}</pre>
        </details>
    }
}

fn set_jobs(
    jobs: &UseStateHandle<Vec<MassJob>>,
    jobs_ref: &Rc<RefCell<Vec<MassJob>>>,
    next: Vec<MassJob>,
) {
    *jobs_ref.borrow_mut() = next.clone();
    jobs.set(next);
}

fn advance_job(
    idx: usize,
    stage: JobState,
    jobs: &UseStateHandle<Vec<MassJob>>,
    jobs_ref: &Rc<RefCell<Vec<MassJob>>>,
    running: &UseStateHandle<bool>,
) {
    let mut next = jobs_ref.borrow().clone();
    if idx >= next.len() {
        return;
    }
    if next[idx].state != stage {
        return;
    }

    match stage {
        JobState::Compiling => {
            if next[idx].worker_started {
                return;
            }
            let Some(source) = next[idx].compile_source.clone() else {
                next[idx].state = JobState::Failed;
                next[idx].log.assembler_error = "Compiler input was not captured".into();
                set_jobs(jobs, jobs_ref, next);
                return;
            };
            let Some(macro_sources) = next[idx].compile_macros.clone() else {
                next[idx].state = JobState::Failed;
                next[idx].log.assembler_error = "Compiler macro input was not captured".into();
                set_jobs(jobs, jobs_ref, next);
                return;
            };
            next[idx].worker_started = true;
            set_jobs(jobs, jobs_ref, next);
            start_compile_job(
                idx,
                source,
                macro_sources,
                jobs.clone(),
                jobs_ref.clone(),
                running.clone(),
            );
            return;
        }
        JobState::Assembling => assemble_job(idx, &mut next),
        JobState::Running => run_job(idx, &mut next),
        _ => {}
    }

    if next
        .iter()
        .all(|job| matches!(job.state, JobState::Complete | JobState::Failed))
    {
        running.set(false);
    }
    set_jobs(jobs, jobs_ref, next);
}

fn start_compile_job(
    idx: usize,
    source: String,
    macro_sources: Vec<(String, String)>,
    jobs: UseStateHandle<Vec<MassJob>>,
    jobs_ref: Rc<RefCell<Vec<MassJob>>>,
    running: UseStateHandle<bool>,
) {
    spawn_local(async move {
        let result = pipeline::run_compiler_cooperative(&source, &macro_sources).await;
        let mut next = jobs_ref.borrow().clone();
        if idx >= next.len() || next[idx].state != JobState::Compiling {
            return;
        }

        next[idx].log.compiler_output = result.compiler_output;
        next[idx].summary = format!("compile: {} instructions", result.instructions);

        if let Some(error) = result.error {
            next[idx].log.assembler_error = error;
            next[idx].state = JobState::Failed;
        } else if let Some(assembly) = result.assembly {
            next[idx].log.assembly_source = assembly;
            next[idx].state = JobState::Assembling;
        } else {
            next[idx].log.assembler_error = "Compiler produced no assembly".into();
            next[idx].state = JobState::Failed;
        }

        if next
            .iter()
            .all(|job| matches!(job.state, JobState::Complete | JobState::Failed))
        {
            running.set(false);
        }
        set_jobs(&jobs, &jobs_ref, next);
    });
}

fn assemble_job(idx: usize, jobs: &mut [MassJob]) {
    match pipeline::assemble_program(&jobs[idx].log.assembly_source) {
        Ok(report) => {
            jobs[idx].log.assembler_listing = report.listing;
            jobs[idx].summary = format!("assembled {} bytes", report.byte_count);
            jobs[idx].state = JobState::Running;
        }
        Err(error) => {
            jobs[idx].log.assembler_error = error;
            jobs[idx].state = JobState::Failed;
        }
    }
}

fn run_job(idx: usize, jobs: &mut [MassJob]) {
    let result = pipeline::run_program(&jobs[idx].log.assembly_source);
    jobs[idx].log.run_output = result.output;
    jobs[idx].summary = format!("run: {} instructions", result.instructions);
    if let Some(error) = result.error {
        jobs[idx].log.run_error = error;
        jobs[idx].state = JobState::Failed;
    } else {
        jobs[idx].state = JobState::Complete;
    }
}

fn default_macro_sources(demo_idx: usize) -> Vec<(String, String)> {
    DEMOS[demo_idx]
        .macros
        .iter()
        .map(|m| (m.name.to_string(), m.source.to_string()))
        .collect()
}

fn editor_macro_sources(macros: &[MacroFile]) -> Vec<(String, String)> {
    macros
        .iter()
        .map(|m| (m.name.clone(), m.source.clone()))
        .collect()
}

fn job_source(
    job_demo_idx: usize,
    edited_sources: &[Option<String>],
    edited_macros: &[Option<Vec<MacroFile>>],
) -> (String, Vec<(String, String)>) {
    let source = edited_sources
        .get(job_demo_idx)
        .and_then(|source| source.clone())
        .unwrap_or_else(|| DEMOS[job_demo_idx].source.to_string());
    let macros = edited_macros
        .get(job_demo_idx)
        .and_then(|macros| macros.clone())
        .map(|macros| editor_macro_sources(&macros))
        .unwrap_or_else(|| default_macro_sources(job_demo_idx));
    (source, macros)
}

fn compiler_input_log(source: &str, macro_sources: &[(String, String)]) -> String {
    let mut input = String::new();
    for (name, macro_source) in macro_sources {
        let include_name = name.strip_suffix(".msw").unwrap_or(name);
        input.push_str(&format!("FILE:{include_name}\n"));
        input.push_str(macro_source);
        if !macro_source.ends_with('\n') {
            input.push('\n');
        }
        input.push_str("<RS>\n");
    }
    if macro_sources.is_empty() {
        input.push_str(source);
    } else {
        input.push_str("SOURCE:\n");
        input.push_str(source);
    }
    if !input.ends_with('\n') {
        input.push('\n');
    }
    input.push_str("<EOT>");
    input
}
