use crate::common::errors::FFResult;
use serde::{Deserialize, Serialize};
use serde_json::Error;
use std::io::Read;
use std::time::SystemTime;

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