//! Unofficial settings-proto/1 notification subtree; retain all untouched wire fields.
pub use crate::guild_folders::MAX_SETTINGS_RESPONSE;
use crate::{
	DecodeError,
	guild_folders::{fields, integer_wrapper, message},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use model::notification_settings::{Change, Email, Overview};
use serde::Deserialize;
const MAX_SUBTREE: usize = 16 * 1024;
pub struct Settings {
	pub version: u64,
	pub overview: Overview,
	wire: Vec<u8>,
	voice_wire: Vec<u8>,
}
pub fn decode_response(bytes: &[u8]) -> Result<Settings, DecodeError> {
	#[derive(Deserialize)]
	struct Response {
		settings: String,
		#[serde(default)]
		out_of_date: bool,
	}
	if bytes.len() > MAX_SETTINGS_RESPONSE {
		return Err(DecodeError);
	}
	let response: Response = serde_json::from_slice(bytes).map_err(|_| DecodeError)?;
	if response.out_of_date {
		return Err(DecodeError);
	}
	let wire = STANDARD
		.decode(response.settings)
		.map_err(|_| DecodeError)?;
	let mut version = None;
	let mut subtree = None;
	let mut voice = None;
	for field in fields(&wire)? {
		match field.number {
			1 => {
				if version.is_some() {
					return Err(DecodeError);
				}
				let mut data = None;
				for v in fields(field.message()?)? {
					if v.number == 3 {
						if data.is_some() {
							return Err(DecodeError);
						}
						data = Some(u32::try_from(v.integer()?).map_err(|_| DecodeError)? as u64);
					}
				}
				version = Some(data.unwrap_or(0));
			}
			5 => {
				if voice.is_some() {
					return Err(DecodeError);
				}
				let bytes = field.message()?;
				if bytes.len() > MAX_SUBTREE {
					return Err(DecodeError);
				}
				voice = Some(bytes);
			}
			7 => {
				if subtree.is_some() {
					return Err(DecodeError);
				}
				let bytes = field.message()?;
				if bytes.len() > MAX_SUBTREE {
					return Err(DecodeError);
				}
				subtree = Some(bytes);
			}
			_ => {}
		}
	}
	let wire = subtree.unwrap_or_default();
	let mut overview = Overview::default();
	let voice = voice.unwrap_or_default();
	let mut streaming = None;
	for field in fields(voice)? {
		if field.number == 7 {
			if streaming.is_some() {
				return Err(DecodeError);
			}
			streaming = Some(bool_wrapper(field.message()?)?);
		}
	}
	overview.streaming = streaming.unwrap_or(true);
	let mut seen = 0u32;
	for field in fields(wire)? {
		let target = match field.number {
			12 => Some(&mut overview.friends_online),
			14 => Some(&mut overview.friend_anniversary),
			16 => Some(&mut overview.profile_updates),
			22 => Some(&mut overview.upcoming_event),
			7 => {
				if seen & (1 << 7) != 0 {
					return Err(DecodeError);
				}
				seen |= 1 << 7;
				overview.reactions = u32::try_from(field.integer()?).map_err(|_| DecodeError)?;
				None
			}
			_ => None,
		};
		if let Some(target) = target {
			if seen & (1 << field.number) != 0 {
				return Err(DecodeError);
			}
			seen |= 1 << field.number;
			*target = bool_wrapper(field.message()?)?;
		}
	}
	Ok(Settings {
		version: version.ok_or(DecodeError)?,
		overview,
		wire: wire.to_vec(),
		voice_wire: voice.to_vec(),
	})
}
fn bool_wrapper(bytes: &[u8]) -> Result<bool, DecodeError> {
	let mut value = None;
	for field in fields(bytes)? {
		if field.number == 1 {
			if value.is_some() {
				return Err(DecodeError);
			}
			value = Some(match field.integer()? {
				0 => false,
				1 => true,
				_ => return Err(DecodeError),
			});
		}
	}
	Ok(value.unwrap_or(false))
}
pub fn encode_patch(current: &Settings, change: Change) -> Result<String, DecodeError> {
	let (number, value) = match change {
		Change::Streaming(b) => (7, u64::from(b)),
		Change::FriendsOnline(b) => (12, u64::from(b)),
		Change::FriendAnniversary(b) => (14, u64::from(b)),
		Change::ProfileUpdates(b) => (16, u64::from(b)),
		Change::UpcomingEvent(b) => (22, u64::from(b)),
		Change::Reactions(v) if v <= 2 => (7, u64::from(v)),
		_ => return Err(DecodeError),
	};
	let mut wire = Vec::new();
	let streaming = matches!(change, Change::Streaming(_));
	for field in fields(if streaming {
		&current.voice_wire
	} else {
		&current.wire
	})? {
		if field.number != number {
			wire.extend_from_slice(field.raw);
		}
	}
	if number == 7 && !streaming {
		wire.extend_from_slice(&[56, value as u8]);
	} else {
		integer_wrapper(number, value, &mut wire);
	}
	if wire.len() > MAX_SUBTREE {
		return Err(DecodeError);
	}
	let mut patch = Vec::new();
	message(if streaming { 5 } else { 7 }, &wire, &mut patch);
	Ok(STANDARD.encode(patch))
}
pub const MAX_EMAIL_RESPONSE: usize = 16 * 1024;
pub fn decode_email(bytes: &[u8]) -> Result<Email, DecodeError> {
	#[derive(Deserialize)]
	struct Response {
		categories: Email,
	}
	if bytes.len() > MAX_EMAIL_RESPONSE {
		return Err(DecodeError);
	}
	serde_json::from_slice::<Response>(bytes)
		.map(|r| r.categories)
		.map_err(|_| DecodeError)
}
pub fn email_patch(change: Change) -> Result<serde_json::Value, DecodeError> {
	use serde_json::json;
	let (key, value) = match change {
		Change::Communication(b) => ("communication", b),
		Change::Social(b) => ("social", b),
		Change::Announcements(b) => ("updates_and_announcements", b),
		Change::Tips(b) => ("tips", b),
		Change::Recommendations(b) => ("recommendations_and_events", b),
		Change::UnsubscribeMarketing => {
			return Ok(
				json!({"settings":{"categories":{"updates_and_announcements":false,"tips":false,"recommendations_and_events":false}}}),
			);
		}
		_ => return Err(DecodeError),
	};
	Ok(json!({"settings":{"categories":{key:value}}}))
}
#[cfg(test)]
mod tests {
	use super::*;
	fn response(wire: &[u8]) -> Vec<u8> {
		serde_json::to_vec(&serde_json::json!({"settings":STANDARD.encode(wire)})).unwrap()
	}
	#[test]
	fn notification_patch_retains_future_fields_and_checks_bounds() {
		let mut subtree = vec![56, 99];
		message(50, b"future", &mut subtree);
		integer_wrapper(12, 0, &mut subtree);
		let mut wire = vec![10, 2, 24, 7];
		message(7, &subtree, &mut wire);
		let current = decode_response(&response(&wire)).unwrap();
		assert!(!current.overview.friends_online);
		assert_eq!(current.overview.reactions, 99);
		let patch = STANDARD
			.decode(encode_patch(&current, Change::ProfileUpdates(false)).unwrap())
			.unwrap();
		let mut saved = vec![10, 2, 24, 8];
		saved.extend_from_slice(&patch);
		let saved = decode_response(&response(&saved)).unwrap();
		assert!(!saved.overview.profile_updates);
		assert_eq!(saved.overview.reactions, 99);
		assert!(saved.wire.windows(6).any(|v| v == b"future"));
		let stream_patch = STANDARD
			.decode(encode_patch(&current, Change::Streaming(false)).unwrap())
			.unwrap();
		assert_eq!(fields(&stream_patch).unwrap()[0].number, 5);
		let mut saved_wire = wire.clone();
		saved_wire.extend_from_slice(&stream_patch);
		let streaming_saved = decode_response(&response(&saved_wire)).unwrap();
		assert!(!streaming_saved.overview.streaming);
		assert_eq!(streaming_saved.overview.reactions, 99);
		assert!(encode_patch(&current, Change::Reactions(99)).is_err());
		assert!(decode_response(&response(&[10, 0, 58, 4, 98, 0, 98, 0])).is_err());
		assert!(decode_response(&vec![0; MAX_SETTINGS_RESPONSE + 1]).is_err());
		let email =
			decode_email(br#"{"categories":{"communication":true,"family_center_digest":true}}"#)
				.unwrap();
		assert!(email.communication);
		let mut expected = email;
		Change::UnsubscribeMarketing.apply_email(&mut expected);
		assert!(expected.communication);
		assert_eq!(
			email_patch(Change::Tips(false)).unwrap(),
			serde_json::json!({"settings":{"categories":{"tips":false}}})
		);
		assert!(decode_email(br#"{}"#).is_err());
	}
}

/// Notification center only; unknown item kinds are ignored without breaking the session.
pub fn social_notification(
	bytes: &[u8],
) -> Result<Option<model::notification_settings::SocialNotification>, DecodeError> {
	use model::{
		Id,
		notification_settings::{SocialKind, SocialNotification},
	};
	#[derive(Deserialize)]
	struct User {
		id: Id,
	}
	#[derive(Deserialize)]
	struct Item {
		id: Id,
		#[serde(rename = "type")]
		kind: String,
		body: String,
		acked: bool,
		completed: bool,
		#[serde(default)]
		message_channel_id: Option<Id>,
		#[serde(default)]
		guild_id: Option<Id>,
		#[serde(default)]
		other_user: Option<User>,
	}
	if bytes.len() > 64 * 1024 {
		return Err(DecodeError);
	}
	let item: Item = serde_json::from_slice(bytes).map_err(|_| DecodeError)?;
	if item.acked || item.completed {
		return Ok(None);
	}
	let kind = match item.kind.as_str() {
		"go_live_push" => SocialKind::Streaming,
		"scheduled_guild_event_started" => SocialKind::UpcomingEvent,
		"reaction_sent" => SocialKind::Reaction,
		_ => return Ok(None),
	};
	if item.body.is_empty() || item.body.len() > 1024 {
		return Err(DecodeError);
	}
	let body = item
		.body
		.chars()
		.filter(|c| !c.is_control() || *c == '\n')
		.collect::<String>();
	Ok(Some(SocialNotification {
		id: item.id,
		kind,
		body,
		channel: item.message_channel_id,
		guild: item.guild_id,
		other_user: item.other_user.map(|u| u.id),
	}))
}

#[cfg(test)]
mod social_tests {
	use super::*;
	#[test]
	fn notification_center_known_only_and_bounded() {
		let mut value = serde_json::json!({"id":"1","type":"go_live_push","body":"Synthetic stream","acked":false,"completed":false,"guild_id":"2"});
		assert!(
			social_notification(&serde_json::to_vec(&value).unwrap())
				.unwrap()
				.is_some()
		);
		value["acked"] = true.into();
		assert!(
			social_notification(&serde_json::to_vec(&value).unwrap())
				.unwrap()
				.is_none()
		);
		value["acked"] = false.into();
		value["type"] = "future".into();
		assert!(
			social_notification(&serde_json::to_vec(&value).unwrap())
				.unwrap()
				.is_none()
		);
		value["type"] = "reaction_sent".into();
		value["body"] = "x".repeat(1025).into();
		assert!(social_notification(&serde_json::to_vec(&value).unwrap()).is_err());
	}
}
