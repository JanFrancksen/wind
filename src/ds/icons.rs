use eframe::egui;

#[derive(Clone, Copy)]
#[cfg_attr(target_os = "macos", allow(dead_code))]
pub enum Icon {
    ArrowLeft,
    ArrowRight,
    ChevronDown,
    ChevronUp,
    Pin,
    Plus,
    Reload,
    X,
}

impl Icon {
    pub fn image(self, size: f32, color: egui::Color32) -> egui::Image<'static> {
        egui::Image::new(self.source())
            .fit_to_exact_size(egui::vec2(size, size))
            .tint(color)
    }

    fn source(self) -> egui::ImageSource<'static> {
        match self {
            Icon::ArrowLeft => egui::include_image!("../../assets/icons/tabler/arrow-left.svg"),
            Icon::ArrowRight => egui::include_image!("../../assets/icons/tabler/arrow-right.svg"),
            Icon::ChevronDown => {
                egui::include_image!("../../assets/icons/tabler/chevron-down.svg")
            }
            Icon::ChevronUp => egui::include_image!("../../assets/icons/tabler/chevron-up.svg"),
            Icon::Pin => egui::include_image!("../../assets/icons/tabler/pin.svg"),
            Icon::Plus => egui::include_image!("../../assets/icons/tabler/plus.svg"),
            Icon::Reload => egui::include_image!("../../assets/icons/tabler/reload.svg"),
            Icon::X => egui::include_image!("../../assets/icons/tabler/x.svg"),
        }
    }
}
