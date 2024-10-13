use std::{fs, io, thread, usize};
use std::fs::{create_dir_all, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::str::{FromStr};
use thread_helper::ThreadPool;
use lazy_static::lazy_static;
use http_resources::{HttpProtocols, HttpResponse, HttpResponseOptions};
use crate::ConnectionError::InternalServerErr;

lazy_static!{
    static ref ERR_PAGE: Option<String> = {
        let page = fs::read_to_string("website/__errors__/404.html").unwrap_or_else(|_| {
            let mut file = File::create("website/__errors__/404.html").unwrap();
            file.write_all(b"<!DOCTYPE html><html><body><h1>404</h1></body></html>").unwrap();
            "<html><body><h1>404</h1></body></html>".to_string()
        });
        Some(format!("HTTP/1.1 404 NOT FOUND\r\nContent-Len: {}\r\n\r\n{page}", page.len()))
    };

    static ref SERVER_ERR_PAGE: Option<String> = {
        let page = fs::read_to_string("website/__errors__/500.html").unwrap_or_else(|_| {
            let mut file = File::create("website/__errors__/404.html").unwrap();
            file.write_all(b"<!DOCTYPE html><html><body><h1>500</h1></body></html>").unwrap();
            "<html><body><h1>404</h1></body></html>".to_string()
        });
        Some(format!("HTTP/1.1 500 Internal Server Error\r\nContent-Len: {}\r\n\r\n{page}", page.len()))
    };

    static ref CONF: Config = parse_config().unwrap_or_else(|| {
        println!("Error! The config cannot be properly parsed.");
        println!("Aborting the startup of the web server until the config file can be accessed.");
        finish_wait();
        Config {
            ip: "".to_string(),
            port: "".to_string(),
            home_name: "home".to_string(),
            ssl: "".to_string(),
            threads: 20,
        }
    });
}

enum ConnectionError {
    TCPReadFailed,
    SourceNotFound,
    InternalServerErr,
}

impl ConnectionError {
    fn get_html_err_msg(&self) -> &[u8] {
        match self {
            ConnectionError::TCPReadFailed => "HTTP/1.1 400 BAD REQUEST".as_bytes(),
            ConnectionError::SourceNotFound => ERR_PAGE.as_ref().map_or_else(|| "HTTP/1.1 404 NOT FOUND".as_bytes(), |s| s.as_bytes()),
            InternalServerErr => SERVER_ERR_PAGE.as_ref().map_or_else(|| "HTTP/1.1 500 Internal Server Error".as_bytes(), |s| s.as_bytes()),
        }
    }
}

struct Config {
    ip: String,
    port: String,
    threads: usize,
    home_name: String,
    ssl: String,
}

fn main() {
    println!("Starting web server...");

    match fs::read_dir("website") {
        Ok(_) => {}
        Err(_) => create_dir_all("website/__errors__").unwrap_or(()),
    }

    let ip = format!("{}:{}", &CONF.ip.as_str(), &CONF.port.as_str());

    let listener = TcpListener::bind(&ip).map_err(|_| {
        println!("Error! Unable to bind to port {ip}!");
        finish_wait();
    }).unwrap();
    let pool = ThreadPool::new(CONF.threads);

    let input_thread = thread::spawn(move || {
        let mut input = String::new();
        loop {
            io::stdin().read_line(&mut input).map_or_else(|_| 0, |s| s);
            if input.trim() == "stop" {
                println!("Stopping the web server...");
                break;
            } else if input.trim() == "config-reload" {
                println!("Reloading the config...");
                println!("Beware that only changeable values will change, such as the location of the website. Static values will not, like the ip and port. To change those settings, restart the server.");
            }
            input.clear();
        }
        finish_wait();
    });

    println!("Successfully started! Listening on: {ip}...");

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(r) => r,
            Err(_) => continue,
        };

        pool.execute(move || {
            let result = handle_connection(&mut stream);
            match result {
                Ok(response) => response.send(&mut stream),
                Err(e) => {
                    stream.write(e.get_html_err_msg()).unwrap_or(0);
                    ()
                },
            };
            stream.flush().unwrap_or(());
        });
    }

    input_thread.join().expect("Input thread panicked");

    finish_wait();
}

