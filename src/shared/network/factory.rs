use anyhow::Result;
use clap::ValueEnum;
use crate::shared::network::{DataTransport, http::config::HttpArgs};
use super::http::HttpAdapter;

#[derive(Debug,ValueEnum,Clone)]
pub enum TransportType{
    #[cfg(feature="local")]
    Http,
}

pub fn handle_http(http_args:HttpArgs,transport_type:&TransportType)-> Result<impl DataTransport>{
    match transport_type{
        #[cfg(feature="local")]
        TransportType::Http=>{
            HttpAdapter::new(http_args)
        }
    }
}