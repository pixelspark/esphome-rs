use crate::api::{self, ConnectResponse, HelloResponse};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use protobuf::{CodedInputStream, CodedOutputStream};
use std::{
	collections::HashMap,
	error::Error,
	io::{Read, Write},
	time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

#[derive(Debug, Clone)]
pub enum State {
	Binary(bool),
	Measurement(f32),
	Text(String),
}

#[derive(Debug)]
pub struct EntityInfo {
	name: String,
	key: u32,
}

#[derive(Debug)]
pub struct ExtendedInfo {
	object_id: String,
	unique_id: String,
}

#[derive(Debug)]
pub enum Entity {
	BinarySensor(EntityInfo, ExtendedInfo),
	Camera(EntityInfo, ExtendedInfo),
	Climate(EntityInfo, ExtendedInfo),
	Cover(EntityInfo, ExtendedInfo),
	Fan(EntityInfo, ExtendedInfo),
	Light(EntityInfo, ExtendedInfo),
	Number(EntityInfo, ExtendedInfo),
	Select(EntityInfo, ExtendedInfo),
	Sensor(EntityInfo, ExtendedInfo),
	Services(EntityInfo),
	Switch(EntityInfo, ExtendedInfo),
	TextSensor(EntityInfo, ExtendedInfo),
}

pub struct Connection<'a> {
	cis: CodedInputStream<'a>,
	cos: CodedOutputStream<'a>,
	states: HashMap<u32, State>,
}

#[derive(Debug, Copy, Clone, FromPrimitive)]
pub enum MessageType {
	HelloRequest = 1,
	HelloResponse = 2,
	ConnectRequest = 3,
	ConnectResponse = 4,
	DisconnectRequest = 5,
	DisconnectResponse = 6,
	PingRequest = 7,
	PingResponse = 8,
	DeviceInfoRequest = 9,
	DeviceInfoResponse = 10,
	ListEntitiesRequest = 11,
	ListEntitiesBinarySensorResponse = 12,
	ListEntitiesCoverResponse = 13,
	ListEntitiesFanResponse = 14,
	ListEntitiesLightResponse = 15,
	ListEntitiesSensorResponse = 16,
	ListEntitiesSwitchResponse = 17,
	ListEntitiesTextSensorResponse = 18,
	ListEntitiesDoneResponse = 19,
	SubscribeStatesRequest = 20,

	BinarySensorStateResponse = 21,
	CoverStateResponse = 22,
	FanStateResponse = 23,
	LightStateResponse = 24,
	SensorStateResponse = 25,
	SwitchStateResponse = 26,
	TextSensorStateResponse = 27,

	ClimateStateResponse = 47,
	NumberStateResponse = 50,
	SelectStateResponse = 53,

	GetTimeRequest = 36,
	GetTimeResponse = 37,

	ListEntitiesServicesResponse = 41,
	ListEntitiesCameraResponse = 43,
	ListEntitiesClimateResponse = 46,
	ListEntitiesNumberResponse = 49,
	ListEntitiesSelectResponse = 52,
}

#[derive(Error, Debug)]
pub enum ESPHomeError {
	#[error("The password was not valid")]
	InvalidPassword,

	#[error("Received an unexpected response type (expected {expected:?}, received {received:?})")]
	UnexpectedResponse {
		expected: MessageType,
		received: u32,
	},
}

#[derive(Debug)]
struct MessageHeader {
	message_length: u32,
	message_type: u32,
}

