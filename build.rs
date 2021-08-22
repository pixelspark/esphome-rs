use protobuf_codegen_pure::Customize;

fn main() {
	let out_dir = std::env::var("OUT_DIR").unwrap();

	protobuf_codegen_pure::Codegen::new()
		.customize(Customize {
			gen_mod_rs: Some(true),
			..Default::default()
		})
		.out_dir(out_dir)
		.input("src/api.proto")
		.include("src/")
		.run()
		.expect("protoc");
}
