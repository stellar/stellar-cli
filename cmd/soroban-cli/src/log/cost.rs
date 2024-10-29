use crate::xdr::SorobanResources;
use std::fmt::{Debug, Display};

struct Cost<'a>(&'a SorobanResources);

impl Debug for Cost<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Should we output the footprint here?
        writeln!(f, "==================== Cost ====================")?;
        writeln!(f, "CPU used: {}", self.0.instructions,)?;
        writeln!(f, "Bytes read: {}", self.0.read_bytes,)?;
        writeln!(f, "Bytes written: {}", self.0.write_bytes,)?;
        writeln!(f, "==============================================")?;
        Ok(())
    }
}

impl Display for Cost<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

pub fn cost(resources: &SorobanResources) {
    let cost = Cost(resources);
    tracing::debug!(?cost);
}
