use slint::Global;

slint::include_modules!();

mod app_ui;
mod config;
mod file_types;
mod processing;

fn main() -> Result<(), slint::PlatformError> {
    let app = AppWindow::new()?;
    Palette::get(&app).set_color_scheme(slint::language::ColorScheme::Dark);

    app_ui::install(&app);
    app.run()
}
