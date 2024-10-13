use std::collections::HashMap;
use std::hash::{Hash};
use std::io::Write;
use std::net::TcpStream;

#[derive(Debug)]
#[derive(Hash)]
#[derive(Eq, PartialEq)]
pub enum HttpResponseOptions {
    ContentType,
    ContentLength,
}

impl HttpResponseOptions {
    pub fn get_name(&self) -> &str {
        match self {
            HttpResponseOptions::ContentType => "Content-Type",
            HttpResponseOptions::ContentLength => "Content-Length",
        }
    }
}

#[derive(Debug)]
#[derive(PartialEq)]
pub enum HttpProtocols {
    ZeroNine,
    One,
    OneOne,
    Two
}

impl HttpProtocols {
    pub fn get_name(&self) -> &str {
        match self {
            HttpProtocols::ZeroNine => "HTTP/0.9",
            HttpProtocols::One => "HTTP/1.0",
            HttpProtocols::OneOne => "HTTP/1.1",
            HttpProtocols::Two => "HTTP/2.0",
        }
    }
}

#[derive(Debug)]
#[derive(PartialEq)]
pub enum HttpResponseStatusCode {
    OK,
    NotFound,
    InternalServerError,
}

impl HttpResponseStatusCode {
    pub fn get_header(&self) -> &str {
        match self {
            HttpResponseStatusCode::OK => "200 Ok",
            HttpResponseStatusCode::NotFound => "404 Not Found",
            HttpResponseStatusCode::InternalServerError => "500 Internal Server Error",
        }
    }
}

#[derive(Debug)]
#[derive(PartialEq)]
pub struct HttpResponse {
    protocol: HttpProtocols,
    status: HttpResponseStatusCode,
    options: HashMap<HttpResponseOptions, &'static str>,
    payload: Vec<u8>,
}

impl HttpResponse {
    pub const SEPARATOR: &'static str = "\r\n";

    pub fn new(protocol: HttpProtocols) -> HttpResponse {
        if protocol != HttpProtocols::OneOne {
            println!("Warning! You are using the \"{}\" HTTP protocol, which is not directly supported by this library. Only proceed if you know what you are doing!", protocol.get_name());
        }
        HttpResponse {
            protocol,
            status: HttpResponseStatusCode::OK,
            options: HashMap::new(),
            payload: Vec::new(),
        }
    }

    pub fn set_status(&mut self, new_status: HttpResponseStatusCode) {
        self.status = new_status;
    }

    pub fn append_option(&mut self, option: HttpResponseOptions, payload: &'static str) {
        self.options.insert(option, payload);
    }

    pub fn append_payload(&mut self, payload: Vec<u8>) {
        self.payload = payload
    }

    pub fn send(&self, stream: &mut TcpStream) {
        let out: String = self.get_header();
        stream.write_all(out.as_bytes()).unwrap_or(());
        stream.write(&self.payload).unwrap_or(0);
    }

    pub fn get_header(&self) -> String {
        let mut out: String = String::new();
        out.push_str(self.protocol.get_name());
        out.push_str(" ");
        out.push_str(self.status.get_header());
        out.push_str(Self::SEPARATOR);
        for (key, value) in &self.options {
            out.push_str(key.get_name());
            out.push_str(": ");
            out.push_str(value);
            out.push_str(Self::SEPARATOR);
        }
        out.push_str(Self::SEPARATOR);
        out
    }
}

#[cfg(test)]
mod tests {  }
