use crate::common::errors::FFResult;
use serde::{Deserialize, Serialize};
use serde_json::Error;
use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;
use std::time::{Duration, SystemTime};
use reqwest::blocking::{Client, Response};
use reqwest::header;
use reqwest::header::HeaderMap;

pub type Port = u16;
pub enum IpQuery {
    Success(IpQuerySuccess),
    Fail(IpQueryFail)
}
#[derive(Serialize, Deserialize)]
pub struct IpQuerySuccess {
    pub query: String,
    pub status: String,
    pub country: String,
    #[serde(rename = "countryCode")]
    pub country_code: String,
    pub region: String,
    #[serde(rename = "regionName")]
    pub region_name: String,
    pub city: String,
    pub zip: String,
    pub lat: f64,
    pub lon: f64,
    pub timezone: String,
    pub isp: String,
    pub org: String,
    #[serde(rename = "as")]
    pub autonomous_system: String,
    pub mobile: bool,
    pub proxy: bool,
    pub hosting: bool,
    #[serde(skip, default)]
    pub req_time: Option<SystemTime>
}

#[derive(Serialize, Deserialize)]
struct IpQueryFail {
    pub query: String,
    pub message: String,
    pub status: String,
}

impl IpQuery {
    pub fn query(mut ip: &str) -> FFResult<IpQuery> {
        if let Some(pos) = ip.find(":") {
            ip = ip.split_at(pos).0;
        }
        let query = format!("http://ip-api.com/json/{}?fields=17035263", ip);
        let mut res = reqwest::blocking::get(query)?;
        let mut body = String::new();
        let _ = res.read_to_string(&mut body)?;
        let yes: Result<IpQuerySuccess, Error> = serde_json::from_str(&body);
        let no: Result<IpQueryFail, Error> = serde_json::from_str(&body);
        match yes {
            Ok(mut it) => {
                it.req_time = Some(SystemTime::now());
                Ok(IpQuery::Success(it))
            }

            Err(it) => {
                match no {
                    Ok(it) => {Ok(IpQuery::Fail(it))}
                    Err(_) => {Err(Box::new(it))}
                }
            }
        }
    }
    pub fn needs_retry(&self) -> bool {
        match self {
            IpQuery::Success(it) => {
                false
            }
            IpQuery::Fail(it) => {
                if it.message.eq_ignore_ascii_case("private range") {
                    false
                } else {
                    true
                }
            }
        }
    }
    pub fn to_normal_name(&self) -> String {
        match self {
            IpQuery::Success(it) => {
                format!("{}, {}, {}", it.city, it.region, it.country_code)
            }
            IpQuery::Fail(it) => {
                if it.message.eq_ignore_ascii_case("private range") {
                    "private range".to_string()
                } else {
                    format!("Ip Request Error: {}", it.message)
                }
            }
        }
    }
}
pub fn get_routable_address() -> (Option<Ipv6Addr>, Option<Ipv4Addr>) {
    //Ok, because of how mobile hotspots work, we have to prefer ipv6 connections, I think
    //Use a default user agent for privacy
    //let host = "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/111.0.0.0 Mobile Safari/537.36";
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(2))
        .build().unwrap();
    /*let res = client.get("https://ipv4.icanhazip.com/")/*.header("Host", host)*/.send();
    let text = res.expect("1").text().expect("2");
    println!("text: {}", text);
    let v4 = Ipv4Addr::from_str(text.trim()).expect("3");*/
    let res = client.get("https://ipv6.icanhazip.com/")/*.header("Host", host)*/.send();
    let v6 = res.and_then(Response::text).ok().and_then(|it| Ipv6Addr::from_str(it.trim()).ok());
    let res = client.get("https://ipv4.icanhazip.com/")/*.header("Host", host)*/.send();
    let v4 = res.and_then(Response::text).ok().and_then(|it| Ipv4Addr::from_str(it.trim()).ok());
    (v6, v4)
}
/*enum ServerAddress {
    Ip(IpAddr),
    ViaDNS(String)
}
impl TryFrom<String> for ServerAddress {
    fn from(value: String) -> Self {
        if let Ok(value) = value.parse::<IpAddr>() {

        } else {

        }
    }
}*/