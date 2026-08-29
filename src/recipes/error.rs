use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum RecipeCatalogError {
    Read { path: PathBuf, message: String },
    Parse { path: PathBuf, message: String },
    Schema { path: PathBuf, errors: Vec<String> },
    Semantic { path: PathBuf, message: String },
    Completeness { message: String },
}

impl fmt::Display for RecipeCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, message } => write!(formatter, "read {}: {message}", path.display()),
            Self::Parse { path, message } => {
                write!(formatter, "parse {}: {message}", path.display())
            }
            Self::Schema { path, errors } => {
                write!(
                    formatter,
                    "schema validation {}: {}",
                    path.display(),
                    errors.join("; ")
                )
            }
            Self::Semantic { path, message } => {
                write!(formatter, "recipe {}: {message}", path.display())
            }
            Self::Completeness { message } => formatter.write_str(message),
        }
    }
}

impl std::error::Error for RecipeCatalogError {}
