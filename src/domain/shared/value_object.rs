use std::fmt::Debug;

pub trait ValueObject: Clone + PartialEq + Debug + Send + Sync {}
