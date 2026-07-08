use eframe::egui;

#[derive(Clone)]
struct Tab {
    title: String,
    url: String,
}

struct BrowserApp {
    tabs: Vec<Tab>,
    active_tab: usize,
    address_input: String,
}

impl Default for BrowserApp {
    fn default() -> Self {
        Self {
            tabs: vec![Tab {
                title: "New Tab".to_string(),
                url: "https://example.com".to_string(),
            }],
            active_tab: 0,
            address_input: "https://example.com".to_string(),
        }
    }
}

impl eframe::App for BrowserApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::left("sidebar")
            .resizable(true)
            .default_width(240.0)
            .show(ctx, |ui| {
                ui.heading("Arc Browser");

                if ui.button("+ New Tab").clicked() {
                    self.tabs.push(Tab {
                        title: "New Tab".to_string(),
                        url: "https://example.com".to_string(),
                    });

                    self.active_tab = self.tabs.len() - 1;
                    self.address_input = self.tabs[self.active_tab].url.clone();
                }

                ui.separator();

                let mut close_tab: Option<usize> = None;

                for index in 0..self.tabs.len() {
                    ui.horizontal(|ui| {
                        let selected = self.active_tab == index;

                        if ui
                            .selectable_label(selected, &self.tabs[index].title)
                            .clicked()
                        {
                            self.active_tab = index;
                            self.address_input = self.tabs[index].url.clone();
                        }

                        if ui.button("×").clicked() {
                            close_tab = Some(index);
                        }
                    });
                }

                if let Some(index) = close_tab {
                    self.tabs.remove(index);

                    if self.tabs.is_empty() {
                        self.tabs.push(Tab {
                            title: "New Tab".to_string(),
                            url: "https://example.com".to_string(),
                        });
                    }

                    self.active_tab = self.active_tab.min(self.tabs.len() - 1);
                    self.address_input = self.tabs[self.active_tab].url.clone();
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                let response = ui.text_edit_singleline(&mut self.address_input);

                let pressed_enter = response.lost_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter));

                if ui.button("Go").clicked() || pressed_enter {
                    let url = normalize_url(&self.address_input);

                    self.tabs[self.active_tab].url = url.clone();
                    self.tabs[self.active_tab].title = url.clone();
                    self.address_input = url;
                }
            });

            ui.separator();

            let active = &self.tabs[self.active_tab];

            ui.heading(&active.title);
            ui.label(format!("Current URL: {}", active.url));

            ui.add_space(40.0);
            ui.label("CEF will render the webpage here later.");
        });
    }
}

fn normalize_url(input: &str) -> String {
    let trimmed = input.trim();

    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else if trimmed.contains('.') {
        format!("https://{}", trimmed)
    } else {
        format!("https://www.google.com/search?q={}", trimmed.replace(' ', "+"))
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();

    eframe::run_native(
        "Arc Browser Prototype",
        options,
        Box::new(|_cc| Ok(Box::new(BrowserApp::default()))),
    )
}