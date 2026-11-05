//! Finished rows kept as their JSON text ([`J::Raw`]).
use super::J;

/// JSON text, written as it is ([`J::Raw`]).
#[derive(Clone, Debug)]
pub(super) struct Raw(pub(super) Box<serde_json::value::RawValue>);
impl PartialEq for Raw {
    fn eq(&self, other: &Self) -> bool {
        self.0.get() == other.0.get()
    }
}

impl J {
    /// A [`J::Raw`] read back into its tree (tests read the rows a plan wrote).
    #[cfg(test)]
    pub(super) fn parsed(self) -> J {
        match self {
            J::Raw(r) => serde_json::from_str(r.0.get()).unwrap_or(J::Null),
            other => other,
        }
    }
    /// This value as its JSON text ([`J::Raw`]).
    pub(super) fn into_raw(self) -> J {
        match serde_json::value::to_raw_value(&self) {
            Ok(raw) => J::Raw(Raw(raw)),
            Err(_) => self,
        }
    }
}
