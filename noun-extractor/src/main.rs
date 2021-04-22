extern crate noun_extractor;

use clap::Clap;
use noun_extractor::model::State;
use std::io::{self, Read};

#[derive(Clap)]
#[clap(version = "1.0", author = "Eunchul Song. <ec.song@ejn.kr>")]
struct Opts {
    #[clap(subcommand)]
    subcmd: SubCommand,
    #[clap(about = "store path", default_value = "./noun-extractor-model")]
    store: String,
    #[clap(short, long)]
    verbose: bool,
}

#[derive(Clap)]
enum SubCommand {
    #[clap(about = "train with dataset")]
    Train(Train),
    #[clap(about = "extract nouns")]
    Extract(Extract),
    #[clap(about = "extract nouns v2(more passive)")]
    Extract2(Extract),
}

#[derive(Clap)]
struct Train {
    #[clap(about = "train dataset path")]
    input: String,
}

#[derive(Clap)]
struct Extract {
    #[clap(about = "input path")]
    input: Option<String>,
    #[clap(about = "output path(deafult stdout)")]
    output: Option<String>,
    #[clap(
        short,
        long,
        default_value = "0.9",
        about = "filter out noun candidates have probability below this threshold"
    )]
    prob_threshold: f32,
    #[clap(
        short,
        long,
        default_value = "10",
        about = "filter out noun candidates have count below this threshold"
    )]
    count_threshold: u32,
    #[clap(
        short,
        long,
        default_value = "5",
        about = "filter out noun candidates have unique suffixes count below this threshold"
    )]
    unique_suffixes_threshold: u32,
    /*#[clap(
        short,
        long,
        about = "online training with noun candidates have probability larger than 0.9"
    )]
    online: bool,*/
    #[clap(
        short,
        long,
        default_value = "1.0",
        about = "addictive smoothing factor"
    )]
    smooth_factor: f64,
}

fn main() -> anyhow::Result<()> {
    let opts: Opts = Opts::parse();
    let mut state = State::open(&opts.store)?;
    if opts.verbose {
        std::env::set_var("RUST_LOG", "debug");
    }
    env_logger::init();
    match opts.subcmd {
        SubCommand::Train(opts) => {
            state.train(&opts.input)?;
            state.save()?;
        }
        SubCommand::Extract2(opts) => {
            state.set_smooth_factor(opts.smooth_factor);
            let input = match &opts.input {
                Some(input) => std::fs::read_to_string(&input)?,
                None => {
                    let mut buf = String::new();
                    io::stdin().read_to_string(&mut buf)?;
                    buf
                }
            };
            let rows = state.extract_nouns2(&input)?.into_iter().filter(|r| {
                r.1.noun_probability >= opts.prob_threshold
                    && r.1.count >= opts.count_threshold
                    && r.1.unique_suffixes_hll.len() >= opts.unique_suffixes_threshold.into()
            });
            /*if opts.online {
                let ac = aho_corasick::AhoCorasick::new(
                    rows.iter()
                        .filter(|r| {
                            r.1.noun_probability >= 0.9
                                && r.1.unique_suffixes_count >= 4
                                && r.1.count >= 10
                        })
                        .map(|r| &r.0),
                );
                let matches = ac
                    .find_iter(&input)
                    .map(|mat| (mat.start(), mat.end() - mat.start()))
                    .collect::<Vec<_>>();
                //println!("matches: {:?}", matches);
                //&input.find(opts.smooth_factor);
                //state.train_line_bytes(&text, &noun_poses)?;
                //nl_str
            }*/
            let nl_str = rows
                .into_iter()
                .map(|r| {
                    format!(
                        "{}\t{}\t{}\t{}",
                        r.0,
                        r.1.noun_probability,
                        r.1.count,
                        r.1.unique_suffixes_hll.len()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");

            match opts.output {
                Some(output) => std::fs::write(output, nl_str)?,
                None => println!("{}", &nl_str),
            }
        }
        SubCommand::Extract(opts) => {
            state.set_smooth_factor(opts.smooth_factor);
            let input = match &opts.input {
                Some(input) => std::fs::read_to_string(&input)?,
                None => {
                    let mut buf = String::new();
                    io::stdin().read_to_string(&mut buf)?;
                    buf
                }
            };
            let rows = state.extract_nouns(&input)?.into_iter().filter(|r| {
                r.1.noun_probability >= opts.prob_threshold
                    && r.1.count >= opts.count_threshold
                    && r.1.unique_suffixes_hll.len() >= opts.unique_suffixes_threshold.into()
            });
            let nl_str = rows
                .into_iter()
                .map(|r| {
                    format!(
                        "{}\t{}\t{}\t{}",
                        r.0,
                        r.1.noun_probability,
                        r.1.count,
                        r.1.unique_suffixes_hll.len()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            match opts.output {
                Some(output) => std::fs::write(output, nl_str)?,
                None => println!("{}", &nl_str),
            }
        }
    }
    Ok(())
}
