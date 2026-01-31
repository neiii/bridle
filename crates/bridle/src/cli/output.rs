use clap::ValueEnum;
use serde::Serialize;

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
    #[default]
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedFormat {
    Text,
    Json,
}

impl OutputFormat {
    pub fn resolve(self) -> ResolvedFormat {
        match self {
            Self::Text | Self::Auto => ResolvedFormat::Text,
            Self::Json => ResolvedFormat::Json,
        }
    }
}

pub fn output<T, F>(data: &T, format: ResolvedFormat, text_fn: F)
where
    T: Serialize,
    F: FnOnce(&T),
{
    match format {
        ResolvedFormat::Json => {
            println!(
                "{}",
                serde_json::to_string(data).expect("serialization should not fail")
            );
        }
        ResolvedFormat::Text => {
            text_fn(data);
        }
    }
}

pub fn output_list<T, F>(items: &[T], format: ResolvedFormat, text_fn: F)
where
    T: Serialize,
    F: FnOnce(&[T]),
{
    match format {
        ResolvedFormat::Json => {
            println!(
                "{}",
                serde_json::to_string(items).expect("serialization should not fail")
            );
        }
        ResolvedFormat::Text => {
            text_fn(items);
        }
    }
}
