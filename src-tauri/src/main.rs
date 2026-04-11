use clap::Parser;

#[derive(Parser)]
#[command(name = "noru", version, about = "Local-first meeting capture and transcription")]
struct Cli {
    #[arg(long, hide = true)]
    cli: bool,
}

fn main() {
    let _ = Cli::parse();
    noru_lib::run();
}
