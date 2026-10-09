use std::{env, fs};
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
	let dir = env::var("CARGO_MANIFEST_DIR").expect("no manifest dir");
	let root = PathBuf::from(&dir).join("..");
	let generator_root = root.join("generator");
	println!("cargo::rerun-if-changed={}", generator_root.canonicalize().unwrap().display());
	copy(&root, false);
	copy(&root, true);
}
fn copy(root: &Path, arm: bool) {
	let gen_loc = if !arm { root.join("target/release/generator") } else {
		root.join("target/aarch64-unknown-linux-gnu/release/generator")
	};
	let out = PathBuf::from(&env::var("OUT_DIR").unwrap())
		.join(if arm {"arm.bin"} else {"x86.bin"});
	println!("cargo::warning={},{}", gen_loc.display(), out.display());
	fs::copy(gen_loc, out.clone()).expect("could not write");

	let bytes = root.join("generator/def_key/public.pem");
	let bytes = fs::read(bytes).unwrap();
	let file = fs::read(out).unwrap();
	for i in 0..file.len()-bytes.len() {
		if bytes.eq(&file[i..i + bytes.len()]) {
			if arm {
				println!("cargo::rustc-env=LOC_ARM={}", i);
			} else {
				println!("cargo::rustc-env=LOC={}", i);
			}
			return;
		}
	}
}