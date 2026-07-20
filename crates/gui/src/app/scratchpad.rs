//! The scratchpad workspace: a free-form notes field for jotting down what
//! the user is computing. No numeric logic — plain text in, plain text out —
//! persisted across sessions via `App::save`/`App::new`.

use super::App;

impl App {
    pub(super) fn scratchpad_body(&mut self, ui: &mut egui::Ui) {
        // Captured before entering the scroll area, whose content region
        // reports an unbounded height rather than the tab's actual remaining
        // space. `add_sized` then stretches the field to fill it; content
        // taller than that still scrolls, via the area outside it.
        let available = ui.available_size();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_sized(
                    available,
                    egui::TextEdit::multiline(&mut self.scratchpad)
                        .font(egui::FontId::new(15.0, egui::FontFamily::Monospace))
                        .hint_text("Notes..."),
                );
            });
    }
}
