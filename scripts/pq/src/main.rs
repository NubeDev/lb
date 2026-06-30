mod app;
mod args;
mod cli;
mod editor;
mod library;
mod macros;
mod output;
mod picker;
mod role;
mod template;

fn main() {
    if let Err(err) = app::run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}
