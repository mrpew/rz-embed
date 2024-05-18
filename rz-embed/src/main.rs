use anyhow::Result;
use clap::Parser;
use log::LevelFilter;
use rz_embed::{calculate_compression_rate, compress_resources, generate_code, ResourceFile};
use std::{fs::File, io::Write, path::PathBuf};

#[derive(Parser)]
struct Args {
    input: PathBuf,
    output: PathBuf,

    #[arg(short, long)]
    verbose: bool,
}

fn init_logger(verbose: bool) {
    let mut builder = env_logger::Builder::from_default_env();

    if verbose {
        builder.filter(Some("rz-embed"), LevelFilter::Debug);
    } else {
        builder.filter(None, LevelFilter::Info);
    }

    builder.init();
}

fn main() -> Result<()> {
    let args = Args::parse();
    init_logger(args.verbose);

    let input = args.input;
    let output = args.output;
    //
    let resources = ResourceFile::collect(&input);
    log::info!("Collected {} resource files", resources.len());
    let code_file = {
        let mut path = output.clone();
        path.push("rz_embed.rs");
        path
    };
    let code = generate_code(&output, &resources).expect("failed to generate code");

    let (original, compressed) = compress_resources(&input, &output, resources)?;
    let rate = calculate_compression_rate(original, compressed);
    log::info!("Compressed resources: {original} -> {compressed} ({rate:.2}%)");

    let mut code_file = File::create(code_file)?;
    code_file.write_all(code.as_bytes())?;

    Ok(())
}
