use anyhow::Result;
use tracing::{error,trace,info};
use url::Url;
use tokio_stream::StreamExt;
use crate::shared::{fs::FsOps, network::http_args::HttpArgs};
use super::cli::LocalArgs;
use crate::shared::network::{factory::{TransportType,handle_http},DataTransport};
use crate::shared::fs::local::LocalFsBuilder;

pub async fn handle(url:Url,args:LocalArgs,http_args:HttpArgs)->Result<()>{
    let file_path=args.output;
    let file_parent_dir=file_path.ancestors().next().unwrap().to_path_buf();
    let transport=match args.transport{
        TransportType::Http=>{
            handle_http(http_args,TransportType::Http)
        }
    }?;
    let builder=LocalFsBuilder::new().path(file_path.clone()).root_path(file_parent_dir).build().await?;
    
    let stream_result=transport.receive_data(url.clone()).await;
    match stream_result{
        Ok( mut stream)=>{
            loop{
                
                match stream.try_next().await {
                    Ok(Some(bytes))=>{
                        let bytes_size=std::mem::size_of_val(&bytes);
                        trace!("Writing bytes of size {} to {file_path:?}",bytes_size);
                        builder.append_bytes(bytes).await?;

                    }

                    Ok(None)=>{
                        info!("Reach the EOF,streaming completed.");
                        builder.close_fs().await;
                        break;
                    }
                    Err(err)=>{
                        error!("Can't get next item on network stream,caused by:{err}");
                        builder.close_fs().await;
                        return Err(err.into());

                    }
                }
            }
            }   

        Err(err)=>{
            error!("Can't get {url} data, caused by:{err}");
        }

        }
        
    Ok(())
    }

