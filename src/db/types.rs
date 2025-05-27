use std::fmt::Display;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, PartialEq, Copy, Clone)]
pub enum Order {
    Asc,
    Desc,
}

impl Display for Order {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Asc => write!(f, "ASC"),
            Self::Desc => write!(f, "DESC"),
        }
    }
}

impl Default for Order {
    fn default() -> Self {
        Self::Asc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_default() {
        assert_eq!(Order::default(), Order::Asc);
    }

    #[test]
    fn test_order_display() {
        assert_eq!(Order::Asc.to_string(), "ASC");
        assert_eq!(Order::Desc.to_string(), "DESC");
        assert_eq!(format!("{}", Order::Asc), "ASC");
        assert_eq!(format!("{}", Order::Desc), "DESC");
    }
}
