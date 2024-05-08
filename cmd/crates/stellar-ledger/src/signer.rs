pub trait Blob {
    type Key;
    type Error;
    async fn get_public_key(
        &self,
        key: impl Into<Self::Key>,
    ) -> Result<stellar_strkey::ed25519::PublicKey, Self::Error>;
    async fn sign_blob(
        &self,
        key: impl Into<Self::Key>,
        blob: &[u8],
    ) -> Result<Vec<u8>, Self::Error>;
}
