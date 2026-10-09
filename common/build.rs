extern crate core;

use flate2::read::GzDecoder;
use reqwest::blocking::*;
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use tar::Archive;

fn main() {
	let target = env::var("TARGET").unwrap();
	let mut split = target.split("-");
	let wstunnel_version = match split.next().unwrap() {
		"x86_64" => "386",
		"aarch64" => "arm64",
		_ => "386"
	};
	println!("cargo:warning=wstunnel version: {}", wstunnel_version);
	println!("cargo:rerun-if-changed=build.rs");
	let loc = format!("https://github.com/erebe/wstunnel/releases/download/v11.0.0/wstunnel_11.0.0_linux_{}.tar.gz", wstunnel_version);
	let out = get(&loc).unwrap();
	//let bytes = out.bytes().unwrap();
	let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
	let dest = out_dir.join("data.tar");
	let mut file = File::create(&dest).unwrap();
	let mut decoder = GzDecoder::new(out);
	let mut buf = vec![0; 1024];
	while let Ok(it) = decoder.read(&mut buf) && it != 0 {
		file.write_all(&buf[0..it]).unwrap();
	}
	file.flush().unwrap();
	let file = File::open(dest).unwrap();
	let mut tar = Archive::new(file);
	for mut file in tar.entries().unwrap().filter_map(Result::ok) {
		if file.path().unwrap().ends_with("wstunnel") {
			file.unpack_in(&out_dir).unwrap();
		}
	}
}