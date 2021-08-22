use protobuf::{CodedInputStream, CodedOutputStream};
use std::{
	error::Error,
	io::{Read, Write}
};
use thiserror::Error;
use crate::api::{self, ConnectResponse, HelloResponse};

pub struct Connection<'a> {
	cis: CodedInputStream<'a>,
	cos: CodedOutputStream<'a>,
}

#[derive(Debug, Copy, Clone)]
pub enum MessageType {
	HelloRequest = 1,
	HelloResponse = 2,
	ConnectRequest = 3,
	ConnectResponse = 4,
}

#[derive(Error, Debug)]
pub enum ESPHomeError {
	#[error("The password was not valid")]
	InvalidPassword,

	#[error("Received an unexpected response type (expected {expected:?}, received {received:?})")]
	UnexpectedResponse {
		expected: MessageType,
		received: u32
	}
}

impl<'a> Connection<'a> {
	pub fn new<R, W>(reader: &'a mut R, writer: &'a mut W) -> Connection<'a>
	where
		R: Read,
		W: Write,
	{
		Connection {
			cis: CodedInputStream::new(reader),
			cos: CodedOutputStream::new(writer),
		}
	}
}

impl<'a> Connection<'a> {
	fn send_message<M>(
		&mut self,
		message_type: MessageType,
		message: &M,
	) -> Result<(), Box<dyn Error>>
	where
		M: protobuf::Message,
	{
		let message_bytes = message.write_to_bytes()?;
		self.cos.write_raw_byte(0)?;
		self.cos.write_raw_varint32(message_bytes.len() as u32)?;
		self.cos.write_raw_varint32(message_type as u32)?;
		self.cos.write_raw_bytes(&message_bytes)?;
		self.cos.flush()?;
		Ok(())
	}

	fn receive_message<M>(
		&mut self,
		message_type: MessageType,
	) -> Result<M, Box<dyn Error>>
	where
		M: protobuf::Message
	{
		let mut zero = [0u8; 1];
		self.cis.read_exact(&mut zero)?;
		let len = self.cis.read_raw_varint32()?;
		let tp = self.cis.read_raw_varint32()?;
		if tp != (message_type as u32) {
			return Err(Box::new(ESPHomeError::UnexpectedResponse { expected: message_type, received: tp }))
		}
		let mut message_bytes = vec![0u8; len as usize];
		self.cis.read_exact(&mut message_bytes)?;

		Ok(M::parse_from_bytes(&message_bytes)?)
	}

	pub fn connect(mut self) -> Result<Device<'a>, Box<dyn Error>> {
		let mut hr = api::HelloRequest::new();
		hr.set_client_info("esphome.rs".to_string());
		self.send_message(MessageType::HelloRequest, &hr)?;

		
		let hr: HelloResponse = self.receive_message(MessageType::HelloResponse)?;
		Ok(Device::new(self, hr))
	}
}

pub struct Device<'a> {
	connection: Connection<'a>,
	hello_information: api::HelloResponse
}

impl<'a> Device<'a> {
	fn new(connection: Connection<'a>, hello_information: HelloResponse) -> Device<'a> {
		Device {
			connection,
			hello_information
		}
	}

	pub fn server_info(&self) -> String {
		self.hello_information.get_server_info().to_owned()
	}

	pub fn authenticate(mut self, password: &str) -> Result<AuthenticatedDevice<'a>, Box<dyn Error>> {
		let mut cr = api::ConnectRequest::new();
		cr.set_password(password.to_string());
		self.connection.send_message(MessageType::ConnectRequest, &cr)?;
		let cr: ConnectResponse = self.connection.receive_message(MessageType::ConnectResponse)?;

		if cr.get_invalid_password() {
			return Err(Box::new(ESPHomeError::InvalidPassword))
		}

		Ok(AuthenticatedDevice::new(self))
	}
}

pub struct AuthenticatedDevice<'a> {
	device: Device<'a>
}

impl<'a> AuthenticatedDevice<'a> {
	fn new(device: Device<'a>) -> AuthenticatedDevice<'a> {
		AuthenticatedDevice {
			device
		}
	}
}