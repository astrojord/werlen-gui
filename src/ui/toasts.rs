//! short-lived messages in the bottom-right corner

use std::time::Duration;

use egui_material_icons::icons::{ICON_CHECK_CIRCLE, ICON_ERROR};

use super::style::{ERR_COLOR, ok_color, with_icon};
use super::{WerlenApp, thousands};
use crate::shop::Notice;

/// how long info toasts stay up, and how long of that they spend fading out
const TOAST_SECS: f64 = 4.0;
const TOAST_FADE_SECS: f64 = 0.6;

/// a short-lived message in the bottom-right corner; errors stay until dismissed
pub(super) struct Toast {
    text: String,
    error: bool,
    /// egui time it was created, in seconds
    created: f64,
}

impl WerlenApp {
    fn toast(&mut self, ctx: &egui::Context, text: String, error: bool) {
        let created = ctx.input(|i| i.time);
        self.toasts.push(Toast {
            text,
            error,
            created,
        });
    }

    pub(super) fn show_toasts(&mut self, ctx: &egui::Context) {
        if let Some(notice) = self.shop.notice.take() {
            let (text, error) = match notice {
                Notice::Imported(summary) => (
                    format!(
                        "Imported CSV — {} added, {} updated",
                        thousands(summary.added),
                        thousands(summary.updated)
                    ),
                    false,
                ),
                Notice::Error(msg) => (msg, true),
            };
            self.toast(ctx, text, error);
        }

        let now = ctx.input(|i| i.time);
        self.toasts
            .retain(|toast| toast.error || now - toast.created < TOAST_SECS);
        if self.toasts.is_empty() {
            return;
        }

        let mut dismissed = None;
        egui::Area::new(egui::Id::unique("toasts"))
            .anchor(egui::Align2::RIGHT_BOTTOM, [-12.0, -36.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                for (i, toast) in self.toasts.iter().enumerate() {
                    ui.scope(|ui| {
                        if !toast.error {
                            let left = TOAST_SECS - (now - toast.created);
                            ui.set_opacity((left / TOAST_FADE_SECS).clamp(0.0, 1.0) as f32);
                        }
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                if toast.error {
                                    ui.colored_label(ERR_COLOR, with_icon(ICON_ERROR, &toast.text));
                                    if ui.small_button("Dismiss").clicked() {
                                        dismissed = Some(i);
                                    }
                                } else {
                                    ui.colored_label(
                                        ok_color(ui.visuals().dark_mode),
                                        with_icon(ICON_CHECK_CIRCLE, &toast.text),
                                    );
                                }
                            });
                        });
                    });
                }
            });

        if let Some(i) = dismissed {
            self.toasts.remove(i);
        }
        if self.toasts.iter().any(|toast| !toast.error) {
            // keep animating the fade
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
}
