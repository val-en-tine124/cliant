use std::path::PathBuf;
use url::Url;
use clap::{Parser,command,arg};
use crate::shared::network::{http::config::HttpArgs,factory::TransportType};

#[derive(Clone,Parser)]
pub struct LocalArgs{
    ///Http url of file or to download. 
    #[arg(short='u')]
    url:Url,
    ///Path to save download.
    #[arg(short='o',)]
    pub output:PathBuf,
    #[command(flatten)]
    pub http_args:HttpArgs,
    ///Transport to use for send and recieving data. It can be http/https,http-over-tor,bit torrent.
    #[arg(short='t',long,value_enum,default_value_t=TransportType::Http)]
    pub transport:TransportType,
}
