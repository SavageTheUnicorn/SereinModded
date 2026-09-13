//! Acknowledged account preferences, scoped by the session envelope and request identity.
use crate::{
	Command, State,
	auth::{AuthState, Failure},
};
use model::notification_settings::{Change, Email, Overview, Section, Snapshot};
#[derive(Default)]
pub struct Settings {
	pub overview: Option<Overview>,
	pub email: Option<Email>,
	pub pending: Option<Section>,
	pub error: Option<Failure>,
	pub overview_error: Option<Failure>,
	pub email_error: Option<Failure>,
	request: u64,
	social: std::collections::VecDeque<model::notification_settings::SocialNotification>,
	seen: std::collections::VecDeque<model::Id>,
}
impl State {
	pub fn request_notification_settings(&mut self, section: Section) -> Option<Command> {
		self.notification_settings_command(section, None)
	}
	pub fn update_notification_settings(&mut self, change: Change) -> Option<Command> {
		if !change.valid() {
			return None;
		}
		let loaded = match change.section() {
			Section::Overview => self.notification_settings.overview.is_some(),
			Section::Email => self.notification_settings.email.is_some(),
		};
		if !loaded {
			return None;
		}
		self.notification_settings_command(change.section(), Some(change))
	}
	fn notification_settings_command(
		&mut self,
		section: Section,
		change: Option<Change>,
	) -> Option<Command> {
		if self.notification_settings.pending.is_some() {
			return None;
		}
		if !self.demo && (self.auth != AuthState::Authenticated || !self.gateway_connected) {
			let failure = Failure::ProtocolAt("Connect to Discord to update notification settings");
			self.notification_settings.error = Some(failure);
			match section {
				Section::Overview => self.notification_settings.overview_error = Some(failure),
				Section::Email => self.notification_settings.email_error = Some(failure),
			}
			return None;
		}
		self.notification_settings.error = None;
		match section {
			Section::Overview => self.notification_settings.overview_error = None,
			Section::Email => self.notification_settings.email_error = None,
		}
		if self.demo {
			match section {
				Section::Overview => {
					let value = self
						.notification_settings
						.overview
						.get_or_insert_with(Overview::default);
					if let Some(change) = change {
						change.apply_overview(value)
					}
				}
				Section::Email => {
					let value = self
						.notification_settings
						.email
						.get_or_insert_with(Email::default);
					if let Some(change) = change {
						change.apply_email(value)
					}
				}
			}
			return None;
		}
		self.notification_settings.request = self.notification_settings.request.wrapping_add(1);
		self.notification_settings.pending = Some(section);
		Some(Command::AccountNotificationSettings {
			request: self.notification_settings.request,
			section,
			change,
		})
	}
	pub fn apply_account_notification_settings(
		&mut self,
		request: u64,
		result: Result<Snapshot, Failure>,
	) {
		let Some(section) = self.notification_settings.pending else {
			return;
		};
		if request != self.notification_settings.request {
			return;
		}
		self.notification_settings.pending = None;
		let failure = result.as_ref().err().copied();
		match section {
			Section::Overview => self.notification_settings.overview_error = failure,
			Section::Email => self.notification_settings.email_error = failure,
		}
		match result {
			Ok(Snapshot::Overview(v)) if section == Section::Overview => {
				self.notification_settings.overview = Some(v)
			}
			Ok(Snapshot::Email(v)) if section == Section::Email => {
				self.notification_settings.email = Some(v)
			}
			Ok(_) => {
				let failure = Failure::ProtocolAt(
					"Discord returned a different notification settings section",
				);
				self.notification_settings.error = Some(failure);
				match section {
					Section::Overview => self.notification_settings.overview_error = Some(failure),
					Section::Email => self.notification_settings.email_error = Some(failure),
				}
			}
			Err(failure) => {
				self.notification_settings.error = Some(failure);
				if failure.ends_session() && failure != Failure::Capacity {
					self.fail(failure);
				}
			}
		}
	}
}
#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn notification_updates_wait_for_ack_and_demo_stays_offline() {
		let mut state = State {
			auth: AuthState::Authenticated,
			gateway_connected: true,
			..State::default()
		};
		let Command::AccountNotificationSettings { request, .. } = state
			.request_notification_settings(Section::Overview)
			.unwrap()
		else {
			panic!()
		};
		state.apply_account_notification_settings(
			request + 1,
			Ok(Snapshot::Overview(Overview::default())),
		);
		assert!(state.notification_settings.overview.is_none());
		state.apply_account_notification_settings(
			request,
			Ok(Snapshot::Overview(Overview::default())),
		);
		let command = state
			.update_notification_settings(Change::Streaming(false))
			.unwrap();
		assert!(state.notification_settings.overview.unwrap().streaming);
		state.command_rejected(command);
		assert!(state.notification_settings.pending.is_none());
		assert!(state.notification_settings.error.is_some());
		state.logout();
		assert!(state.notification_settings.overview.is_none());
		state.demo = true;
		assert!(
			state
				.request_notification_settings(Section::Overview)
				.is_none()
		);
		assert!(
			state
				.update_notification_settings(Change::Streaming(false))
				.is_none()
		);
		assert!(!state.notification_settings.overview.unwrap().streaming);
	}
}

