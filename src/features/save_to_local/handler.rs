use anyhow::{Context, Result};
use tracing::{debug, error, info, instrument, trace};
use tokio_stream::StreamExt;
use crate::shared::fs::FsOps;
use super::cli::LocalArgs;
use crate::shared::network::{factory::{TransportType,handle_http},DataTransport};
use crate::shared::fs::local::LocalFsBuilder;
use crate::shared::progress_tracker::{CliProgressTracker,ProgressTracker};

#[instrument(name="handle_http_download",fields(args))]
pub async fn handle(args:LocalArgs,)->Result<()>{
    let file_path=args.output;
    let url=args.url;
    let http_args=args.http_args;
    let file_parent_dir=file_path.parent().context(format!("Can't determine parent directory of:{}",file_path.clone().display()))?.to_path_buf();
    debug!("File path is {:?}",file_path.clone());
    debug!("File parent directory is: {:?}",file_parent_dir.clone());
    let transport=match args.transport{
        TransportType::Http=>{
            handle_http(http_args,&TransportType::Http)
        }
    }?;

    let builder=LocalFsBuilder::new().path(file_path.clone()).root_path(file_parent_dir).build().await?;
    
    let stream_result=transport.receive_data(url.clone()).await;
    let total_bytes=transport.total_bytes(url.clone()).await?;
    let tracker=CliProgressTracker::new(total_bytes,file_path.clone())?;
    match stream_result{
        Ok( mut stream)=>{
            info!("Starting streaming...");
            while let Some(bytes) =stream.try_next().await? {
                let bytes_size=bytes.len();
                trace!("Writing bytes of size {} to {file_path:?}",bytes_size);                
                builder.append_bytes(bytes).await?;
                tracker.update(bytes_size).await;
            }
            
            info!("Reached the EOF,streaming completed.");
            builder.close_fs().await;
    
            }   

        Err(err)=>{
            error!("Can't get {url} data, caused by:{err}");
        }

        }
     tracker.finish().await;
     
    Ok(())
    }

#[tokio::test]
///Replace this test in the future.
async fn test_handle()->anyhow::Result<()>{
    use crate::shared::network::http::config::HttpArgs;
    use anyhow::Context;
    let link=url::Url::parse("http://localhost:8000/Python_Datascience.pdf")?;
    let dwnld_path=dirs::download_dir().context("Can't get download dir")?.join("Python_Datascience.pdf");
    handle(LocalArgs{url:link,http_args:HttpArgs::default(),output:dwnld_path,transport:TransportType::Http}).await?;
    
    Ok(())
}