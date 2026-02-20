use terminal_miku3d::{app, cli};

fn main() -> anyhow::Result<()> {
    let cli = cli::parse();
    app::run(cli)
}
