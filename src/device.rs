use crate::api::{self, ConnectResponse, HelloResponse};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use protobuf::{CodedInputStream, CodedOutputStream};
use std::{
	error::Error,
	io::{Read, Write},
};
use thiserror::Error;

pub struct Connection<'a> {
	cis: CodedInputStream<'a>,
	cos: CodedOutputStream<'a>,
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
		Ok(self.receive_message_body(&header)?)
	}

	fn receive_message_body<M>(&mut self, header: &MessageHeader) -> Result<M, Box<dyn Error>>
	where
		M: protobuf::Message,
	{
		let mut message_bytes = vec![0u8; header.message_length as usize];
		self.cis.read_exact(&mut message_bytes)?;
		Ok(M::parse_from_bytes(&message_bytes)?)
	}

	fn receive_message_header(&mut self) -> Result<MessageHeader, Box<dyn Error>> {
		let mut zero = [0u8; 1];
		self.cis.read_exact(&mut zero)?;
		let len = self.cis.read_raw_varint32()?;
		let tp = self.cis.read_raw_varint32()?;
		Ok(MessageHeader {
			message_length: len,
			message_type: tp,
		})
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
	connection: Connection<'a>,
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

#[derive(Debug)]
pub struct ServicesInfo {
	name: String,
	key: u32,
}

#[derive(Debug)]
pub struct EntityInfo {
	object_id: String,
	name: String,
	unique_id: String,
	key: u32,
}

impl From<api::ListEntitiesSensorResponse> for EntityInfo {
	fn from(m: api::ListEntitiesSensorResponse) -> Self {
		EntityInfo {
			object_id: m.object_id,
			name: m.name,
			unique_id: m.unique_id,
			key: m.key,
		}
	}
}

impl From<api::ListEntitiesFanResponse> for EntityInfo {
	fn from(m: api::ListEntitiesFanResponse) -> Self {
		EntityInfo {
			object_id: m.object_id,
			name: m.name,
			unique_id: m.unique_id,
			key: m.key,
		}
	}
}

impl From<api::ListEntitiesLightResponse> for EntityInfo {
	fn from(m: api::ListEntitiesLightResponse) -> Self {
		EntityInfo {
			object_id: m.object_id,
			name: m.name,
			unique_id: m.unique_id,
			key: m.key,
		}
	}
}

impl From<api::ListEntitiesBinarySensorResponse> for EntityInfo {
	fn from(m: api::ListEntitiesBinarySensorResponse) -> Self {
		EntityInfo {
			object_id: m.object_id,
			name: m.name,
			unique_id: m.unique_id,
			key: m.key,
		}
	}
}

impl From<api::ListEntitiesCoverResponse> for EntityInfo {
	fn from(m: api::ListEntitiesCoverResponse) -> Self {
		EntityInfo {
			object_id: m.object_id,
			name: m.name,
			unique_id: m.unique_id,
			key: m.key,
		}
	}
}

impl From<api::ListEntitiesSwitchResponse> for EntityInfo {
	fn from(m: api::ListEntitiesSwitchResponse) -> Self {
		EntityInfo {
			object_id: m.object_id,
			name: m.name,
			unique_id: m.unique_id,
			key: m.key,
		}
	}
}

impl From<api::ListEntitiesTextSensorResponse> for EntityInfo {
	fn from(m: api::ListEntitiesTextSensorResponse) -> Self {
		EntityInfo {
			object_id: m.object_id,
			name: m.name,
			unique_id: m.unique_id,
			key: m.key,
		}
	}
}

impl From<api::ListEntitiesCameraResponse> for EntityInfo {
	fn from(m: api::ListEntitiesCameraResponse) -> Self {
		EntityInfo {
			object_id: m.object_id,
			name: m.name,
			unique_id: m.unique_id,
			key: m.key,
		}
	}
}

impl From<api::ListEntitiesClimateResponse> for EntityInfo {
	fn from(m: api::ListEntitiesClimateResponse) -> Self {
		EntityInfo {
			object_id: m.object_id,
			name: m.name,
			unique_id: m.unique_id,
			key: m.key,
		}
	}
}

impl From<api::ListEntitiesNumberResponse> for EntityInfo {
	fn from(m: api::ListEntitiesNumberResponse) -> Self {
		EntityInfo {
			object_id: m.object_id,
			name: m.name,
			unique_id: m.unique_id,
			key: m.key,
		}
	}
}

impl From<api::ListEntitiesSelectResponse> for EntityInfo {
	fn from(m: api::ListEntitiesSelectResponse) -> Self {
		EntityInfo {
			object_id: m.object_id,
			name: m.name,
			unique_id: m.unique_id,
			key: m.key,
		}
	}
}

impl From<api::ListEntitiesServicesResponse> for ServicesInfo {
	fn from(m: api::ListEntitiesServicesResponse) -> Self {
		ServicesInfo {
			name: m.name,
			key: m.key,
		}
	}
}

#[derive(Debug)]
pub enum Entity {
	BinarySensor(EntityInfo),
	Camera(EntityInfo),
	Climate(EntityInfo),
	Cover(EntityInfo),
	Fan(EntityInfo),
	Light(EntityInfo),
	Number(EntityInfo),
	Select(EntityInfo),
	Sensor(EntityInfo),
	Services(ServicesInfo),
	Switch(EntityInfo),
	TextSensor(EntityInfo),
}

pub struct AuthenticatedDevice<'a> {
	pub device: Device<'a>,
}

