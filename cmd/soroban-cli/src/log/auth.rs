use crate::xdr::{SorobanAuthorizationEntry, VecM};

pub fn auth(auth: &[VecM<SorobanAuthorizationEntry>]) {
    if !auth.is_empty() {
        tracing::debug!("{auth:#?}");
    }
}