fn handle_connection(mut stream: &mut TcpStream) -> Result<HttpResponse, ConnectionError> {
    let buf_reader = BufReader::new(&mut stream);
    let mut http_request = buf_reader
        .lines()
        .map(|result| result.map_err(|_| ConnectionError::TCPReadFailed))
        .map(|result| result.unwrap_or("".to_string()))
        .take_while(|line| !line.is_empty());

    let mut path: String = {
        http_request.next()
            .ok_or(ConnectionError::TCPReadFailed)?
            .split(" ")
            .nth(1)
            .map(|s| s.to_string())
    }.ok_or(ConnectionError::TCPReadFailed)?;

    let mut response: HttpResponse = HttpResponse::new(HttpProtocols::OneOne);
    match Path::new(path.as_str()).extension().and_then(|ext| ext.to_str()) {
        Some("html") | None => {
            if path == "/" {
                path = ("/".to_owned() + *&CONF.home_name.as_str()).to_string();
            }
            path = path + ".html";
            response.append_option(HttpResponseOptions::ContentType, "text/html")
        },
        Some("css") => response.append_option(HttpResponseOptions::ContentType, "text/css"),
        Some("png") => response.append_option(HttpResponseOptions::ContentType, "image/png"),
        Some("ico") => response.append_option(HttpResponseOptions::ContentType, "image/x-icon"),
        Some("js") => response.append_option(HttpResponseOptions::ContentType, "application/javascript"),
        Some("wasm") => response.append_option(HttpResponseOptions::ContentType, "application/wasm"),
        _ => return Err(InternalServerErr)
    };

    let mut content: Vec<u8> = Vec::new();
    File::open(format!("website{path}")).ok().ok_or(ConnectionError::SourceNotFound)?.read_to_end(&mut content).ok().ok_or(InternalServerErr)?;

    response.append_option(HttpResponseOptions::ContentLength, Box::leak(Box::new(content.len().to_string())).as_str());
    response.append_payload(content);

    Ok(response)
}

fn parse_config() -> Option<Config> {
    let file = match File::open("settings.cfg") {
        Ok(file) => file,
        Err(err) => {
            eprintln!("Error opening configuration file: {}", err);
            return None;
        }
    };
    let reader = BufReader::new(file);

    let mut out = Config {
        ip: "127.0.0.1".to_string(),
        port: "8080".to_string(),
        home_name: "home".to_string(),
        ssl: "".to_string(),
        threads: 20,
    };

    let mut suppress_warning: bool = false;

    for line in reader.lines().map(|s| s.map_or_else(|_| "".to_string(), |s| s)).collect::<Vec<String>>() {

        if line.trim().is_empty() || line.trim().starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.split('=').map(|s| s.trim()).collect();

        if parts.len() != 2 {
            if !suppress_warning {
                println!("Warning: Invalid line in settings.cfg: {}", line);
                println!("Continuing, but this line will be skipped.");
                println!("To ignore these warnings add \"suppress-warnings = true\" at the top of the settings.cfg file.");
            }
            continue;
        }

        let key = parts[0];
        let value = parts[1];

        match key {
            "ip" => out.ip = value.trim_matches('\"').to_string(),
            "port" => out.port = value.trim_matches('\"').to_string(),
            "num-threads" => out.threads = usize::from_str(value).unwrap_or(20),
            "suppress-warnings" => suppress_warning = bool::from_str(value).unwrap_or(true),
            "home-name" => out.home_name = value.trim_matches('\"').to_string(),
            "ssl-cert" => out.ssl = value.trim_matches('\"').to_string(),
            _ => {}
        }
    }

    Some(out)
}

fn finish_wait() {
    println!("Press enter to continue...");
    let mut temp = String::new();
    io::stdin().read_line(&mut temp).unwrap();
    std::process::exit(0);
}