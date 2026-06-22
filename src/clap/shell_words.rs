use std::ops::Deref;

use clap::Error;
use clap::builder::TypedValueParser;
use clap::builder::ValueParserFactory;
use clap::error::ErrorKind;

/// Shell-quoted arguments.
///
/// Specified on the command-line as one string, parsed as a `Vec<String>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellWords(Vec<String>);

impl AsRef<Vec<String>> for ShellWords {
    fn as_ref(&self) -> &Vec<String> {
        &self.0
    }
}

impl Deref for ShellWords {
    type Target = Vec<String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ValueParserFactory for ShellWords {
    type Parser = ShellWordsParser;
    fn value_parser() -> Self::Parser {
        ShellWordsParser
    }
}

#[derive(Clone, Debug)]
pub struct ShellWordsParser;
impl TypedValueParser for ShellWordsParser {
    type Value = ShellWords;

    fn parse_ref(
        &self,
        _cmd: &clap::Command,
        arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        fn arg_display(arg: Option<&clap::Arg>) -> String {
            arg.map(|arg| arg.to_string())
                .unwrap_or_else(|| "unknown arg".to_owned())
        }

        let value = value.to_str().ok_or_else(|| {
            Error::raw(
                ErrorKind::InvalidValue,
                format!(
                    "Value for {} is not valid UTF-8: {:?}",
                    arg_display(arg),
                    value
                ),
            )
        })?;

        match shell_words::split(value) {
            Ok(words) => Ok(ShellWords(words)),
            Err(err) => Err(Error::raw(
                ErrorKind::InvalidValue,
                format!(
                    "Value for {} could not be shell-unquoted: {}: {:?}",
                    arg_display(arg),
                    err,
                    value,
                ),
            )),
        }
    }
}
