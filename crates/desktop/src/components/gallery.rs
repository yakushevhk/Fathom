//! Generative UI gallery components mirroring OpenBot (`app/src/components/gallery/`).
//! Provides high-fidelity rendering for RecordCard, MetricsCard, ChecklistCard, NoticeCard,
//! BarChartCard, ProgressChartCard, ChoiceCard, and RefusedCard.

use crate::theme::Theme;
use gpui::{
    div, prelude::*, px, ClickEvent, Context, Div, SharedString,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordField {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordCardData {
    pub title: String,
    pub subtitle: Option<String>,
    pub status: Option<String>,
    pub status_tone: Option<String>, // "positive" | "caution" | "negative" | "neutral"
    pub fields: Vec<RecordField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsCardData {
    pub title: String,
    pub value: String,
    pub unit: Option<String>,
    pub delta: Option<String>,
    pub delta_positive: Option<bool>,
    pub subtitle: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub label: String,
    pub checked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistCardData {
    pub title: String,
    pub items: Vec<ChecklistItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoticeCardData {
    pub title: String,
    pub message: String,
    pub tone: Option<String>, // "info" | "warning" | "error" | "success"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartPoint {
    pub label: String,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarChartData {
    pub title: String,
    pub caption: Option<String>,
    pub points: Vec<ChartPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressChartData {
    pub title: String,
    pub current: f64,
    pub total: f64,
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChoiceOption {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChoiceCardData {
    pub request_id: String,
    pub question: String,
    pub options: Vec<ChoiceOption>,
    pub selected: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefusedCardData {
    pub tool_name: String,
    pub rule_name: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmActionCardData {
    pub request_id: String,
    pub title: String,
    pub description: String,
    pub command_or_tool: String,
    pub risk_level: String, // "low" | "medium" | "high"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormParameterField {
    pub key: String,
    pub label: String,
    pub default_value: Option<String>,
    pub is_secret: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormParameterCardData {
    pub request_id: String,
    pub title: String,
    pub description: Option<String>,
    pub fields: Vec<FormParameterField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputerStatusCardData {
    pub agent_id: String,
    pub container_id: String,
    pub status: String, // "running" | "stopped" | "idle"
    pub current_url: Option<String>,
    pub memory_mb: Option<u64>,
}

/// Renders a ConfirmActionCard (Human-in-the-loop critical action confirmation)
pub fn render_confirm_action_card<V: 'static>(
    card: &ConfirmActionCardData,
    cx: &mut Context<V>,
    on_action: impl Fn(&str, bool, &mut V, &mut Context<V>) + 'static + Clone,
) -> Div {
    let req_id = card.request_id.clone();
    let req_id_clone = req_id.clone();
    let on_approve = on_action.clone();
    let on_deny = on_action;

    let risk_color = match card.risk_level.as_str() {
        "high" => Theme::danger_red(),
        "medium" => Theme::warning_yellow(),
        _ => Theme::accent_blue(),
    };

    div()
        .flex()
        .flex_col()
        .p_3()
        .rounded_lg()
        .bg(Theme::bg_card())
        .border_1()
        .border_color(risk_color)
        .gap_2p5()
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(Theme::text_primary())
                        .child(card.title.clone()),
                )
                .child(
                    div()
                        .px_2()
                        .py_0p5()
                        .rounded_sm()
                        .bg(Theme::bg_elevated())
                        .border_1()
                        .border_color(risk_color)
                        .text_xs()
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(risk_color)
                        .child(format!("Risk: {}", card.risk_level.to_uppercase())),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(Theme::text_secondary())
                .child(card.description.clone()),
        )
        .child(
            div()
                .p_2()
                .rounded_md()
                .bg(Theme::bg_elevated())
                .text_xs()
                .font_family("JetBrains Mono")
                .text_color(Theme::text_primary())
                .child(card.command_or_tool.clone()),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .pt_1()
                .child(
                    div()
                        .id("confirm-approve-btn")
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(Theme::accent_purple())
                        .hover(|s| s.bg(Theme::accent_blue()))
                        .text_xs()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(Theme::text_primary())
                        .cursor_pointer()
                        .child("Approve & Execute")
                        .on_click(cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                            on_approve(&req_id, true, this, cx);
                        })),
                )
                .child(
                    div()
                        .id("confirm-deny-btn")
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(Theme::bg_elevated())
                        .border_1()
                        .border_color(Theme::border_subtle())
                        .text_xs()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(Theme::danger_red())
                        .cursor_pointer()
                        .child("Deny Action")
                        .on_click(cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                            on_deny(&req_id_clone, false, this, cx);
                        })),
                ),
        )
}

/// Renders a ComputerStatusCard (Container / Sandbox runtime metrics)
pub fn render_computer_status_card(card: &ComputerStatusCardData) -> Div {
    let is_running = card.status == "running";
    let status_color = if is_running { Theme::success_green() } else { Theme::danger_red() };

    div()
        .flex()
        .flex_col()
        .p_3()
        .rounded_lg()
        .bg(Theme::bg_card())
        .border_1()
        .border_color(Theme::border_subtle())
        .gap_2()
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .size(px(8.0))
                                .rounded_full()
                                .bg(status_color),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(Theme::text_primary())
                                .child(format!("Computer Sandbox: {}", card.agent_id)),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .font_family("JetBrains Mono")
                        .text_color(Theme::text_muted())
                        .child(format!("ID: {}", card.container_id)),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .text_xs()
                .children(card.current_url.as_ref().map(|url| {
                    div()
                        .flex()
                        .gap_1()
                        .child(div().text_color(Theme::text_muted()).child("URL:"))
                        .child(div().text_color(Theme::accent_blue()).child(url.clone()))
                }))
                .children(card.memory_mb.as_ref().map(|mem| {
                    div()
                        .flex()
                        .gap_1()
                        .child(div().text_color(Theme::text_muted()).child("Memory:"))
                        .child(div().text_color(Theme::text_secondary()).child(format!("{} MB", mem)))
                })),
        )
}

/// Renders a structured RecordCard (key-value grid with status badge)
pub fn render_record_card(card: &RecordCardData) -> Div {
    let badge_color = match card.status_tone.as_deref() {
        Some("positive") => Theme::success_green(),
        Some("caution") => Theme::warning_yellow(),
        Some("negative") => Theme::danger_red(),
        _ => Theme::accent_blue(),
    };

    div()
        .flex()
        .flex_col()
        .p_3()
        .rounded_lg()
        .bg(Theme::bg_card())
        .border_1()
        .border_color(Theme::border_subtle())
        .gap_2()
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(Theme::text_primary())
                                .child(card.title.clone()),
                        )
                        .children(card.subtitle.as_ref().map(|sub| {
                            div()
                                .text_xs()
                                .text_color(Theme::text_muted())
                                .child(sub.clone())
                        })),
                )
                .children(card.status.as_ref().map(|st| {
                    div()
                        .px_2()
                        .py_0p5()
                        .rounded_md()
                        .bg(Theme::bg_elevated())
                        .border_1()
                        .border_color(badge_color)
                        .text_xs()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(badge_color)
                        .child(st.clone())
                })),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_3()
                .pt_2()
                .border_t_1()
                .border_color(Theme::border_subtle())
                .children(card.fields.iter().map(|field| {
                    div()
                        .flex()
                        .flex_col()
                        .min_w(px(120.0))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(Theme::text_muted())
                                .child(field.label.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .font_family("JetBrains Mono")
                                .text_color(Theme::text_primary())
                                .child(field.value.clone()),
                        )
                })),
        )
}

/// Renders a MetricsCard (headline metric with delta badge and trend indicator)
pub fn render_metrics_card(card: &MetricsCardData) -> Div {
    let delta_positive = card.delta_positive.unwrap_or(true);
    let delta_color = if delta_positive {
        Theme::success_green()
    } else {
        Theme::danger_red()
    };
    let delta_icon = if delta_positive { "↑" } else { "↓" };

    div()
        .flex()
        .flex_col()
        .p_3()
        .rounded_lg()
        .bg(Theme::bg_card())
        .border_1()
        .border_color(Theme::border_subtle())
        .gap_1()
        .child(
            div()
                .text_xs()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(Theme::text_muted())
                .child(card.title.clone()),
        )
        .child(
            div()
                .flex()
                .items_baseline()
                .gap_2()
                .child(
                    div()
                        .text_xl()
                        .font_weight(gpui::FontWeight::BOLD)
                        .font_family("JetBrains Mono")
                        .text_color(Theme::text_primary())
                        .child(card.value.clone()),
                )
                .children(card.unit.as_ref().map(|u| {
                    div()
                        .text_xs()
                        .text_color(Theme::text_muted())
                        .child(u.clone())
                }))
                .children(card.delta.as_ref().map(|d| {
                    div()
                        .flex()
                        .items_center()
                        .gap_0p5()
                        .px_1p5()
                        .py_0p5()
                        .rounded_sm()
                        .bg(Theme::bg_elevated())
                        .text_color(delta_color)
                        .child(format!("{} {}", delta_icon, d))
                })),
        )
        .children(card.subtitle.as_ref().map(|sub| {
            div()
                .text_xs()
                .text_color(Theme::text_muted())
                .child(sub.clone())
        }))
}

/// Renders a ChecklistCard with interactive/progress check marks
pub fn render_checklist_card(card: &ChecklistCardData) -> Div {
    let total = card.items.len();
    let completed = card.items.iter().filter(|i| i.checked).count();

    div()
        .flex()
        .flex_col()
        .p_3()
        .rounded_lg()
        .bg(Theme::bg_card())
        .border_1()
        .border_color(Theme::border_subtle())
        .gap_2()
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(Theme::text_primary())
                        .child(card.title.clone()),
                )
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(Theme::text_muted())
                        .child(format!("{}/{} completed", completed, total)),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1p5()
                .children(card.items.iter().map(|item| {
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_xs()
                        .child(
                            div()
                                .size(px(14.0))
                                .rounded_sm()
                                .bg(if item.checked { Theme::accent_purple() } else { Theme::bg_elevated() })
                                .border_1()
                                .border_color(if item.checked { Theme::accent_purple() } else { Theme::border_subtle() })
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(Theme::text_primary())
                                .child(if item.checked { "✓" } else { "" }),
                        )
                        .child(
                            div()
                                .text_color(if item.checked { Theme::text_muted() } else { Theme::text_primary() })
                                .child(item.label.clone()),
                        )
                })),
        )
}

/// Renders a NoticeCard with alert tone
pub fn render_notice_card(card: &NoticeCardData) -> Div {
    let (icon, border_color, text_color) = match card.tone.as_deref() {
        Some("warning") => ("⚠️", Theme::warning_yellow(), Theme::warning_yellow()),
        Some("error") => ("🛑", Theme::danger_red(), Theme::danger_red()),
        Some("success") => ("✓", Theme::success_green(), Theme::success_green()),
        _ => ("ℹ️", Theme::accent_blue(), Theme::accent_blue()),
    };

    div()
        .flex()
        .items_start()
        .gap_2p5()
        .p_3()
        .rounded_lg()
        .bg(Theme::bg_elevated())
        .border_1()
        .border_color(border_color)
        .child(
            div()
                .text_sm()
                .child(icon),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(text_color)
                        .child(card.title.clone()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(Theme::text_secondary())
                        .child(card.message.clone()),
                ),
        )
}

/// Renders a BarChartCard with proportional horizontal bars
pub fn render_barchart_card(card: &BarChartData) -> Div {
    let max_val = card.points.iter().map(|p| p.value).fold(1.0, f64::max);

    div()
        .flex()
        .flex_col()
        .p_3()
        .rounded_lg()
        .bg(Theme::bg_card())
        .border_1()
        .border_color(Theme::border_subtle())
        .gap_2()
        .child(
            div()
                .text_xs()
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(Theme::text_primary())
                .child(card.title.clone()),
        )
        .children(card.caption.as_ref().map(|cap| {
            div()
                .text_xs()
                .text_color(Theme::text_muted())
                .child(cap.clone())
        }))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .pt_2()
                .children(card.points.iter().map(|pt| {
                    let pct = ((pt.value / max_val) * 100.0).clamp(5.0, 100.0);
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .text_xs()
                                .child(
                                    div()
                                        .text_color(Theme::text_secondary())
                                        .child(pt.label.clone()),
                                )
                                .child(
                                    div()
                                        .font_family("JetBrains Mono")
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(Theme::text_primary())
                                        .child(format!("{:.1}", pt.value)),
                                ),
                        )
                        .child(
                            div()
                                .h(px(6.0))
                                .w_full()
                                .rounded_full()
                                .bg(Theme::bg_elevated())
                                .child(
                                    div()
                                        .h_full()
                                        .w(px(pct as f32 * 2.5))
                                        .rounded_full()
                                        .bg(Theme::accent_purple()),
                                ),
                        )
                })),
        )
}