impl State {
	pub fn receive_social_notification(
		&mut self,
		item: model::notification_settings::SocialNotification,
	) {
		if item.body.len() > 1024
			|| item.body.capacity() > 4096
			|| self.notification_settings.seen.contains(&item.id)
		{
			return;
		}
		if self.notification_settings.seen.len() == 128 {
			self.notification_settings.seen.pop_front();
		}
		self.notification_settings.seen.push_back(item.id);
		self.enqueue_social_notification(item);
	}
	fn enqueue_social_notification(
		&mut self,
		item: model::notification_settings::SocialNotification,
	) {
		if !self.social_notification_allowed(&item) {
			return;
		}
		while self.notification_settings.social.len() >= 16
			|| self
				.notification_settings
				.social
				.iter()
				.map(|v| v.bytes())
				.sum::<usize>()
				+ item.bytes()
				> 20 * 1024
		{
			self.notification_settings.social.pop_front();
		}
		self.notification_settings.social.push_back(item);
	}
	pub fn take_social_notification(
		&mut self,
	) -> Option<model::notification_settings::SocialNotification> {
		while let Some(item) = self.notification_settings.social.pop_front() {
			if self.social_notification_allowed(&item) {
				return Some(item);
			}
		}
		None
	}
	fn social_notification_allowed(
		&self,
		item: &model::notification_settings::SocialNotification,
	) -> bool {
		use model::notification_settings::SocialKind;
		let Some(settings) = self.notification_settings.overview else {
			return false;
		};
		if item
			.other_user
			.is_some_and(|id| self.user_blocked(id) == Some(true))
		{
			return false;
		}
		let enabled = match item.kind {
			SocialKind::Streaming => settings.streaming,
			SocialKind::FriendsOnline => settings.friends_online,
			SocialKind::ProfileUpdates => settings.profile_updates,
			SocialKind::UpcomingEvent => settings.upcoming_event,
			SocialKind::Reaction => {
				settings.reactions == 0 || (settings.reactions == 1 && item.guild.is_none())
			}
		};
		enabled && self.social_notification_scope_allowed(item.channel, item.guild)
	}
	pub(crate) fn interrupt_notification_settings(&mut self, failure: Failure) {
		if let Some(section) = self.notification_settings.pending.take() {
			match section {
				Section::Overview => self.notification_settings.overview_error = Some(failure),
				Section::Email => self.notification_settings.email_error = Some(failure),
			}
		}
		self.notification_settings.social.clear();
	}
}

