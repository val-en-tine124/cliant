use std::path::PathBuf;
use url::Url;
use clap::{Parser,command,arg};
use crate::shared::network::{http::config::HttpArgs,factory::TransportType};

#[derive(Clone,Parser)]
pub struct LocalArgs{
    ///Http url of file or to download. 
    #[arg(short='u',value_parser=parse_url)]
    pub url:Url,
    ///Path to save download.
    #[arg(short='o',value_parser=parse_output_path)]
    pub output:PathBuf,
    #[command(flatten)]
    pub http_args:HttpArgs,
    ///Transport to use for send and recieving data. It can be http/https,http-over-tor,bit torrent.
    #[arg(short='t',long,value_enum,default_value_t=TransportType::Http)]
    pub transport:TransportType,
}
///Perform path validation with this function,if path is a dir,
/// this function will throw an Err,else it will return a string
fn parse_output_path(path:&str)->Result<PathBuf,String>{
    let to_path=PathBuf::from(path);
    if !to_path.is_file(){
        return Err("Provided path is likely a directory and not a path to a file.".into());
    }
    Ok(to_path)

}

///This method takes a url as a string literal,checks and validate http
/// scheme in the url,parses it and return a Result Url or String
/// type if any error occur.
fn parse_url(url: &str) -> Result<Url, String> {
    if url.starts_with("https://") || url.starts_with("http://") {
        let parsed_url =
            Url::parse(url).map_err(|e| format!("Invalid Url {url} {e}"));
        return parsed_url;
    }
    let new_url = format!("https://{url}");
    Url::parse(&new_url).map_err(|e| format!("Invalid Url {url} {e}"))
}