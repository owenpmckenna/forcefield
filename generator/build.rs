use rsa::RsaPublicKey;
use rsa::pkcs8::DecodePublicKey;

fn main() {
	let out = include_str!("def_key/public.pem");
	let key = RsaPublicKey::from_public_key_pem(out).unwrap();
}