use esphome::Connection;
use std::{
	error::Error,
	net::TcpStream,
	time::{Duration, SystemTime, UNIX_EPOCH},
};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
struct Opt {
	#[structopt(short, long, default_value = "10.10.3.148:6053")]
	address: String,

	#[structopt(short, long)]
	password: Option<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
	let opt = Opt::from_args();
	let mut stream = TcpStream::connect(opt.address)?;
	let mut write_stream = stream.try_clone()?;

	let connection = Connection::new(&mut stream, &mut write_stream);
	let device = connection.connect()?;
	println!("Connected to {}", device.server_info());

	if let Some(password) = opt.password {
		let mut ad = device.authenticate(&password)?;
		println!("Authenticated!");

		ad.device.ping()?;
		println!("Pong!");

		let my_time = (SystemTime::now().duration_since(UNIX_EPOCH)?).as_secs() as u32;
		println!("Device time: {} our time: {}", ad.get_time()?, my_time);
		println!("Device info={:?}", ad.device_info()?);

		ad.subscribe_states()?;
		let entities = ad.list_entities()?;

		loop {
			ad.device.ping()?;
			std::thread::sleep(Duration::from_secs(1));

			for e in &entities {
				println!("- {:?}: {:?}", e, ad.device.connection.get_last_state(e));
			}
		}
	}

	Ok(())
}
