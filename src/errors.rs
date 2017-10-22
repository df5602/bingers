use hyper::StatusCode;

error_chain! {
    foreign_links {
        Io(::std::io::Error);
        NativeTls(::native_tls::Error);
        Uri(::hyper::error::UriError);
        Hyper(::hyper::Error);
        SerdeJson(::serde_json::error::Error);
    }

    errors {
        HttpError(status: StatusCode) {
            description("HTTP error"),
            display("HTTP error: Received status code {}", status),
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
