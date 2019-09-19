use std::env;

mod parser;
mod service;

use parser::config;

// Main function
fn main() -> std::io::Result<()> {

  let args: Vec<String> = env::args().collect();
  let mut filename_arg = None;
  if args.len() > 1 {
    filename_arg = Some(&args[1]);
  }

  let conf = config::get_config(filename_arg).expect("Failed to load YAML config.");
  let service_obj = service::Service::new(conf.clone())?;

  service_obj.start()
}
