use dotenv::dotenv;
use log::info;

mod commands;
mod gui;
mod renderer_impl;

fn main() {
    dotenv().ok(); // load environment variables
    env_logger::init();
    info!("Starting up engine...");

    renderer_impl::start();
}
