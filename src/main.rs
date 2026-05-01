use terminal_miku3d::{app, cli, runtime};

fn main() -> anyhow::Result<()> {
    runtime::app::setup_panic_hook();
    let cli = cli::parse();
    app::run(cli)
}