impl Entity {
	pub fn key(&self) -> u32 {
		match self {
			Entity::BinarySensor(b, _)
			| Entity::Camera(b, _)
			| Entity::Climate(b, _)
			| Entity::Cover(b, _)
			| Entity::Fan(b, _)
			| Entity::Light(b, _)
			| Entity::Number(b, _)
			| Entity::Select(b, _)
			| Entity::Sensor(b, _)
			| Entity::Switch(b, _)
			| Entity::TextSensor(b, _) => b.key,

			Entity::Services(b) => b.key,
		}
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
			states: HashMap::new(),
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

	pub fn get_last_state(&mut self, entity: &Entity) -> Result<Option<State>, Box<dyn Error>> {
		match self.states.get(&entity.key()) {
			Some(s) => Ok(Some(s.clone())),
			None => Ok(None),
		}
	}

	fn receive_message<M>(&mut self, message_type: MessageType) -> Result<M, Box<dyn Error>>
	where
		M: protobuf::Message,
	{
		let header = self.receive_message_header()?;
		if header.message_type != (message_type as u32) {
			return Err(Box::new(ESPHomeError::UnexpectedResponse {
				expected: message_type,
				received: header.message_type,
			}));
		}
		self.receive_message_body(&header)
	}

	fn receive_message_body<M>(&mut self, header: &MessageHeader) -> Result<M, Box<dyn Error>>
	where
		M: protobuf::Message,
	{
		let mut message_bytes = vec![0u8; header.message_length as usize];
		self.cis.read_exact(&mut message_bytes)?;
		Ok(M::parse_from_bytes(&message_bytes)?)
	}

	fn ignore_bytes(&mut self, bytes: u32) -> Result<(), Box<dyn Error>> {
		self.cis.skip_raw_bytes(bytes)?;
		Ok(())
	}

	fn process_unsolicited(&mut self, header: &MessageHeader) -> Result<bool, Box<dyn Error>> {
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
				println!("State update {:#?}", self.states);
				Ok(true)
			}

			Some(MessageType::BinarySensorStateResponse) => {
				let ssr: api::BinarySensorStateResponse = self.receive_message_body(&header)?;
				self.states.insert(ssr.key, State::Binary(ssr.state));
				println!("State update {:#?}", self.states);
				Ok(true)
			}

			Some(MessageType::TextSensorStateResponse) => {
				let ssr: api::TextSensorStateResponse = self.receive_message_body(&header)?;
				self.states.insert(ssr.key, State::Text(ssr.state));
				println!("State update {:#?}", self.states);
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

	fn receive_message_header(&mut self) -> Result<MessageHeader, Box<dyn Error>> {
		loop {
			let mut zero = [0u8; 1];
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
			} else {
				println!("Skip internal message {:#?}", header);
			}
		}
	}

	fn request<M, R>(
		&mut self,
		message_type: MessageType,
		message: &M,
		reply_type: MessageType,
	) -> Result<R, Box<dyn Error>>
	where
		M: protobuf::Message,
		R: protobuf::Message,
	{
		self.send_message(message_type, message)?;
		self.receive_message::<R>(reply_type)
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
	pub connection: Connection<'a>,
	hello_information: api::HelloResponse,
}

impl<'a> Device<'a> {
	fn new(connection: Connection<'a>, hello_information: HelloResponse) -> Device<'a> {
		Device {
			connection,
			hello_information,
		}
	}

	pub fn server_info(&self) -> String {
		self.hello_information.get_server_info().to_owned()
	}

	pub fn authenticate(
		mut self,
		password: &str,
	) -> Result<AuthenticatedDevice<'a>, Box<dyn Error>> {
		let mut cr = api::ConnectRequest::new();
		cr.set_password(password.to_string());
		self.connection
			.send_message(MessageType::ConnectRequest, &cr)?;
		let cr: ConnectResponse = self
			.connection
			.receive_message(MessageType::ConnectResponse)?;

		if cr.get_invalid_password() {
			return Err(Box::new(ESPHomeError::InvalidPassword));
		}

		Ok(AuthenticatedDevice::new(self))
	}

	pub fn ping(&mut self) -> Result<(), Box<dyn Error>> {
		let _r: api::PingResponse = self.connection.request(
			MessageType::PingRequest,
			&api::PingRequest::new(),
			MessageType::PingResponse,
		)?;
		Ok(())
	}

	pub fn disconnect(mut self) -> Result<(), Box<dyn Error>> {
		let _r: api::DisconnectResponse = self.connection.request(
			MessageType::DisconnectRequest,
			&api::DisconnectRequest::new(),
			MessageType::DisconnectResponse,
		)?;
		Ok(())
	}
}

#[derive(Debug)]
pub struct DeviceInfo {
	info: api::DeviceInfoResponse,
}

impl DeviceInfo {
	pub fn new(info: api::DeviceInfoResponse) -> DeviceInfo {
		DeviceInfo { info }
	}

	pub fn name(&self) -> &str {
		&self.info.name
	}

	pub fn mac_address(&self) -> &str {
		&self.info.mac_address
	}

	pub fn esphome_version(&self) -> &str {
		&self.info.esphome_version
	}

	pub fn compilation_time(&self) -> &str {
		&self.info.compilation_time
	}

	pub fn model(&self) -> &str {
		&self.info.model
	}
}

macro_rules! extended_info_from {
	($message_type: ty) => {
		impl From<$message_type> for ExtendedInfo {
			fn from(m: $message_type) -> Self {
				ExtendedInfo {
					object_id: m.object_id,
					unique_id: m.unique_id,
				}
			}
		}

		impl From<$message_type> for EntityInfo {
			fn from(m: $message_type) -> Self {
				EntityInfo {
					name: m.name,
					key: m.key,
				}
			}
		}
	};
}

extended_info_from!(api::ListEntitiesSensorResponse);
extended_info_from!(api::ListEntitiesBinarySensorResponse);
extended_info_from!(api::ListEntitiesCoverResponse);
extended_info_from!(api::ListEntitiesFanResponse);
extended_info_from!(api::ListEntitiesLightResponse);
extended_info_from!(api::ListEntitiesSwitchResponse);
extended_info_from!(api::ListEntitiesTextSensorResponse);
extended_info_from!(api::ListEntitiesCameraResponse);
extended_info_from!(api::ListEntitiesClimateResponse);
extended_info_from!(api::ListEntitiesSelectResponse);
extended_info_from!(api::ListEntitiesNumberResponse);

impl From<api::ListEntitiesServicesResponse> for EntityInfo {
	fn from(m: api::ListEntitiesServicesResponse) -> Self {
		EntityInfo {
			name: m.name,
			key: m.key,
		}
	}
}

pub struct AuthenticatedDevice<'a> {
	pub device: Device<'a>,
}

impl<'a> AuthenticatedDevice<'a> {
	fn new(device: Device<'a>) -> AuthenticatedDevice<'a> {
		AuthenticatedDevice { device }
	}

	pub fn get_time(&mut self) -> Result<u32, Box<dyn Error>> {
		let r: api::GetTimeResponse = self.device.connection.request(
			MessageType::GetTimeRequest,
			&api::GetTimeRequest::new(),
			MessageType::GetTimeResponse,
		)?;
		Ok(r.epoch_seconds)
	}

	pub fn device_info(&mut self) -> Result<DeviceInfo, Box<dyn Error>> {
		let r: api::DeviceInfoResponse = self.device.connection.request(
			MessageType::DeviceInfoRequest,
			&api::DeviceInfoRequest::new(),
			MessageType::DeviceInfoResponse,
		)?;
		Ok(DeviceInfo::new(r))
	}

	pub fn listen(&mut self) -> Result<(), Box<dyn Error>> {
		let hdr = self.device.connection.receive_message_header()?;
		println!("Receive header: {:?}", hdr);
		println!("States: {:#?}", self.device.connection.states);
		Ok(())
	}

	pub fn subscribe_states(&mut self) -> Result<(), Box<dyn Error>> {
		self.device.connection.send_message(
			MessageType::SubscribeStatesRequest,
			&api::SubscribeStatesRequest::new(),
		)
	}

	pub fn list_entities(&mut self) -> Result<Vec<Entity>, Box<dyn Error>> {
		self.device.connection.send_message(
			MessageType::ListEntitiesRequest,
			&api::ListEntitiesRequest::new(),
		)?;

		let mut entities: Vec<Entity> = vec![];

		loop {
			let header = self.device.connection.receive_message_header()?;

			match FromPrimitive::from_u32(header.message_type) {
				Some(MessageType::ListEntitiesSensorResponse) => {
					let sr: api::ListEntitiesSensorResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::Sensor(
						EntityInfo::from(sr.clone()),
						ExtendedInfo::from(sr),
					))
				}

				Some(MessageType::ListEntitiesBinarySensorResponse) => {
					let sr: api::ListEntitiesBinarySensorResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::BinarySensor(
						EntityInfo::from(sr.clone()),
						ExtendedInfo::from(sr),
					))
				}

				Some(MessageType::ListEntitiesCoverResponse) => {
					let sr: api::ListEntitiesCoverResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::Cover(
						EntityInfo::from(sr.clone()),
						ExtendedInfo::from(sr),
					))
				}

				Some(MessageType::ListEntitiesFanResponse) => {
					let sr: api::ListEntitiesFanResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::Fan(
						EntityInfo::from(sr.clone()),
						ExtendedInfo::from(sr),
					))
				}

