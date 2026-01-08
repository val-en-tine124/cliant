use anyhow::Result;

use crate::shared::network::{DataTransport, http_args::HttpArgs};
use super::http::HttpAdapter;


pub enum TransportType{
    Http(HttpArgs),
}

pub fn handle(transport_type:TransportType)-> Result<impl DataTransport>{
    match transport_type{
        TransportType::Http(args)=>{
            HttpAdapter::new(args)
        }
    }
}