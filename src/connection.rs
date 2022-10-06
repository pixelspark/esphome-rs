use crate::{
	api::{self, HelloResponse},
	model::State,
	Device, Entity, EspHomeError, MessageType,
};
use num_traits::FromPrimitive;
use protobuf::{CodedInputStream, CodedOutputStream};
use std::{
	collections::HashMap,
	error::Error,
	io::{Read, Write},
	time::{SystemTime, UNIX_EPOCH}, mem::MaybeUninit,
};

#[derive(Debug)]
pub(crate) struct MessageHeader {
	message_length: u32,
	message_type: u32,
}

impl MessageHeader {
	pub(crate) fn message_type(&self) -> u32 {
		self.message_type
	}
}

pub struct Connection<'a> {
	cis: CodedInputStream<'a>,
	cos: CodedOutputStream<'a>,
	states: HashMap<u32, State>,
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
			states: HashMap::new(),
		}
	}
}

impl<'a> Connection<'a> {
	pub(crate) fn send_message<M>(
		&mut self,
		message_type: MessageType,
		message: &M,
	) -> Result<(), EspHomeError>
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

	pub fn get_last_state(&mut self, entity: &Entity) -> Result<Option<State>, Box<dyn Error>> {
		match self.states.get(&entity.key()) {
			Some(s) => Ok(Some(s.clone())),
			None => Ok(None),
		}
	}

	pub(crate) fn receive_message<M>(
		&mut self,
		message_type: MessageType,
	) -> Result<M, EspHomeError>
	where
		M: protobuf::Message,
	{
		let header = self.receive_message_header()?;
		if header.message_type != (message_type as u32) {
			return Err(EspHomeError::UnexpectedResponse {
				expected: message_type,
				received: header.message_type,
			});
		}
		self.receive_message_body(&header)
	}

	pub(crate) fn receive_message_body<M>(
		&mut self,
		header: &MessageHeader,
	) -> Result<M, EspHomeError>
	where
		M: protobuf::Message,
	{
		let mut message_bytes: [MaybeUninit::<u8>; 4096] = unsafe { MaybeUninit::uninit().assume_init() };
		self.cis.read_exact(&mut message_bytes[0..header.message_length as usize] )?;
		let data = unsafe { std::mem::transmute::<_, [u8; 4096]>(message_bytes) };
		Ok(M::parse_from_bytes(&data[0..header.message_length as usize])?)
	}

	fn ignore_bytes(&mut self, bytes: u32) -> Result<(), EspHomeError> {
		self.cis.skip_raw_bytes(bytes)?;
		Ok(())
	}

	fn process_unsolicited(&mut self, header: &MessageHeader) -> Result<bool, EspHomeError> {
		match FromPrimitive::from_u32(header.message_type) {
			Some(MessageType::PingRequest) => {
				self.receive_message_body::<api::PingRequest>(&header)?;
				self.send_message(MessageType::PingResponse, &api::PingResponse::new())?;
				Ok(true)
			}
			Some(MessageType::DisconnectRequest) => {
				self.receive_message_body::<api::DisconnectRequest>(&header)?;
				self.send_message(
					MessageType::DisconnectResponse,
					&api::DisconnectResponse::new(),
				)?;
				// TODO: actually disconnect
				Ok(true)
			}
			Some(MessageType::GetTimeRequest) => {
				self.receive_message_body::<api::GetTimeRequest>(&header)?;
				let mut res = api::GetTimeResponse::new();
				res.epoch_seconds =
					(SystemTime::now().duration_since(UNIX_EPOCH)?).as_secs() as u32;
				self.send_message(MessageType::GetTimeResponse, &res)?;
				Ok(true)
			}

			Some(MessageType::SensorStateResponse) => {
				let ssr: api::SensorStateResponse = self.receive_message_body(&header)?;
				self.states.insert(ssr.key, State::Measurement(ssr.state));
				Ok(true)
			}

			Some(MessageType::BinarySensorStateResponse) => {
				let ssr: api::BinarySensorStateResponse = self.receive_message_body(&header)?;
				self.states.insert(ssr.key, State::Binary(ssr.state));
				Ok(true)
			}

			Some(MessageType::TextSensorStateResponse) => {
				let ssr: api::TextSensorStateResponse = self.receive_message_body(&header)?;
				self.states.insert(ssr.key, State::Text(ssr.state));
				Ok(true)
			}

			// State updates
			Some(MessageType::CoverStateResponse)
			| Some(MessageType::FanStateResponse)
			| Some(MessageType::LightStateResponse)
			| Some(MessageType::SwitchStateResponse)
			| Some(MessageType::ClimateStateResponse)
			| Some(MessageType::NumberStateResponse)
			| Some(MessageType::SelectStateResponse) => {
				// Skip these messages
				println!("Receive state update: {:?}", header.message_type);
				self.ignore_bytes(header.message_length)?;
				Ok(true)
			}

			Some(_) => Ok(false),
			None => {
				panic!("unknown message type received: {}", header.message_type);
			}
		}
	}

	pub(crate) fn receive_message_header(&mut self) -> Result<MessageHeader, EspHomeError> {
		loop {
			let mut zero = [MaybeUninit::uninit() ; 1];
			self.cis.read_exact(&mut zero)?;
			let len = self.cis.read_raw_varint32()?;
			let tp = self.cis.read_raw_varint32()?;

			let header = MessageHeader {
				message_length: len,
				message_type: tp,
			};

			// Handle internal messages
			if !self.process_unsolicited(&header)? {
				return Ok(header);
			}
		}
	}

	pub(crate) fn request<M, R>(
		&mut self,
		message_type: MessageType,
		message: &M,
		reply_type: MessageType,
	) -> Result<R, EspHomeError>
	where
		M: protobuf::Message,
		R: protobuf::Message,
	{
		self.send_message(message_type, message)?;
		self.receive_message::<R>(reply_type)
	}

	pub fn connect(mut self) -> Result<Device<'a>, EspHomeError> {
		let mut hr = api::HelloRequest::new();
		hr.client_info = "esphome.rs".to_string();
		self.send_message(MessageType::HelloRequest, &hr)?;

		let hr: HelloResponse = self.receive_message(MessageType::HelloResponse)?;
		Ok(Device::new(self, hr))
	}
}
