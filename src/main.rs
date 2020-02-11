use html5ever::tokenizer::{
  BufferQueue, Tag, TagKind, TagToken, Token, TokenSink, TokenSinkResult, Tokenizer,
  TokenizerOpts,
};
use std::borrow::Borrow;
use url::{ParseError, Url};

use async_std::task;
use surf;
use std::error::Error;
use std::io;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::io::prelude::*;


type CrawlResult = Result<(), Box<dyn std::error::Error + Send + Sync + 'static>>;
type BoxFuture = std::pin::Pin<Box<dyn std::future::Future<Output=CrawlResult> + Send>>;

#[derive(Default, Debug)]
struct Links {
  links: Vec<String>,
}

impl TokenSink for &mut Links {
  type Handle = ();

  // <a href="link">some text</a>
  fn process_token(&mut self, token: Token, line_number: u64) -> TokenSinkResult<Self::Handle> {

    match token {
      TagToken(
        ref tag @ Tag {
          kind: TagKind::StartTag,
          ..
        },
      ) => {
        println!("Je rentre dans processtoken");
        if tag.name.as_ref() == "a" {
          for attribute in tag.attrs.iter() {
            if attribute.name.local.as_ref() == "href" {
              let url_str: &[u8] = attribute.value.borrow();
              self.links
                .push(String::from_utf8_lossy(url_str).into_owned());
            }
          }
        }
      }
      _ => {}
    }
    TokenSinkResult::Continue
  }
}

pub fn get_links(url: &Url, page: String) -> Vec<Url> {
  let mut domain_url = url.clone();
  domain_url.set_path("");
  domain_url.set_query(None);

  let mut links = Links::default();
  let mut tokenizer = Tokenizer::new(&mut links, TokenizerOpts::default());
  let mut buffer = BufferQueue::new();
  buffer.push_back(page.into());
  let _ = tokenizer.feed(&mut buffer);

  links.links.iter().map(|link| match Url::parse(link) {
      Err(ParseError::RelativeUrlWithoutBase) => domain_url.join(link).unwrap(),
      Err(_) => panic!("Malformed link found: {}", link),
      Ok(url) => url,
    })
    .collect()
}

fn box_crawl(url: Url, current: u8, max: u8) -> BoxFuture {
  Box::pin(crawl(url, current, max))
}

async fn crawl(url: Url, current: u8, max: u8) -> CrawlResult {
  println!("Current Depth: {}, Max Depth: {}", current, max);
  if current > max {
    println!("Reached Max Depth");
    return Ok(());
  }
  let task = task::spawn(async move {
    let mut body = surf::get(&url).recv_string().await?;
    let links = get_links(&url, body);
    for link in links {
      write_link_in_file(&link);
    }
    box_crawl(url, current + 1, max).await
  });
  task.await?;
  Ok(())
}

fn write_link_in_file(url: &Url){
  let mut file = OpenOptions::new()
    .write(true)
    .append(true)
    .open("links.txt")
    .unwrap();
  if let Err(e) = writeln!(file, "{}", url.to_string()) {
    eprintln!("Couldn't write to file: {}", e);
  }
}

fn read_file() -> Result<Vec<String>, Box<dyn Error>> {
  let mut results = csv::Reader::from_reader(io::stdin());
  let mut urls: Vec<String> = Vec::new();
  for result in results.records() {
    let record = result?;
    let url = record[0].to_string();
    urls.push(url);
  }
  Ok(urls)
}

fn main() {
  let urls = read_file();
  for url in urls.unwrap() {
    task::block_on(async {
      box_crawl(Url::parse(&url).unwrap(), 1, 5).await
    });
  }
}