/// Renders a ProgressChartCard (linear progress bar with fraction)
pub fn render_progress_card(card: &ProgressChartData) -> Div {
    let pct = if card.total > 0.0 {
        ((card.current / card.total) * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };

    div()
        .flex()
        .flex_col()
        .p_3()
        .rounded_lg()
        .bg(Theme::bg_card())
        .border_1()
        .border_color(Theme::border_subtle())
        .gap_2()
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(Theme::text_primary())
                        .child(card.title.clone()),
                )
                .child(
                    div()
                        .text_xs()
                        .font_family("JetBrains Mono")
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(Theme::accent_purple())
                        .child(format!("{:.0}%", pct)),
                ),
        )
        .child(
            div()
                .h(px(8.0))
                .w_full()
                .rounded_full()
                .bg(Theme::bg_elevated())
                .child(
                    div()
                        .h_full()
                        .w(px(pct as f32 * 3.0))
                        .rounded_full()
                        .bg(Theme::accent_purple()),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(Theme::text_muted())
                .child(format!(
                    "{:.1} of {:.1} {}",
                    card.current,
                    card.total,
                    card.unit.as_deref().unwrap_or("")
                )),
        )
}

/// Renders a RefusedCard for policy violations (Blocked: rule name and reason)
pub fn render_refused_card(card: &RefusedCardData) -> Div {
    div()
        .flex()
        .flex_col()
        .p_3()
        .rounded_lg()
        .bg(Theme::bg_elevated())
        .border_1()
        .border_color(Theme::danger_red())
        .gap_1p5()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .px_2()
                        .py_0p5()
                        .rounded_sm()
                        .bg(Theme::danger_red())
                        .text_xs()
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(Theme::text_primary())
                        .child("BLOCKED BY POLICY"),
                )
                .child(
                    div()
                        .text_xs()
                        .font_family("JetBrains Mono")
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(Theme::text_primary())
                        .child(card.tool_name.clone()),
                ),
        )
        .child(
            div()
                .text_xs()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(Theme::warning_yellow())
                .child(format!("Rule Matched: {}", card.rule_name)),
        )
        .child(
            div()
                .text_xs()
                .text_color(Theme::text_secondary())
                .child(card.reason.clone()),
        )
}

