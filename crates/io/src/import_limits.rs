//! Source payloads and decoded images have independent memory budgets.
pub(crate) const MAX_MODEL_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub(crate) const MAX_TEXTURE_BYTES: usize = 256 * 1024 * 1024;
