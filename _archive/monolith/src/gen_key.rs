use solana_sdk::signature::{Keypair, Signer};

fn main() {
    let keypair = Keypair::new();
    println!("{:?}", keypair.to_bytes());
}