/// Renders interactive ChoiceCard allowing operator to select from multiple options
pub fn render_choice_card<V: 'static>(
    card: &ChoiceCardData,
    cx: &mut Context<V>,
    on_select: impl Fn(&str, &str, &mut V, &mut Context<V>) + 'static + Clone,
) -> Div {
    let req_id = card.request_id.clone();

    div()
        .flex()
        .flex_col()
        .p_3()
        .rounded_lg()
        .bg(Theme::bg_card())
        .border_1()
        .border_color(Theme::border_focus())
        .gap_2()
        .child(
            div()
                .text_xs()
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(Theme::accent_purple())
                .child(format!("Decision Required: {}", card.question)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1p5()
                .children(card.options.iter().map(|opt| {
                    let opt_id = opt.id.clone();
                    let is_selected = card.selected.as_deref() == Some(&opt.id);
                    let cb = on_select.clone();
                    let r_id = req_id.clone();
                    let o_id = opt_id.clone();

                    div()
                        .id(SharedString::from(format!("choice-opt-{}-{}", req_id, opt.id)))
                        .flex()
                        .items_center()
                        .justify_between()
                        .p_2()
                        .rounded_md()
                        .bg(if is_selected { Theme::bg_elevated_hover() } else { Theme::bg_elevated() })
                        .border_1()
                        .border_color(if is_selected { Theme::accent_purple() } else { Theme::border_subtle() })
                        .cursor_pointer()
                        .hover(|s| s.bg(Theme::bg_elevated_hover()))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(Theme::text_primary())
                                        .child(opt.label.clone()),
                                )
                                .children(opt.description.as_ref().map(|desc| {
                                    div()
                                        .text_xs()
                                        .text_color(Theme::text_muted())
                                        .child(desc.clone())
                                })),
                        )
                        .child(
                            div()
                                .size(px(14.0))
                                .rounded_full()
                                .border_1()
                                .border_color(if is_selected { Theme::accent_purple() } else { Theme::border_subtle() })
                                .bg(if is_selected { Theme::accent_purple() } else { Theme::bg_window() }),
                        )
                        .on_click(cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                            cb(&r_id, &o_id, this, cx);
                        }))
                })),
        )
}
