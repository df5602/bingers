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
