use crate::connection::Connection;
use crate::model::{Entity, EntityInfo, EntityKind, ExtendedInfo};
use crate::{
	api::{self, ConnectResponse, HelloResponse},
	EspHomeError, MessageType,
};
use num_traits::FromPrimitive;
use std::error::Error;

pub struct Device<'a> {
	pub connection: Connection<'a>,
	hello_information: api::HelloResponse,
}

impl<'a> Device<'a> {
	pub(crate) fn new(connection: Connection<'a>, hello_information: HelloResponse) -> Device<'a> {
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
			return Err(Box::new(EspHomeError::InvalidPassword));
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

	pub fn device_info(&mut self) -> Result<DeviceInfo, EspHomeError> {
		let r: api::DeviceInfoResponse = self.device.connection.request(
			MessageType::DeviceInfoRequest,
			&api::DeviceInfoRequest::new(),
			MessageType::DeviceInfoResponse,
		)?;
		Ok(DeviceInfo::new(r))
	}

	pub fn listen(&mut self) -> Result<(), EspHomeError> {
		let _hdr = self.device.connection.receive_message_header()?;
		Ok(())
	}

	pub fn subscribe_states(&mut self) -> Result<(), EspHomeError> {
		self.device.connection.send_message(
			MessageType::SubscribeStatesRequest,
			&api::SubscribeStatesRequest::new(),
		)
	}

	pub fn list_entities(&mut self) -> Result<Vec<Entity>, EspHomeError> {
		self.device.connection.send_message(
			MessageType::ListEntitiesRequest,
			&api::ListEntitiesRequest::new(),
		)?;

		let mut entities: Vec<Entity> = vec![];

		loop {
			let header = self.device.connection.receive_message_header()?;

			match FromPrimitive::from_u32(header.message_type()) {
				Some(MessageType::ListEntitiesSensorResponse) => {
					let sr: api::ListEntitiesSensorResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::new(
						EntityInfo::from(sr.clone()),
						EntityKind::Sensor(ExtendedInfo::from(sr)),
					))
				}

				Some(MessageType::ListEntitiesBinarySensorResponse) => {
					let sr: api::ListEntitiesBinarySensorResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::new(
						EntityInfo::from(sr.clone()),
						EntityKind::BinarySensor(ExtendedInfo::from(sr)),
					))
				}

				Some(MessageType::ListEntitiesCoverResponse) => {
					let sr: api::ListEntitiesCoverResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::new(
						EntityInfo::from(sr.clone()),
						EntityKind::Cover(ExtendedInfo::from(sr)),
					))
				}

				Some(MessageType::ListEntitiesFanResponse) => {
					let sr: api::ListEntitiesFanResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::new(
						EntityInfo::from(sr.clone()),
						EntityKind::Fan(ExtendedInfo::from(sr)),
					))
				}

				Some(MessageType::ListEntitiesLightResponse) => {
					let sr: api::ListEntitiesLightResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::new(
						EntityInfo::from(sr.clone()),
						EntityKind::Light(ExtendedInfo::from(sr)),
					))
				}

				Some(MessageType::ListEntitiesSwitchResponse) => {
					let sr: api::ListEntitiesSwitchResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::new(
						EntityInfo::from(sr.clone()),
						EntityKind::Switch(ExtendedInfo::from(sr)),
					))
				}

				Some(MessageType::ListEntitiesTextSensorResponse) => {
					let sr: api::ListEntitiesTextSensorResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::new(
						EntityInfo::from(sr.clone()),
						EntityKind::TextSensor(ExtendedInfo::from(sr)),
					))
				}

				Some(MessageType::ListEntitiesCameraResponse) => {
					let sr: api::ListEntitiesCameraResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::new(
						EntityInfo::from(sr.clone()),
						EntityKind::Camera(ExtendedInfo::from(sr)),
					))
				}

				Some(MessageType::ListEntitiesClimateResponse) => {
					let sr: api::ListEntitiesClimateResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::new(
						EntityInfo::from(sr.clone()),
						EntityKind::Climate(ExtendedInfo::from(sr)),
					))
				}

				Some(MessageType::ListEntitiesServicesResponse) => {
					let sr: api::ListEntitiesServicesResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::new(
						EntityInfo::from(sr.clone()),
						EntityKind::Services,
					))
				}

				Some(MessageType::ListEntitiesSelectResponse) => {
					let sr: api::ListEntitiesSelectResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::new(
						EntityInfo::from(sr.clone()),
						EntityKind::Select(ExtendedInfo::from(sr)),
					))
				}

				Some(MessageType::ListEntitiesNumberResponse) => {
					let sr: api::ListEntitiesNumberResponse =
						self.device.connection.receive_message_body(&header)?;
					entities.push(Entity::new(
						EntityInfo::from(sr.clone()),
						EntityKind::Number(ExtendedInfo::from(sr)),
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
