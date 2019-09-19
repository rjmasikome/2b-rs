use actix_rt;
use actix_web::{get, web, App, HttpRequest, HttpResponse, HttpServer};
use serde_yaml::Value;
use std::io::Error;

pub struct Service {
  config: Value,
}

#[get("/jobs/{name}")]
fn get_jobs(data: web::Data<Value>, req: HttpRequest, name: web::Path<String>) -> String {
    println!("REQ: {:?}", req);
    format!("Hello: {}!\r\n", name)
}

#[get("/jobs/{name}/run")]
fn run_jobs(req: HttpRequest, name: web::Path<String>) -> String {
    println!("REQ: {:?}", req);
    format!("Hello: {}!\r\n", name)
}

// Healthcheck/Liveness Endpoint handler
fn liveness_ep(_req: HttpRequest) -> HttpResponse {

  HttpResponse::Ok()
    .content_type("text/plain")
    .body(format!("Ok!"))
}

impl Service {
  pub fn new(conf: Value) -> Result<Service, Error> {
    let config = conf;
    Ok(Service { config })
  }

  pub fn start(&self) -> std::io::Result<()> {

    let sys = actix_rt::System::new("pinger-rs");
    let shared_config = web::Data::new(self.config.clone());

    let host = self.config["server"]["host"]
      .as_str()
      .unwrap_or("127.0.0.1");
    let port = self.config["server"]["port"].as_u64().unwrap_or(8080);

    let liveness_endpoint = self.config["server"]["health"]
      .as_str()
      .unwrap_or("/healthcheck");

    let liveness_endpoint_str = liveness_endpoint.to_string();

    HttpServer::new(move || {
      App::new()
        .data(shared_config.clone())
        .service(get_jobs)
        .service(run_jobs)
        .service(web::resource(&liveness_endpoint_str).to(liveness_ep))
    })
    .bind(format!("{}:{}", host, port))?
    .start();

    println!(
      "Healthcheck ready at: http://{}:{}{}",
      host, port, liveness_endpoint
    );

    sys.run()
  }
}
