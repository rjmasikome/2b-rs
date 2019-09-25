use actix_rt;
use actix_web::{get, post, web, App, HttpRequest, HttpResponse, HttpServer};
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::io::Error;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::collections::HashMap;

pub struct Service {
  config: Value
}

struct SharedData {
  config: Value,
  running_state: HashMap<String, bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JobConfig {
  #[serde(skip_serializing_if = "Option::is_none")]
  name: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  scripts: Option<Vec<String>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  path: Option<String>,
}

#[post("/jobs/{name}/run")]
fn run_jobs(
  data: web::Data<Arc<Mutex<SharedData>>>,
  req: HttpRequest,
  name: web::Path<String>,
) -> HttpResponse {

  let job_configs: Vec<JobConfig> = data.lock().unwrap()
    .config["2b"]["jobs"]
    .as_sequence()
    .expect("Wrong configs")
    .iter()
    .map(|value| serde_yaml::from_value(value.clone()).unwrap())
    .collect();

  let matched_jobs: Vec<JobConfig> = job_configs
    .iter()
    .filter(|value| value.name.clone().unwrap() == name.to_string())
    .cloned()
    .collect();

  if matched_jobs.len() == 0 {
    return HttpResponse::InternalServerError()
      .content_type("text/plain")
      .body(format!("Job {} not found!\r\n", name));
  }

  println!("REQ: {:?}", req);

  let job_scripts = matched_jobs[0].clone().scripts;
  let job_path = matched_jobs[0].clone().path;
  // let job_states: Vec<JobConfig> = data.lock().unwrap().running_state;

  if (job_scripts.is_some()) {
    thread::spawn(move || {
      for script in job_scripts.unwrap() {
        let mut command_scripts = Command::new("sh")
          .arg("-c")
          .arg(script)
          .output()
          .expect("failed to execute process");

        let s = String::from_utf8_lossy(&command_scripts.stdout);
        println!("{}", s);
      }
    });
  }

  if (job_path.is_some()) {
    Command::new("sh")
      .arg(job_path.unwrap())
      .spawn()
      .expect("failed to execute process");
  }

  HttpResponse::Ok()
    .content_type("text/plain")
    .body(format!("Hello: {}!\r\n", name))
}

#[get("/jobs/{name}")]
fn get_jobs(req: HttpRequest, name: web::Path<String>) -> String {

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

    let sys = actix_rt::System::new("2b-rs");
    let host = self.config["server"]["host"].as_str().unwrap_or("127.0.0.1");
    let port = self.config["server"]["port"].as_u64().unwrap_or(8080);
    let liveness_endpoint = self.config["server"]["health"].as_str().unwrap_or("/healthcheck");
    let liveness_endpoint_str = liveness_endpoint.to_string();

    let shared_data = web::Data::new(Arc::new(Mutex::new(
      SharedData {
        config: self.config.clone(),
        running_state: HashMap::new(),
    })));

    HttpServer::new(move || {
      App::new()
        .register_data(shared_data.clone())
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
