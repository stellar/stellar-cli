use std::future::Future;

pub trait Blob {
    type Key: Send;
    type Error;
    fn get_public_key(
        &self,
        key: &Self::Key,
    ) -> impl Future<Output = Result<stellar_strkey::ed25519::PublicKey, Self::Error>> + Send;
    fn sign_blob(
        &self,
        key: &Self::Key,
        blob: &[u8],
    ) -> impl Future<Output = Result<Vec<u8>, Self::Error>> + Send;
}
