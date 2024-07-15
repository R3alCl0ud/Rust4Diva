mod modmanagement;
mod gamebanana_async;

slint::slint! {
    import { Button, VerticalBox } from "std-widgets.slint";
    export component App inherits Window {
        width: 1280px;
        height: 720px;
        // background: #000000;
        title: "Rust4Diva: Project Diva MM+ Mod Manager";
        in property <int> counter: 4;
        callback clicked <=> btn.clicked;
        VerticalBox{
            Text { text: "Rust" + counter + "Diva"; }

            btn := Button { text: "Diva Button"; }

        GridLayout {
            padding: 20px;
            spacing: 10px;
            Row {
                Text { text: "Enabled";}
                Text { text: "Name";}
                Text { text: "Authors";}
                Text { text: "Version";}
                Text { text: "Description";}
            }
            Row {
                Text { text: "1";}
                Text { text: "2";}
                Text { text: "3";}
                Text { text: "4";}
                Text { text: "5";}

            }
        }
            }
    }
}
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