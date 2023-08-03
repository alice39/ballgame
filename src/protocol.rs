use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::vector::Vector;

const ZERO_PROTOCOL_ID: u8 = 0x00;
const JSON_PROTOCOL_ID: u8 = 0x01;

pub struct PacketBuf {
	buf: Vec<u8>,
	state: PacketBufState,
}

enum PacketBufState {
	Header,
	Content,
}

#[derive(Debug, Clone)]
pub enum PacketProtocol<T: Packet> {
	Raw {
		id: u32,
		protocol: u8,
		content: Vec<u8>,
	},
	Zero(T),
	Json(T),
}

pub trait Packet: Serialize + DeserializeOwned {
	fn id() -> u32;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientPacket {
	pub player_id: u32,
	pub orientation: u32,
	pub propulsor: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerPacket {
	pub player_id: u32,
	pub position: Vector,
	pub velocity: Vector,
	pub orientation: u32,
	pub design: u8,
	pub propulsor: u8,
	pub hits: u32,
}

impl PacketBuf {
	const HEADER_LEN: usize = 9;

	pub fn new() -> Self {
		Self {
			buf: vec![],
			state: PacketBufState::Header,
		}
	}

	pub fn process<T: Packet>(&mut self, bytes: &[u8]) -> Option<PacketProtocol<T>> {
		self.buf.extend_from_slice(bytes);

		match self.state {
			PacketBufState::Header => {
				if (self.buf.len() < PacketBuf::HEADER_LEN) {
					return None;
				}

				self.state = PacketBufState::Content;
				self.process(&[])
			}
			PacketBufState::Content => {
				let content_length = u32::from_be_bytes(self.buf[5..9].try_into().unwrap());
				let packet_length = PacketBuf::HEADER_LEN + content_length as usize;
				if self.buf.len() < packet_length {
					return None;
				}

				let packet_bytes: Vec<u8> = self.buf.drain(0..packet_length).collect();
				Some(PacketProtocol::try_from(packet_bytes.as_slice()).unwrap())
			}
		}
	}
}

impl<T: Packet> PacketProtocol<T> {
	pub fn serialize(self) -> anyhow::Result<Vec<u8>> {
		let (id, protocol, content) = match self {
			PacketProtocol::Raw {
				id,
				protocol,
				content,
			} => (id, protocol, content),
			PacketProtocol::Zero(data) => {
				use bincode::Options;

				let serialized_data = bincode::options()
					.with_big_endian()
					.with_fixint_encoding()
					.serialize(&data)?;

				(T::id(), ZERO_PROTOCOL_ID, serialized_data)
			}
			PacketProtocol::Json(data) => {
				let serialized_data = serde_json::to_string(&data).map(|s| s.into_bytes())?;

				(T::id(), JSON_PROTOCOL_ID, serialized_data)
			}
		};

		Ok([
			id.to_be_bytes().as_slice(),
			protocol.to_be_bytes().as_slice(),
			(content.len() as u32).to_be_bytes().as_slice(),
			content.as_slice(),
		]
		.concat())
	}

	pub fn deserialize(self) -> anyhow::Result<T> {
		match self {
			PacketProtocol::Raw {
				id,
				protocol,
				content,
			} => {
				if id != T::id() {
					anyhow::bail!("Id mismatch");
				}

				match protocol {
					ZERO_PROTOCOL_ID => {
						use bincode::Options;
						Ok(bincode::options()
							.with_big_endian()
							.with_fixint_encoding()
							.deserialize::<T>(&content)?)
					}
					JSON_PROTOCOL_ID => Ok(serde_json::from_slice(&content)?),
					_ => anyhow::bail!("Unknown protocol"),
				}
			}
			PacketProtocol::Zero(data) => Ok(data),
			PacketProtocol::Json(data) => Ok(data),
		}
	}
}

impl Packet for ClientPacket {
	fn id() -> u32 {
		0x00
	}
}

impl Packet for ServerPacket {
	fn id() -> u32 {
		0x01
	}
}

impl<T: Packet> TryFrom<&[u8]> for PacketProtocol<T> {
	type Error = anyhow::Error;

	fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
		let id = u32::from_be_bytes(bytes[0..4].try_into()?);
		let protocol = u8::from_be_bytes(bytes[4..5].try_into()?);

		let expected_length = u32::from_be_bytes(bytes[5..9].try_into()?) as usize;
		let content = bytes[9..].to_vec();

		if expected_length != content.len() {
			anyhow::bail!("Length mismatch");
		}

		Ok(Self::Raw {
			id,
			protocol,
			content,
		})
	}
}
