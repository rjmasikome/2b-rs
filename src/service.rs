use actix_rt;
use actix_web::{get, web, App, HttpRequest, HttpResponse, HttpServer};
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::io::Error;
use std::sync::{Arc, Mutex};

pub struct Service {
  config: Value,
}

#[derive(Debug)]
pub struct JobConfigScripts {
  name: String,
  scripts: Vec<String>,
}

pub struct JobConfigPath {
  name: String,
  path: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JobConfig {
  #[serde(skip_serializing_if = "Option::is_none")]
  name: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  scripts: Option<Vec<String>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  path: Option<String>,
}

impl JobConfig {
  
  fn from_scripts(config: JobConfigScripts) -> JobConfig {
    JobConfig {
      name: Some(config.name),
      scripts: Some(config.scripts),
      path: None,
    }
  }

  fn from_path(config: JobConfigPath) -> JobConfig {
    JobConfig {
      name: Some(config.name),
      scripts: None,
      path: Some(config.path),
    }
  }
}

#[get("/jobs/{name}")]
fn get_jobs(data: web::Data<Arc<Mutex<Value>>>, req: HttpRequest, name: web::Path<String>) -> HttpResponse {

  let job_configs: Vec<JobConfig> = data
    .lock().unwrap()["2b"]["jobs"]
    .as_sequence()
    .expect("Wrong configs")
    .iter().map (
      |value| serde_yaml::from_value (value.clone()).unwrap()
    )
    .collect();

  let matched_job: Vec<String> = job_configs
    .iter()
    .map (
      |value| value.name.clone().unwrap()
    )
    .filter (
      |job_name| job_name.clone() == name.to_string()
    )
    .collect();

  if matched_job.len() > 0 {
    
    println!("{:?}", matched_job[0]);
    println!("REQ: {:?}", req);

    return HttpResponse::Ok()
      .content_type("text/plain")
      .body(format!("Hello: {}!\r\n", name))
  } 

  HttpResponse::InternalServerError()
      .content_type("text/plain")
      .body(format!("Job {} not found!\r\n", name))
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
    let shared_config = web::Data::new(Arc::new(Mutex::new(self.config.clone())));

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
        .register_data(shared_config.clone())
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
