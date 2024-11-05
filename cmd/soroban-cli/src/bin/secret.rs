use soroban_cli::signer::keyring::{add_key, get_public_key, StellarEntry};

fn main() {
    let entry = StellarEntry::new("test").unwrap();
    if let Ok(key) = entry.get_public_key() {
        println!("{key}")
    };

    let secret = soroban_cli::config::secret::Secret::from_seed(None).unwrap();
    let pub_key = secret.public_key(None).unwrap();
    let key_pair = secret.key_pair(None).unwrap();
    entry.add_password(key_pair.as_bytes()).unwrap();
    let pub_key_2 = entry.get_public_key().unwrap();
    assert_eq!(pub_key, pub_key_2);
    println!("{pub_key} == {pub_key_2}");
}
