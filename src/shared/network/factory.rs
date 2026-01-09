use anyhow::Result;
use clap::ValueEnum;
use crate::shared::network::{DataTransport, http_args::HttpArgs};
use super::http::HttpAdapter;

#[derive(Debug,ValueEnum,Clone)]
pub enum TransportType{
    Http,
}

pub fn handle_http(http_args:HttpArgs,transport_type:TransportType)-> Result<impl DataTransport>{
    match transport_type{
        TransportType::Http=>{
            HttpAdapter::new(http_args)
        }
    }
}