#[cfg(test)]
mod social_tests {
	use super::*;
	use model::{
		Id,
		notification_settings::{SocialKind, SocialNotification},
	};
	#[test]
	fn social_queue_deduplicates_bounds_and_rechecks_preferences() {
		let mut state = State {
			gateway_connected: true,
			..State::default()
		};
		state.notification_settings.overview = Some(Overview::default());
		state
			.apply_notification_preferences(crate::notifications::Event::Presence(Some(false)))
			.unwrap();
		state
			.apply_notification_preferences(crate::notifications::Event::Settings {
				entries: vec![crate::notifications::Setting {
					guild: Some(Id(2)),
					muted: Some(false),
					..Default::default()
				}],
				replace: true,
			})
			.unwrap();
		let item = |n| SocialNotification {
			id: Id(n),
			kind: SocialKind::Streaming,
			body: "Synthetic".into(),
			guild: Some(Id(2)),
			channel: None,
			other_user: None,
		};
		state.receive_social_notification(item(1));
		state.receive_social_notification(item(1));
		assert!(state.take_social_notification().is_some());
		assert!(state.take_social_notification().is_none());
		for n in 2..200 {
			state.receive_social_notification(item(n));
		}
		assert_eq!(state.notification_settings.social.len(), 16);
		assert_eq!(state.notification_settings.seen.len(), 128);
		state
			.notification_settings
			.overview
			.as_mut()
			.unwrap()
			.streaming = false;
		assert!(state.take_social_notification().is_none());
		state
			.notification_settings
			.overview
			.as_mut()
			.unwrap()
			.streaming = true;
		state.receive_social_notification(item(201));
		state
			.apply_notification_preferences(crate::notifications::Event::Presence(Some(true)))
			.unwrap();
		assert!(state.take_social_notification().is_none());
	}
}

impl State {
	pub(crate) fn notify_friend_change(
		&mut self,
		user: model::Id,
		kind: model::notification_settings::SocialKind,
	) {
		if self.friend_username(user).is_none() || self.user_blocked(user) != Some(false) {
			return;
		}
		self.enqueue_social_notification(model::notification_settings::SocialNotification {
			id: user,
			kind,
			body: kind.title().into(),
			channel: None,
			guild: None,
			other_user: Some(user),
		});
	}
}

#[cfg(test)]
mod friend_tests {
	use super::*;
	use model::{Id, Patch, notification_settings::SocialKind};
	#[test]
	fn received_friend_changes_skip_initial_snapshots_and_respect_toggles() {
		let mut state = State {
			gateway_connected: true,
			..State::default()
		};
		state.notification_settings.overview = Some(Overview::default());
		state
			.apply_notification_preferences(crate::notifications::Event::Presence(Some(false)))
			.unwrap();
		state
			.apply_notification_preferences(crate::notifications::Event::Settings {
				entries: vec![crate::notifications::Setting {
					muted: Some(false),
					..Default::default()
				}],
				replace: true,
			})
			.unwrap();
		let user = model::User {
			id: Id(9),
			name: "Synthetic".into(),
			avatar: None,
			webhook: false,
			kind: Default::default(),
			discriminator: 0,
		};
		state
			.apply_user_action(crate::user_actions::Event::Relationships(Some(vec![(
				user.id, false,
			)])))
			.unwrap();
		state
			.apply_user_action(crate::user_actions::Event::Friends(Some(vec![(
				user.clone(),
				"synthetic".into(),
			)])))
			.unwrap();
		let update = |status: &str| crate::presence::Update {
			user: Id(9),
			status: Patch::Value(status.into()),
			custom_status: Patch::Absent,
			activities: Patch::Absent,
		};
		state.apply_direct_presence(&[update("online")]);
		assert!(state.take_social_notification().is_none());
		state.apply_direct_presence(&[update("offline")]);
		state.apply_direct_presence(&[update("online")]);
		assert_eq!(
			state.take_social_notification().unwrap().kind,
			SocialKind::FriendsOnline
		);
		state.apply_direct_presence(&[update("online")]);
		assert!(state.take_social_notification().is_none());
		state
			.apply_user_action(crate::user_actions::Event::FriendProfile((
				user.clone(),
				"synthetic".into(),
			)))
			.unwrap();
		assert!(state.take_social_notification().is_none());
		let mut changed = user;
		changed.name = "Changed".into();
		state
			.apply_user_action(crate::user_actions::Event::FriendProfile((
				changed,
				"synthetic".into(),
			)))
			.unwrap();
		assert_eq!(
			state.take_social_notification().unwrap().kind,
			SocialKind::ProfileUpdates
		);
		state
			.notification_settings
			.overview
			.as_mut()
			.unwrap()
			.friends_online = false;
		state.apply_direct_presence(&[update("offline")]);
		state.apply_direct_presence(&[update("online")]);
		assert!(state.take_social_notification().is_none());
	}
}
