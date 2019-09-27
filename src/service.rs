use actix_rt;
use actix_web::{get, middleware, post, web, App, HttpRequest, HttpResponse, HttpServer};
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::HashMap;
use std::path::Path;
use std::fs::File;
use std::io::{self, Error, Write, BufWriter};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;

extern crate env_logger;

struct SharedData {
  config: Value,
  running_state: HashMap<String, bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct JobConfig {
  #[serde(skip_serializing_if = "Option::is_none")]
  name: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  secret: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  scripts: Option<Vec<String>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  path: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct BodyPayload {
  #[serde(skip_serializing_if = "Option::is_none")]
  secret: Option<String>,
}

fn write_out(input: &Vec<u8>, out: Option<String>) {

  let mut out_writer: BufWriter<Box<Write>> = BufWriter::new(match out {
      Some(x) => Box::new(File::create(&Path::new(&x)).unwrap()),
      None => Box::new(io::stdout()),
  });
  out_writer.write(input).unwrap();
}

#[post("/jobs/{name}/run")]
fn run_jobs(
  data: web::Data<Arc<Mutex<SharedData>>>,
  payload: web::Json<BodyPayload>,
  req: HttpRequest,
  name: web::Path<String>,
) -> HttpResponse {

  let job_configs: Vec<JobConfig> = data.clone().lock().unwrap().config["2b"]["jobs"]
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

  let job_secret = matched_jobs[0].clone().secret;

  if job_secret.is_some() {
    let payload_secret = payload.clone().secret;
    if (job_secret.unwrap() != payload_secret.unwrap()) {
      return HttpResponse::Unauthorized()
        .content_type("text/plain")
        .body(format!("Secret is wrong for {}!\r\n", name));
    }
  }

  let job_scripts = matched_jobs[0].clone().scripts;
  let job_path = matched_jobs[0].clone().path;
  // let job_states: Vec<JobConfig> = data.lock().unwrap().running_state;
  // let log_out = data.lock().unwrap().config["2b"]["log"].as_str();

  let log_out = match data.lock().unwrap().config["2b"]["log"].as_str() {
    Some(x) => Some(x.to_string()),
    None => None,
  };

  if (job_scripts.is_some()) {
    thread::spawn(move || {
      for script in job_scripts.unwrap() {
        // TODO: SO uglyyyyy!
        let log_out = match data.lock().unwrap().config["2b"]["log"].as_str() {
          Some(x) => Some(x.to_string()),
          None => None,
        };
        let mut command_scripts = Command::new("sh")
          .arg("-c")
          .arg(script)
          .output()
          .expect("failed to execute process");
        write_out(&command_scripts.stdout, log_out);
      }
    });
  }

  if (job_path.is_some()) {
    thread::spawn(move || {
      let output = Command::new("sh")
        .arg(job_path.unwrap())
        .output()
        .expect("failed to execute process");
      write_out(&output.stdout, log_out);
    });
  }

  HttpResponse::Ok()
    .content_type("text/plain")
    .body(format!("Starting {}!\r\n", name))
}

#[get("/jobs/{name}")]
fn get_jobs(req: HttpRequest, name: web::Path<String>) -> String {
  format!("Starting {}!\r\n", name)
}

// Healthcheck/Liveness Endpoint handler
fn liveness_ep(_req: HttpRequest) -> HttpResponse {
  HttpResponse::Ok()
    .content_type("text/plain")
    .body(format!("Ok!"))
}

fn init_log() -> () {
  std::env::set_var("RUST_LOG", "actix_web=info");
  env_logger::init()
}

pub struct HTTPService {
  config: Value,
}

impl HTTPService {

  pub fn new(conf: Value) -> Result<HTTPService, Error> {
    let config = conf;
    Ok(HTTPService { config })
  }

  pub fn start(&self) -> std::io::Result<()> {

    let sys = actix_rt::System::new("2b-rs");
    let host = self.config["server"]["host"]
      .as_str()
      .unwrap_or("127.0.0.1");
    let port = self.config["server"]["port"].as_u64().unwrap_or(8080);
    let liveness_endpoint = self.config["server"]["health"]
      .as_str()
      .unwrap_or("/healthcheck");
    let liveness_endpoint_str = liveness_endpoint.to_string();

    let shared_data = web::Data::new(Arc::new(Mutex::new(SharedData {
      config: self.config.clone(),
      running_state: HashMap::new(),
    })));

    init_log();

    HttpServer::new(move || {
      App::new()
        .register_data(shared_data.clone())
        .data(web::JsonConfig::default().limit(4096)) // <- limit size of the payload (global configuration)
        .wrap(middleware::Logger::default())
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
