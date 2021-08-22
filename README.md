# ESPHome.rs

ESPHome API client for Rust.

## Usage

````rust
use esphome::Connection;
use std::net::TcpStream;

let mut stream = TcpStream::connect(opt.address)?;
let mut write_stream = stream.try_clone()?;
let connection = Connection::new(&mut stream, &mut write_stream);
let device = connection.connect()?;
println!("Connected to {}", device.server_info());

if let Some(password) = opt.password {
	let ad = device.authenticate(&password)?;
	// ...
}
````

## Running an example

````sh
cargo run --example connect -- -a some.device:6053 -p some_password
````

## License

[MIT](./LICENSE.txt) except for the following:

* [src/api.proto](./src/api.proto) and [src/api_options.proto](./src/api_options.proto): copied from
 [the aioesphomeapi repository](https://github.com/esphome/aioesphomeapi/tree/main/aioesphomeapi) under the MIT license.
