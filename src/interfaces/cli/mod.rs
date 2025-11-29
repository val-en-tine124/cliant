use clap::{Parser, arg, command,};
use std::path::PathBuf;
use url::Url;

#[derive(Parser)]
#[command(version,about,long_about=None)]
pub struct Cliant {
    #[arg(value_parser=parse_url)]
    pub url: Vec<Url>,
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    #[arg(short = 'H', long)]
    pub home_dir: Option<PathBuf>,
    #[arg(short = 't', long)]
    pub timeout: Option<usize>,
    #[arg(short = 'r', long)]
    pub max_no_retries: Option<usize>,
    #[arg(short = 'd', long)]
    pub retry_delay_secs: Option<usize>,

}

fn parse_url(url:&str)->Result<Url,String>{
    if url.starts_with("https://") || url.starts_with("http://"){
        let parsed_url=Url::parse(url).map_err(|e|format!("Invalid Url {url} {e}"));
        return parsed_url;
    }
    let new_url=format!("https://{url}");
    let parsed_url=Url::parse(&new_url).map_err(|e|format!("Invalid Url {url} {e}"));
    parsed_url
}
