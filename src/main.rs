mod gamebanana_async;
mod modmanagement;

slint::include_modules!();
// slint_build::compile

fn main() {
    let app = App::new().unwrap();
    let app_weak = app.as_weak();
    app.on_clicked(move || {
        let app = app_weak.upgrade().unwrap();
        app.set_counter(app.get_counter() + 1);
    });
    app.run().unwrap();
    println!("Starting Rust4Diva Slint Edition");
}