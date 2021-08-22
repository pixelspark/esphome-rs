use std::{error::Error, net::TcpStream};
use esphome::*;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
struct Opt {
	#[structopt(short, long, default_value = "10.10.3.148:6053")]
	address: String,

	#[structopt(short, long)]
	password: Option<String>
}

fn main() -> Result<(), Box<dyn Error>> {
	let opt = Opt::from_args();
	let mut stream = TcpStream::connect(opt.address)?;

	let mut write_stream = stream.try_clone()?;
	//let mut writer = BufWriter::new(&mut write_stream);

	let connection = Connection::new(&mut stream, &mut write_stream);
	let device = connection.connect()?;
	println!("Connected to {}", device.server_info());
	
	if let Some(password) = opt.password {
		let mut ad = device.authenticate(&password)?;
		println!("Authenticated!");

		ad.device.ping()?;
		println!("Pong!");

		println!("Device info={:?}", ad.device_info()?);

		println!("Entities: {:#?}", ad.list_entities()?);

		ad.device.disconnect()?;
	}

	Ok(())
}
