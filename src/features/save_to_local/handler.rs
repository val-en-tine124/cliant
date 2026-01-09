use anyhow::Result;
use crate::shared::network::http_args::HttpArgs;
use super::cli::LocalArgs;
use crate::shared::network::factory::{TransportType,handle_http};
use crate::shared::fs::local::{LocalFs,LocalFsBuilder};

pub async fn handle(args:LocalArgs,http_args:HttpArgs)->Result<()>{
    let file_path=args.output;
    let file_parent_dir=file_path.ancestors().next().unwrap().to_path_buf();
    let transport=match args.transport{
        TransportType::Http=>{
            handle_http(http_args,TransportType::Http)
        }
    };
    let builder=LocalFsBuilder::new().path(file_path).root_path(file_parent_dir).build().await?;
    
    Ok(())

}