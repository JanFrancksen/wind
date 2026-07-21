use eframe::egui;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Icon {
    ArrowLeft,
    ArrowRight,
    Check,
    #[cfg(not(target_os = "macos"))]
    ChevronDown,
    #[cfg(not(target_os = "macos"))]
    ChevronUp,
    Copy,
    #[cfg(not(target_os = "macos"))]
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
            Icon::Check => egui::include_image!("../../assets/icons/tabler/check.svg"),
            #[cfg(not(target_os = "macos"))]
            Icon::ChevronDown => {
                egui::include_image!("../../assets/icons/tabler/chevron-down.svg")
            }
            #[cfg(not(target_os = "macos"))]
            Icon::ChevronUp => egui::include_image!("../../assets/icons/tabler/chevron-up.svg"),
            Icon::Copy => egui::include_image!("../../assets/icons/tabler/copy.svg"),
            #[cfg(not(target_os = "macos"))]
            Icon::Pin => egui::include_image!("../../assets/icons/tabler/pin.svg"),
            Icon::Plus => egui::include_image!("../../assets/icons/tabler/plus.svg"),
            Icon::Reload => egui::include_image!("../../assets/icons/tabler/reload.svg"),
            Icon::X => egui::include_image!("../../assets/icons/tabler/x.svg"),
        }
    }
}
