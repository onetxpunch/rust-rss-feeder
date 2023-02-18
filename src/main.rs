use chrono::prelude::*;
use clap::Parser;
use rss::{ChannelBuilder, Item, ItemBuilder};
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// path to episodes
    #[arg(short, long, default_value = "serve")]
    filepath: String,

    /// title of feed
    #[arg(short, long, default_value = "a default title")]
    title: String,

    /// domain to feed
    #[arg(short, long, default_value = "example.com")]
    domain: String,

    /// description
    #[arg(short, long, default_value = "a default description")]
    subdesc: String,

    /// address to bind
    #[arg(short, long, default_value = "127.0.0.1")]
    bind: String,

    /// port
    #[arg(short, long, default_value = "8080")]
    port: u16,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();
    let directory = Path::new(&args.filepath);
    let mut entries: Vec<(SystemTime, Item)> = directory
        .read_dir()?
        .enumerate()
        .filter_map(|(index, entry)| {
            let res_entry = entry.ok()?;
            let path = res_entry.path();
            let file_name = path.file_name()?.to_str()?.to_owned();
            let ext = path.extension()?.to_str()?.to_owned();
            let mut file = std::io::BufReader::new(File::open(&path).ok()?);
            let mut contents = Vec::new();
            file.read_to_end(&mut contents).ok()?;
            let metadata = path.metadata().ok()?;
            let created = metadata.created().ok()?;
            let datetime: DateTime<Utc> = created.into();
            let item = ItemBuilder::default()
                .title(Some(file_name.trim_end_matches(&ext).to_owned()))
                .link(Some(format!("{}/{}", args.domain, file_name)))
                .description(Some(format!("File {}. {}", index, args.subdesc)))
                .pub_date(Some(datetime.to_string()))
                .build();
            Some((created, item))
        })
        .collect();
    entries.sort_by_key(|&(created, _)| std::cmp::Reverse(created));

    let channel = ChannelBuilder::default()
        .title(args.title)
        .link(args.domain)
        .description(args.subdesc)
        .items(
            entries
                .into_iter()
                .map(|(_, item)| item)
                .collect::<Vec<_>>(),
        )
        .build();

    let addr = format!("{}:{}", args.bind, args.port);
    println!("Listening on: {}", addr);
    let listener = TcpListener::bind(addr).await?;
    let xml = Arc::new(channel.to_string());
    loop {
        let xml = Arc::clone(&xml.to_owned());
        let (mut socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/rss+xml\r\nContent-Length: {}\r\n\r\n{}",
                xml.len(),
                xml
            );
            if let Err(e) = socket.write_all(response.as_bytes()).await {
                eprintln!("failed to write to socket; err = {:?}", e);
            }
        });
    }
}