				Some(MessageType::ListEntitiesLightResponse) => {
					let sr: api::ListEntitiesLightResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::Light(
						EntityInfo::from(sr.clone()),
						ExtendedInfo::from(sr),
					))
				}

				Some(MessageType::ListEntitiesSwitchResponse) => {
					let sr: api::ListEntitiesSwitchResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::Switch(
						EntityInfo::from(sr.clone()),
						ExtendedInfo::from(sr),
					))
				}

				Some(MessageType::ListEntitiesTextSensorResponse) => {
					let sr: api::ListEntitiesTextSensorResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::TextSensor(
						EntityInfo::from(sr.clone()),
						ExtendedInfo::from(sr),
					))
				}

				Some(MessageType::ListEntitiesCameraResponse) => {
					let sr: api::ListEntitiesCameraResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::Camera(
						EntityInfo::from(sr.clone()),
						ExtendedInfo::from(sr),
					))
				}

				Some(MessageType::ListEntitiesClimateResponse) => {
					let sr: api::ListEntitiesClimateResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::Climate(
						EntityInfo::from(sr.clone()),
						ExtendedInfo::from(sr),
					))
				}

				Some(MessageType::ListEntitiesServicesResponse) => {
					let sr: api::ListEntitiesServicesResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::Services(EntityInfo::from(sr)))
				}

				Some(MessageType::ListEntitiesSelectResponse) => {
					let sr: api::ListEntitiesSelectResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::Select(
						EntityInfo::from(sr.clone()),
						ExtendedInfo::from(sr),
					))
				}

				Some(MessageType::ListEntitiesNumberResponse) => {
					let sr: api::ListEntitiesNumberResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::Number(
						EntityInfo::from(sr.clone()),
						ExtendedInfo::from(sr),
					))
				}

				Some(MessageType::ListEntitiesDoneResponse) => {
					self.device
						.connection
						.receive_message_body::<api::ListEntitiesDoneResponse>(&header)?;
					break;
				}
				Some(_) | None => {
					panic!("unexpected reply: {:?}", header)
				}
			}
		}

		Ok(entities)
	}
}
