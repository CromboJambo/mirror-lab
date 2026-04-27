use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tri {
    Neg,  // -1
    Zero, // 0
    Pos,  // +1
}

#[allow(dead_code)]
impl Tri {
    pub fn value(self) -> i32 {
        match self {
            Tri::Neg => -1,
            Tri::Zero => 0,
            Tri::Pos => 1,
        }
    }

    pub fn is_positive(self) -> bool {
        matches!(self, Tri::Pos)
    }

    pub fn is_negative(self) -> bool {
        matches!(self, Tri::Neg)
    }

    pub fn is_zero(self) -> bool {
        matches!(self, Tri::Zero)
    }

    pub fn parse_str(s: &str) -> Option<Self> {
        match s {
            "neg" | "-1" | "fail" => Some(Tri::Neg),
            "zero" | "0" | "hold" => Some(Tri::Zero),
            "pos" | "1" | "pass" => Some(Tri::Pos),
            _ => None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TriParseError {
    #[error("Unknown tri value: {0}")]
    UnknownValue(String),
}

impl std::str::FromStr for Tri {
    type Err = TriParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_str(s).ok_or_else(|| TriParseError::UnknownValue(s.to_string()))
    }
}

impl Tri {
    pub fn to_str(self) -> &'static str {
        match self {
            Tri::Neg => "neg",
            Tri::Zero => "zero",
            Tri::Pos => "pos",
        }
    }
}
