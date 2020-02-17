use nix;
use std::error::Error as StdError;
use std::fmt::{self, Display};
use std::num::ParseIntError;
use std::str::Utf8Error;
use udev;

macro_rules! error_wrapper {
    ( $( $name:ident ( $inner:ty ) ),+ ) => {
        #[derive(Debug)]
        pub enum Error {
            $( $name($inner) ),*
        }

        $(
            impl From<$inner> for Error {
                fn from(e: $inner) -> Self {
                    Error::$name(e)
                }
            }
        )*

        impl Display for Error {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    $(
                        Error::$name(e) => e.fmt(f)
                    ),*
                }
            }
        }


        impl StdError for Error {}
    };
}

error_wrapper! {
    Nix(nix::Error),
    Utf8Error(Utf8Error),
    ParseIntError(ParseIntError),
    UdevError(udev::Error)
}

pub type Result<T> = std::result::Result<T, Error>;
