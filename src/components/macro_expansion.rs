//! Collapsible macro expansion view component.
//!
//! Displays preprocessed PL/SW source with annotated macro expansions.
//! Each `?MACRO(...)` invocation has a toggle to show/hide its GEN
//! block expansion.  Collapsed by default.

use yew::prelude::*;

use crate::preprocessor::{PreprocessResult, SourceLine};

#[derive(Properties, PartialEq)]
pub struct MacroExpansionViewProps {
    /// Preprocessor result containing source lines and expansions.
    pub result: PreprocessResult,
}

#[function_component(MacroExpansionView)]
pub fn macro_expansion_view(props: &MacroExpansionViewProps) -> Html {
    // Track which expansions are open (all collapsed by default)
    let open_set = use_state(std::collections::HashSet::<usize>::new);

    let toggle = {
        let open_set = open_set.clone();
        Callback::from(move |idx: usize| {
            let mut set = (*open_set).clone();
            if set.contains(&idx) {
                set.remove(&idx);
            } else {
                set.insert(idx);
            }
            open_set.set(set);
        })
    };

    let has_expansions = !props.result.expansions.is_empty();

    html! {
        <div class="macro-expansion-view">
            { for props.result.source_lines.iter().enumerate().map(|(line_num, line)| {
                match line {
                    SourceLine::Plain(text) => {
                        html! {
                            <div class="expansion-line" key={line_num}>
                                <span class="expansion-line-num">{line_num + 1}</span>
                                <span class="expansion-line-text">
                                    { highlight_plsw(text) }
                                </span>
                            </div>
                        }
                    }
                    SourceLine::Include(text) => {
                        html! {
                            <div class="expansion-line expansion-include" key={line_num}>
                                <span class="expansion-line-num">{line_num + 1}</span>
                                <span class="expansion-line-text">
                                    <span class="hl-directive">{text}</span>
                                </span>
                            </div>
                        }
                    }
                    SourceLine::Invocation { text, expansion_idx } => {
                        let idx = *expansion_idx;
                        let is_open = open_set.contains(&idx);
                        let expansion = &props.result.expansions[idx];
                        let toggle = toggle.clone();
                        let arrow = if is_open { "\u{25BC}" } else { "\u{25B6}" };

                        html! {
                            <div class="expansion-block" key={line_num}>
                                <div class="expansion-line expansion-invocation"
                                     onclick={Callback::from(move |_: MouseEvent| toggle.emit(idx))}>
                                    <span class="expansion-line-num">{line_num + 1}</span>
                                    <span class="expansion-toggle">{arrow}</span>
                                    <span class="expansion-line-text">
                                        <span class="hl-macro">{text}</span>
                                    </span>
                                    <span class="expansion-badge">
                                        {format!("{} lines", expansion.expanded_lines.len())}
                                    </span>
                                </div>
                                if is_open {
                                    <div class="expansion-content">
                                        { for expansion.expanded_lines.iter().map(|exp_line| {
                                            html! {
                                                <div class="expansion-gen-line">
                                                    <span class="expansion-gutter">{"\u{2502}"}</span>
                                                    <span class="expansion-gen-text">
                                                        { highlight_asm(exp_line) }
                                                    </span>
                                                </div>
                                            }
                                        })}
                                    </div>
                                }
                            </div>
                        }
                    }
                }
            })}
            if !has_expansions {
                <div class="expansion-no-macros">
                    {"No macro invocations found in source"}
                </div>
            }
        </div>
    }
}

/// Minimal PL/SW syntax highlighting for plain source lines.
fn highlight_plsw(line: &str) -> Html {
    // Simple: highlight comments, keywords, strings
    let trimmed = line.trim();

    if trimmed.starts_with("/*") {
        return html! { <span class="hl-comment">{line}</span> };
    }

    // Just return as plain text for now -- full highlighting is in the editor
    html! { <>{line}</> }
}

/// Assembly syntax highlighting for GEN expansion lines.
fn highlight_asm(line: &str) -> Html {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return html! { <>{line}</> };
    }

    // Comments (after ;)
    if let Some(semi) = trimmed.find(';') {
        let code = &trimmed[..semi];
        let comment = &trimmed[semi..];
        return html! {
            <>
                <span class="hl-asm-text">{code}</span>
                <span class="hl-comment">{comment}</span>
            </>
        };
    }

    html! { <span class="hl-asm-text">{line}</span> }
}
