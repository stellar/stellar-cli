use crate::{
    commands::global,
    config::network,
    rpc,
    xdr::{self, Hash},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
        #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
}

#[derive(Debug, clap::Parser)]
pub struct Cmd {
    pub seq: u32,

    #[command(flatten)]
    pub network: network::Args,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let network = self.network.get(&global_args.locator)?;
        let client = network.rpc_client()?;
        let result = client.get_ledgers(self.start(), Some(1)).await;
        println!("RESULT: {:?}", result);
        Ok(())
    }

    // pub async fn fetch_transaction(
    //     &self,
    //     global_args: &global::Args,
    // ) -> Result<GetTransactionResponse, Error> {
    //     let network = self.network.get(&global_args.locator)?;
    //     let client = network.rpc_client()?;
    //     let tx_hash = self.hash.clone();
    //     let tx = client.get_transaction(&tx_hash).await?;
    //     match tx.status.clone() {
    //         val if val == *"NOT_FOUND" => {
    //             if let Some(n) = &self.network.network {
    //                 return Err(Error::NotFound {
    //                     tx_hash,
    //                     network: n.to_string(),
    //                 });
    //             }
    //         }
    //         _ => {}
    //     }
    //     Ok(tx)
    // }

    fn start(&self) -> rpc::LedgerStart {
        rpc::LedgerStart::Ledger(self.seq)
        // let start = match (self.seq, self.cursor.clone()) {
        //     (Some(start), _) => rpc::EventStart::Ledger(start),
        //     (_, Some(c)) => rpc::EventStart::Cursor(c),
        //     // should never happen because of required_unless_present flags
        //     _ => return Err(Error::MissingStartLedgerAndCursor),
        // };
        // Ok(start)
    }
}