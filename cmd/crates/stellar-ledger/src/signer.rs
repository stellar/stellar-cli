#[async_trait::async_trait]
pub trait Blob {
    type Key: Send;
    type Error;
    async fn get_public_key(
        &self,
        key: &Self::Key,
    ) -> Result<stellar_strkey::ed25519::PublicKey, Self::Error>;
    async fn sign_blob(&self, key: &Self::Key, blob: &[u8]) -> Result<Vec<u8>, Self::Error>;
}