impl<'a> AuthenticatedDevice<'a> {
	fn new(device: Device<'a>) -> AuthenticatedDevice<'a> {
		AuthenticatedDevice { device }
	}

	pub fn device_info(&mut self) -> Result<DeviceInfo, Box<dyn Error>> {
		let r: api::DeviceInfoResponse = self.device.connection.request(
			MessageType::DeviceInfoRequest,
			&api::DeviceInfoRequest::new(),
			MessageType::DeviceInfoResponse,
		)?;
		Ok(DeviceInfo::new(r))
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
					entities.push(Entity::Sensor(EntityInfo::from(sr)))
				}

				Some(MessageType::ListEntitiesBinarySensorResponse) => {
					let sr: api::ListEntitiesBinarySensorResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::BinarySensor(EntityInfo::from(sr)))
				}

				Some(MessageType::ListEntitiesCoverResponse) => {
					let sr: api::ListEntitiesCoverResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::Cover(EntityInfo::from(sr)))
				}

				Some(MessageType::ListEntitiesFanResponse) => {
					let sr: api::ListEntitiesFanResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::Fan(EntityInfo::from(sr)))
				}

				Some(MessageType::ListEntitiesLightResponse) => {
					let sr: api::ListEntitiesLightResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::Light(EntityInfo::from(sr)))
				}

				Some(MessageType::ListEntitiesSwitchResponse) => {
					let sr: api::ListEntitiesSwitchResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::Switch(EntityInfo::from(sr)))
				}

				Some(MessageType::ListEntitiesTextSensorResponse) => {
					let sr: api::ListEntitiesTextSensorResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::TextSensor(EntityInfo::from(sr)))
				}

				Some(MessageType::ListEntitiesCameraResponse) => {
					let sr: api::ListEntitiesCameraResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::Camera(EntityInfo::from(sr)))
				}

				Some(MessageType::ListEntitiesClimateResponse) => {
					let sr: api::ListEntitiesClimateResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::Climate(EntityInfo::from(sr)))
				}

				Some(MessageType::ListEntitiesServicesResponse) => {
					let sr: api::ListEntitiesServicesResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::Services(ServicesInfo::from(sr)))
				}

				Some(MessageType::ListEntitiesSelectResponse) => {
					let sr: api::ListEntitiesSelectResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::Select(EntityInfo::from(sr)))
				}

				Some(MessageType::ListEntitiesNumberResponse) => {
					let sr: api::ListEntitiesNumberResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::Number(EntityInfo::from(sr)))
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
