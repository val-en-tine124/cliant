use std::path::{Component, PathBuf};
use url::Url;
use path_clean::PathClean;
use clap::{Parser,command,arg};
use crate::shared::network::{http::config::HttpArgs,factory::TransportType};

#[derive(Clone,Debug,Parser)]
pub struct LocalArgs{
    ///Http url of file or to download. 
    #[arg(value_parser=parse_url,)]
    pub url:Url,
    ///Path to save download.
    #[arg(short='o',value_parser=parse_output_path)]
    pub output:PathBuf,
    #[command(flatten)]
    pub http_args:HttpArgs,
    ///Transport to use for send and recieving data. It can be http/https.
    #[arg(short='t',long,value_enum,default_value_t=TransportType::Http)]
    pub transport:TransportType,
}
///Perform path validation with this function,if path is a dir,
/// this function will throw an Err,else it will return a string.
/// 
/// This function should expand and return the absolute path of the file.
fn parse_output_path(path:&str)->Result<PathBuf,String>{
    let exp_path=shellexpand::tilde(path); // handle edge case of ~ in file path
    let to_path=PathBuf::from(exp_path.as_ref());
    //1. Must not end in a seperator (explicit directory)
    //2. Once normalized (cleaned of ".." and "."), the last part must be a filename.
    if !to_path.to_string_lossy().ends_with(std::path::is_separator) && matches!(to_path.clean().components().next_back(),Some(Component::Normal(_))){
        
        return Ok(to_path);    
    }
    
    Err("Provided path is likely a directory and not a path to a file.".into())


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