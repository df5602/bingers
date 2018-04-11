use hyper::{StatusCode, Uri};

error_chain! {
    foreign_links {
        Io(::std::io::Error);
        NativeTls(::native_tls::Error);
        Uri(::hyper::error::UriError);
        Hyper(::hyper::Error);
        SerdeJson(::serde_json::error::Error);
        AppDirs(::app_dirs::AppDirsError);
        ParseIntError(::std::num::ParseIntError);
        TokioTimer(::tokio_timer::Error);
    }

    errors {
        HttpError(status: StatusCode, uri: Uri) {
            description("HTTP error"),
            display("HTTP error: Received status code {} from {}", status, uri),
        }

        UserDataVersionMismatch(expected: u32, actual: u32) {
            description("User data version mismatch"),
            display("User data version mismatch [Expected: < {}, actual: {}]", expected, actual),
        }
    }
}

impl From<::tokio_retry::Error<Error>> for Error {
    fn from(retry_error: ::tokio_retry::Error<Error>) -> Self {
        match retry_error {
            ::tokio_retry::Error::OperationError(e) => e,
            ::tokio_retry::Error::TimerError(e) => e.into(),
        }
    }
}
