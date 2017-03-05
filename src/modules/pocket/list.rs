use config::{Config, OutputFormat};
use net::{curl, HttpVerb};
use utils::console::*;
use utils::output;

use clap::{App, Arg, ArgMatches, SubCommand};
use serde_json;
use std::collections::HashMap;
use std::str;

pub const NAME: &'static str = "list";

static HEADERS: &'static [&'static str] = &["Content-Type: application/json"];

error_chain! {
    errors {
       PocketListFailed {
            description("failed to list Pocket articles")
            display("failed to list Pocket articles")
        }
    }
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Debug)]
enum State {
    unread,
    archive,
    all,
}

impl<'a> From<&'a str> for State {
    fn from(s: &'a str) -> Self {
        match s {
            "archive" => State::archive,
            "all" => State::all,
            _ => State::unread,
        }
    }
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Debug)]
enum Sort {
    newest,
    oldest,
    title,
    site,
}

impl<'a> From<&'a str> for Sort {
    fn from(s: &'a str) -> Self {
        match s {
            "oldest" => Sort::oldest,
            "title" => Sort::title,
            "site" => Sort::site,
            _ => Sort::newest
        }
    }
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Debug)]
enum DetailType {
    simple,
    complete,
}

impl From<bool> for DetailType {
    fn from(b: bool) -> Self {
        if b {
            DetailType::complete
        } else {
            DetailType::simple
        }
    }
}

#[allow(non_snake_case)]
#[derive(Serialize, Debug)]
struct Request<'a> {
    consumer_key: &'a str,
    access_token: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")] state: Option<State>,
    #[serde(skip_serializing_if = "Option::is_none")] tag: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")] sort: Option<Sort>,
    detailType: DetailType,
    #[serde(skip_serializing_if = "Option::is_none")] search: Option<&'a str>,
}

pub fn build_sub_cli() -> App<'static, 'static> {
    SubCommand::with_name(NAME)
        .about("List saved articles")
        .arg(Arg::with_name("details")
            .long("details")
            .short("d")
            .help("Select details for articles"))
        .arg(Arg::with_name("tag")
            .long("tag")
            .short("t")
            .takes_value(true)
            .help("Select articles tagged with <tag> to list"))
        .arg(Arg::with_name("state")
            .long("state")
            .short("s")
            .takes_value(true)
            .possible_values(&["unread", "archive", "all"])
            .default_value("unread")
            .help("Select articles to list"))
        .arg(Arg::with_name("sort")
            .long("sort")
            .takes_value(true)
            .possible_values(&["newest", "oldest", "title", "site"])
            .default_value("newest")
            .help("Select sort order"))
}

pub fn call(args: Option<&ArgMatches>, config: &Config) -> Result<()> {
    let args = args.unwrap();

    let state = Some(args.value_of("state").unwrap().into());
    let value = if args.is_present("tag") {
        Some(args.value_of("tag").unwrap())
    } else {
        None
    };
    let sort = Some(args.value_of("sort").unwrap().into());
    let detail_type = args.is_present("details").into();
    let search = if args.is_present("search") {
        Some(args.value_of("search").unwrap())
    } else {
        None
    };

    let request = Request {
        consumer_key: &config.pocket.consumer_key,
        access_token: &config.pocket.access_token.as_ref().unwrap(),
        state: state,
        tag: value,
        sort: sort,
        detailType: detail_type,
        search: search,
    };

    info(format!("Getting list of your articles ..."));
    let json = get(config, &request).chain_err(|| ErrorKind::PocketListFailed)?;

    output(&json, &config.general.output_format)
}

#[allow(unused_variables)] // for status codes
fn get(config: &Config, request: &Request) -> Result<String> {
    let mut buffer = Vec::new();
    let request_json = &serde_json::to_string(&request).chain_err(|| "JSON serialization failed")?.into_bytes();
    // TODO: Only continue if 200
    let response_status_code = curl(
        "https://getpocket.com/v3/get",
        HttpVerb::POST,
        Some(&HEADERS),
        Some(request_json),
        Some(&mut buffer)
    ).chain_err(|| "Curl failed")?;
    let response_str = str::from_utf8(&buffer).chain_err(|| "Data copying failed.")?;

    Ok(response_str.to_string())
}

fn output(json: &str, format: &OutputFormat) -> Result<()> {
    match *format {
        OutputFormat::HUMAN => output_human(json),
        OutputFormat::JSON  => output::as_json(json)
            .chain_err(|| ErrorKind::PocketListFailed),
    }
}

#[derive(Deserialize, Debug)]
struct ListResult {
    status: i32,
    complete: i32,
    list: HashMap<String, Article>,
}

#[derive(Deserialize, Debug)]
struct Article {
    item_id: String,
    resolved_title: String,
    resolved_url: String,
}

fn output_human(json: &str) -> Result<()> {
    let list: ListResult = serde_json::from_str(&json).chain_err(|| "JSON parsing failed")?;

    if list.status == 1 {
        msg(format!("Received {} articles.", list.list.values().len()));
    } else {
        msg("Receiving articles failed.");
    }
    for a in list.list.values() {
        msg(format!("{}: '{}', {}", a.item_id, a.resolved_title, a.resolved_url));
    }

    Ok(())
}
