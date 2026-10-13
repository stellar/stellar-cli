#[async_trait::async_trait]
// async_trait emits `#[must_use]` on methods that already return a must_use
// `Pin<Box<dyn Future>>`, tripping clippy 1.99.0's double_must_use. See #2784.
#[allow(clippy::double_must_use)]
pub trait Blob {
    type Key: Send;
    type Error;
    async fn get_public_key(
        &self,
        key: &Self::Key,
    ) -> Result<stellar_strkey::ed25519::PublicKey, Self::Error>;
    async fn sign_blob(&self, key: &Self::Key, blob: &[u8]) -> Result<Vec<u8>, Self::Error>;
}
