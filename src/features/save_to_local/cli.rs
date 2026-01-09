use std::path::PathBuf;
use clap::{Parser,command,arg};
use crate::shared::network::{http_args::HttpArgs,factory::TransportType};

#[derive(Clone,Parser)]
pub struct LocalArgs{
    #[arg(short='o',)]
    pub output:PathBuf,
    #[command(flatten)]
    pub http_args:HttpArgs,
    #[arg(short='t',long,value_enum,default_value_t=TransportType::Http)]
    pub transport:TransportType,
}
