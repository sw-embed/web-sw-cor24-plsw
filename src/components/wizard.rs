//! Wizard sidebar with progressive disclosure steps.
//!
//! Follows the cor24-rs wizard pattern: step indicators in column 2,
//! with an action button that advances to the next step.

use yew::prelude::*;

/// Pipeline steps for progressive disclosure.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WizardStep {
    Source,
    Macros,
    Preprocess,
    Compile,
    Assemble,
    Run,
}

impl WizardStep {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Source => "Source",
            Self::Macros => "Macros",
            Self::Preprocess => "Preprocess",
            Self::Compile => "Compile",
            Self::Assemble => "Assemble",
            Self::Run => "Run",
        }
    }

    pub fn tooltip(&self) -> &'static str {
        match self {
            Self::Source => "Edit PL/SW source (.plsw)",
            Self::Macros => "Edit macro files (.msw)",
            Self::Preprocess => "Expand macros and includes",
            Self::Compile => "Compile PL/SW to COR24 assembly",
            Self::Assemble => "Assemble to machine code",
            Self::Run => "Execute and debug",
        }
    }

    pub fn action_label(&self) -> &'static str {
        match self {
            Self::Source => "Edit Macros",
            Self::Macros => "Preprocess",
            Self::Preprocess => "Compile",
            Self::Compile => "Assemble",
            Self::Assemble => "Run",
            Self::Run => "",
        }
    }

    pub fn next(&self) -> Option<WizardStep> {
        match self {
            Self::Source => Some(Self::Macros),
            Self::Macros => Some(Self::Preprocess),
            Self::Preprocess => Some(Self::Compile),
            Self::Compile => Some(Self::Assemble),
            Self::Assemble => Some(Self::Run),
            Self::Run => None,
        }
    }

    pub fn cell_id(&self) -> &'static str {
        match self {
            Self::Source => "cell-source",
            Self::Macros => "cell-macros",
            Self::Preprocess => "cell-preprocess",
            Self::Compile => "cell-compile",
            Self::Assemble => "cell-assemble",
            Self::Run => "cell-run",
        }
    }

    const ALL: [WizardStep; 6] = [
        Self::Source,
        Self::Macros,
        Self::Preprocess,
        Self::Compile,
        Self::Assemble,
        Self::Run,
    ];
}

#[derive(Properties, PartialEq)]
pub struct WizardSidebarProps {
    pub current_step: WizardStep,
    pub on_step_click: Callback<WizardStep>,
    pub on_advance: Callback<()>,
    pub has_source: bool,
}

/// Wizard step indicators column (column 2 of the 3-column layout).
#[function_component(WizardSidebar)]
pub fn wizard_sidebar(props: &WizardSidebarProps) -> Html {
    let on_step_click = props.on_step_click.clone();

    html! {
        <div class="wizard-steps">
            { for WizardStep::ALL.iter().map(|&step| {
                let is_completed = props.has_source && step <= props.current_step;
                let is_active = step == props.current_step;
                let is_disabled = step > props.current_step;

                let class = classes!(
                    "wizard-step",
                    is_active.then_some("active"),
                    is_completed.then_some("completed"),
                    is_disabled.then_some("disabled"),
                );

                let indicator = if is_completed && !is_active { "\u{2713}" } else { "\u{25cb}" };

                let onclick = if !is_disabled {
                    let cb = on_step_click.clone();
                    Some(Callback::from(move |_: MouseEvent| cb.emit(step)))
                } else {
                    None
                };

                html! {
                    <div class={class} {onclick} data-tooltip={step.tooltip()}>
                        <span class="step-indicator">{indicator}</span>
                        <span class="step-label">{step.label()}</span>
                    </div>
                }
            })}

            // Action button
            if props.has_source && props.current_step != WizardStep::Run {
                <button class="wizard-action-btn"
                    onclick={
                        let cb = props.on_advance.clone();
                        Callback::from(move |_: MouseEvent| cb.emit(()))
                    }>
                    {props.current_step.action_label()}
                </button>
            }

            <div class="wizard-spacer"></div>
        </div>
    }
}
