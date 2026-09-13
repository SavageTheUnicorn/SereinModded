//! Unofficial account preferences; fresh reads and confirmed, minimal writes.
use crate::{DiscordApi, Failure};
use discord_protocol::notification_settings::{
	self as wire, MAX_EMAIL_RESPONSE, MAX_SETTINGS_RESPONSE,
};
use model::notification_settings::{Change, Section, Snapshot};
use reqwest::Method;
impl DiscordApi {
	pub async fn account_notification_settings(
		&self,
		section: Section,
		change: Option<Change>,
	) -> Result<Snapshot, Failure> {
		if change.is_some_and(|v| !v.valid() || v.section() != section) {
			return Err(Failure::Protocol);
		}
		let unavailable =
			Failure::ProtocolAt("Discord notification settings are unavailable or unsupported");
		let unconfirmed =
			Failure::ProtocolAt("Notification settings were not confirmed; reload before retrying");
		match section {
			Section::Overview => {
				let bytes = self
					.request_limited(
						Method::GET,
						"/users/@me/settings-proto/1",
						None,
						MAX_SETTINGS_RESPONSE,
					)
					.await?;
				let current = wire::decode_response(&bytes).map_err(|_| unavailable)?;
				let Some(change) = change else {
					return Ok(Snapshot::Overview(current.overview));
				};
				let mut wanted = current.overview;
				change.apply_overview(&mut wanted);
				if wanted == current.overview {
					return Ok(Snapshot::Overview(wanted));
				}
				let patch = wire::encode_patch(&current, change).map_err(|_| Failure::Protocol)?;
				let bytes = self
					.request_limited(
						Method::PATCH,
						"/users/@me/settings-proto/1",
						Some(
							serde_json::json!({"settings":patch,"required_data_version":current.version}),
						),
						MAX_SETTINGS_RESPONSE,
					)
					.await?;
				let saved = wire::decode_response(&bytes).map_err(|_| unconfirmed)?;
				if saved.overview != wanted {
					return Err(unconfirmed);
				}
				Ok(Snapshot::Overview(saved.overview))
			}
			Section::Email => {
				let bytes = self
					.request_limited(
						Method::GET,
						"/users/@me/email-settings",
						None,
						MAX_EMAIL_RESPONSE,
					)
					.await?;
				let current = wire::decode_email(&bytes).map_err(|_| unavailable)?;
				let Some(change) = change else {
					return Ok(Snapshot::Email(current));
				};
				let mut wanted = current;
				change.apply_email(&mut wanted);
				if wanted == current {
					return Ok(Snapshot::Email(wanted));
				}
				let bytes = self
					.request_limited(
						Method::PATCH,
						"/users/@me/email-settings",
						Some(wire::email_patch(change).map_err(|_| Failure::Protocol)?),
						MAX_EMAIL_RESPONSE,
					)
					.await?;
				let saved = wire::decode_email(&bytes).map_err(|_| unconfirmed)?;
				if saved != wanted {
					return Err(unconfirmed);
				}
				Ok(Snapshot::Email(saved))
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use client_core::auth::SessionSecret;
	use std::{sync::Arc, time::Duration};
	use tokio::{
		io::{AsyncReadExt, AsyncWriteExt},
		net::TcpListener,
	};
	#[tokio::test]
	async fn notification_account_routes_and_minimal_acknowledged_writes() {
		let scenario = async {
			let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
			let mut api = DiscordApi::new(Arc::new(
				SessionSecret::from_owner_input("SYNTHETIC_NOTIFICATION_SETTINGS".into()).unwrap(),
			))
			.unwrap();
			api.base = format!("http://{}", listener.local_addr().unwrap());
			let overview = r#"{"settings":"CgIYBw=="}"#;
			let saved = r#"{"settings":"CgIYCCoEOgIIAA=="}"#;
			let email = r#"{"categories":{"communication":true,"social":true,"tips":true,"updates_and_announcements":true,"recommendations_and_events":true,"family_center_digest":true}}"#;
			let unsubscribed = r#"{"categories":{"communication":true,"social":true,"tips":false,"updates_and_announcements":false,"recommendations_and_events":false,"family_center_digest":true}}"#;
			let server = tokio::spawn(async move {
				for (method, path, expected, response) in [
					("GET", "/users/@me/settings-proto/1", None, overview),
					(
						"PATCH",
						"/users/@me/settings-proto/1",
						Some(serde_json::json!({"settings":"KgQ6AggA","required_data_version":7})),
						saved,
					),
					("GET", "/users/@me/email-settings", None, email),
					(
						"PATCH",
						"/users/@me/email-settings",
						Some(
							serde_json::json!({"settings":{"categories":{"updates_and_announcements":false,"tips":false,"recommendations_and_events":false}}}),
						),
						unsubscribed,
					),
					("GET", "/users/@me/email-settings", None, email),
					(
						"PATCH",
						"/users/@me/email-settings",
						Some(serde_json::json!({"settings":{"categories":{"tips":false}}})),
						email,
					),
				] {
					let (mut socket, _) = listener.accept().await.unwrap();
					let mut request = Vec::new();
					let header_end = loop {
						let mut bytes = [0; 1024];
						let count = socket.read(&mut bytes).await.unwrap();
						assert!(count > 0);
						request.extend_from_slice(&bytes[..count]);
						assert!(request.len() < 8192);
						if let Some(end) = request.windows(4).position(|v| v == b"\r\n\r\n") {
							break end + 4;
						}
					};
					let headers = std::str::from_utf8(&request[..header_end]).unwrap();
					assert!(headers.starts_with(&format!("{method} {path} HTTP/1.1\r\n")));
					let length = headers
						.lines()
						.find_map(|v| {
							v.to_ascii_lowercase()
								.strip_prefix("content-length: ")
								.and_then(|v| v.parse::<usize>().ok())
						})
						.unwrap_or(0);
					assert!(length < 4096);
					while request.len() < header_end + length {
						let mut bytes = [0; 1024];
						let count = socket.read(&mut bytes).await.unwrap();
						assert!(count > 0);
						request.extend_from_slice(&bytes[..count]);
					}
					if let Some(expected) = expected {
						assert_eq!(
							serde_json::from_slice::<serde_json::Value>(
								&request[header_end..header_end + length]
							)
							.unwrap(),
							expected
						);
					}
					socket
						.write_all(
							format!(
								"HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
								response.len()
							)
							.as_bytes(),
						)
						.await
						.unwrap();
				}
			});
			let Snapshot::Overview(value) = api
				.account_notification_settings(Section::Overview, Some(Change::Streaming(false)))
				.await
				.unwrap()
			else {
				panic!()
			};
			assert!(!value.streaming);
			let Snapshot::Email(value) = api
				.account_notification_settings(Section::Email, Some(Change::UnsubscribeMarketing))
				.await
				.unwrap()
			else {
				panic!()
			};
			assert!(value.communication && value.social);
			assert!(!value.tips && !value.announcements && !value.recommendations);
			assert!(
				api.account_notification_settings(Section::Email, Some(Change::Tips(false)))
					.await
					.is_err()
			);
			assert!(
				api.account_notification_settings(Section::Email, Some(Change::Streaming(false)))
					.await
					.is_err()
			);
			server.await.unwrap();
		};
		tokio::time::timeout(Duration::from_secs(10), scenario)
			.await
			.unwrap();
	}
}
