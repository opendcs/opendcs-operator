//use futures::{StreamExt, TryStreamExt};
use kube::Client;
//, PostParams}};

use std::error::Error;
use std::fs::File;

mod api;
mod lrgs;

use lrgs::lrgs::{create_ddsrecv_conf, create_drgsrecv_conf, create_password_file};

use clap::Parser;

#[derive(Parser)]
struct Cli {
    /// The path to write output to
    #[arg(short, long, default_value = "./")]
    conf_dir: std::path::PathBuf,
    #[arg(short, long,)]
    lrgs_service: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::parse();
    // Infer the runtime environment and try to create a Kubernetes Client
    let client = Client::try_default().await?;

    let file = File::create(args.conf_dir.join("ddsrecv.conf"))?;
    create_ddsrecv_conf(client.clone(), file, &args.lrgs_service).await?;

    let file = File::create(args.conf_dir.join("drgsrecv.conf"))?;
    create_drgsrecv_conf(client.clone(), file).await?;

    let pw_file = File::create(args.conf_dir.join(".lrgs.passwd"))?;
    create_password_file(client.clone(), pw_file).await?;

    Ok(())
}

