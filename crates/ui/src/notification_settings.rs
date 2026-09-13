//! Notification sections share the existing settings shell and real account commands.
use crate::{MessagingUi, design};
use client_core::{Command, State};
use egui::RichText;
use model::notification_preferences::Sound;
use model::notification_settings::{Change, Section};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Tab {
	#[default]
	Overview,
	Sounds,
	Badges,
	Email,
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn notification_controls_change_real_state_and_emit_preview_requests() {
		fn texts(shape: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
			match shape {
				egui::Shape::Text(t) => {
					out.push((t.galley.job.text.clone(), t.visual_bounding_rect()))
				}
				egui::Shape::Vec(shapes) => {
					for shape in shapes {
						texts(shape, out);
					}
				}
				_ => {}
			}
		}
		for (width, dark) in [(320.0, true), (900.0, false)] {
			let ctx = egui::Context::default();
			ctx.set_visuals(if dark {
				egui::Visuals::dark()
			} else {
				egui::Visuals::light()
			});
			let mut view = MessagingUi::default();
			let mut state = State {
				demo: true,
				..Default::default()
			};
			let render = |view: &mut MessagingUi, state: &mut State, events| {
				let mut labels = vec![];
				let output = ctx.run_ui(
					egui::RawInput {
						screen_rect: Some(egui::Rect::from_min_size(
							egui::Pos2::ZERO,
							egui::vec2(width, 3600.0),
						)),
						events,
						focused: true,
						..Default::default()
					},
					|ui| {
						let mut commands = vec![];
						view.notification_settings(ui, state, &mut commands);
						assert!(commands.is_empty(), "demo must not emit account writes");
						assert!(
							ui.min_rect().width() <= width,
							"notification settings overflow"
						);
					},
				);
				for shape in &output.shapes {
					texts(&shape.shape, &mut labels);
				}
				output.drop_without_applying_deltas();
				labels
			};
			let labels = render(&mut view, &mut state, vec![]);
			for label in [
				"Overview",
				"Sounds",
				"Badges",
				"Email",
				"Communication Emails",
				"Enable Unread Message Badge",
				"Incoming Ring",
			] {
				assert!(labels.iter().any(|(s, _)| s == label), "missing {label}");
			}
			assert!(!labels.iter().any(|(s, _)| s == "Advanced"));
			for (label, action) in [
				("Friends come online", 0),
				("Communication Emails", 1),
				("Preview Sound", 2),
			] {
				let labels = render(&mut view, &mut state, vec![]);
				let point = labels
					.iter()
					.find(|(text, _)| text == label)
					.unwrap()
					.1
					.center();
				for pressed in [true, false] {
					render(
						&mut view,
						&mut state,
						vec![
							egui::Event::PointerMoved(point),
							egui::Event::PointerButton {
								pos: point,
								button: egui::PointerButton::Primary,
								pressed,
								modifiers: egui::Modifiers::NONE,
							},
						],
					);
				}
				match action {
					0 => assert!(!state.notification_settings.overview.unwrap().friends_online),
					1 => assert!(state.notification_settings.email.unwrap().communication),
					_ => assert_eq!(view.notification_preview.take(), Some(Sound::Message)),
				}
			}
		}
	}
}
impl Tab {
	pub const ALL: [Self; 4] = [Self::Overview, Self::Sounds, Self::Badges, Self::Email];
	pub fn label(self) -> &'static str {
		match self {
			Self::Overview => "Overview",
			Self::Sounds => "Sounds",
			Self::Badges => "Badges",
			Self::Email => "Email",
		}
	}
}
#[derive(Default)]
pub(super) struct Navigation {
	pub active: Tab,
	pub jump: Option<Tab>,
	generation: u64,
	attempted: [bool; 2],
}
impl Navigation {
	fn heading(&mut self, ui: &mut egui::Ui, tab: Tab) {
		if tab != Tab::Overview {
			ui.add_space(32.0);
			ui.separator();
			ui.add_space(32.0);
		}
		let heading = ui.label(
			RichText::new(tab.label())
				.size(26.0)
				.color(design::palette(ui).text_strong),
		);
		if heading.rect.top() <= ui.clip_rect().top() + 28.0 {
			self.active = tab;
		}
		if self.jump == Some(tab) {
			ui.scroll_to_rect(heading.rect.expand(8.0), Some(egui::Align::Min));
			self.jump = None;
		}
		ui.add_space(22.0);
	}
}
fn row(ui: &mut egui::Ui, label: &str, detail: Option<&str>, value: &mut bool) -> bool {
	let changed = design::switch(ui, label, detail, value).changed();
	ui.add_space(10.0);
	changed
}
impl MessagingUi {
	pub(super) fn notification_settings(
		&mut self,
		ui: &mut egui::Ui,
		state: &mut State,
		commands: &mut Vec<Command>,
	) {
		if self.settings.notifications.generation != state.generation {
			self.settings.notifications = Navigation {
				generation: state.generation,
				..Default::default()
			};
		}
		for (index, section) in [(0, Section::Overview), (1, Section::Email)] {
			if !self.settings.notifications.attempted[index]
				&& state.notification_settings.pending.is_none()
			{
				if let Some(command) = state.request_notification_settings(section) {
					commands.push(command);
				}
				self.settings.notifications.attempted[index] = true;
			}
		}
		let colors = design::palette(ui);
		let busy = state.notification_settings.pending.is_some();
		let mut change = None;
		if ui.available_width() < 500.0 {
			ui.horizontal_wrapped(|ui| {
				for tab in Tab::ALL {
					if ui
						.selectable_label(self.settings.notifications.active == tab, tab.label())
						.clicked()
					{
						self.settings.notifications.jump = Some(tab);
					}
				}
			});
		}
		self.settings.notifications.heading(ui, Tab::Overview);
		row(
			ui,
			"Enable Desktop Notifications",
			Some(
				"For per-channel or per-server notifications, right-click the channel or server and select Notification Settings.",
			),
			&mut self.notifications_enabled,
		);
		ui.label(
			RichText::new(if state.demo {
				"Offline preview"
			} else {
				self.notification_status
			})
			.size(12.0)
			.color(colors.muted),
		);
		ui.add_space(22.0);
		ui.label(design::semibold(ui, "Notify me when...", 16.0));
		ui.label(
			RichText::new("These preferences sync with your Discord account.")
				.size(12.0)
				.color(colors.muted),
		);
		ui.add_space(12.0);
		let mut overview = state.notification_settings.overview.unwrap_or_default();
		ui.add_enabled_ui(
			!busy && state.notification_settings.overview.is_some(),
			|ui| {
				for (label, value, make) in [
					(
						"People I know start streaming in small servers",
						&mut overview.streaming,
						Change::Streaming as fn(bool) -> Change,
					),
					(
						"A friend and I reach a friendship anniversary",
						&mut overview.friend_anniversary,
						Change::FriendAnniversary,
					),
					(
						"Friends come online",
						&mut overview.friends_online,
						Change::FriendsOnline,
					),
					(
						"A server has an upcoming event",
						&mut overview.upcoming_event,
						Change::UpcomingEvent,
					),
					(
						"Friends update their profile",
						&mut overview.profile_updates,
						Change::ProfileUpdates,
					),
				] {
					if row(ui, label, None, value) {
						change = Some(make(*value));
					}
				}
				let narrow = ui.available_width() < 520.0;
				let selector = |ui: &mut egui::Ui| {
					ui.label(design::medium(ui, "Someone reacts to my messages", 16.0));
					let old = overview.reactions;
					egui::ComboBox::from_id_salt("notification-reactions")
						.selected_text(match old {
							0 => "All Messages",
							1 => "Only Direct Messages",
							2 => "Never",
							_ => "Custom setting",
						})
						.width(200.0)
						.show_ui(ui, |ui| {
							for (value, label) in [
								(0, "All Messages"),
								(1, "Only Direct Messages"),
								(2, "Never"),
							] {
								ui.selectable_value(&mut overview.reactions, value, label);
							}
						});
					if old != overview.reactions {
						change = Some(Change::Reactions(overview.reactions));
					}
				};
				if narrow {
					ui.vertical(selector);
				} else {
					ui.horizontal(selector);
				}
			},
		);
		self.notification_account_status(ui, state, Section::Overview);
		self.settings.notifications.heading(ui, Tab::Sounds);
		for (label, value, sound) in [
			(
				"New Message",
				&mut self.notification_options.new_message,
				Sound::Message,
			),
			(
				"New Message in the channel I'm currently reading",
				&mut self.notification_options.current_channel,
				Sound::CurrentChannel,
			),
			(
				"Incoming Ring",
				&mut self.notification_options.incoming_ring,
				Sound::IncomingRing,
			),
		] {
			row(ui, label, None, value);
			if ui.link("Preview Sound").clicked() {
				self.notification_preview = Some(sound);
			}
			ui.add_space(14.0);
			ui.separator();
			ui.add_space(10.0);
		}
		row(
			ui,
			"Disable All Notification Sounds",
			Some(
				"Disables notification sounds. Your individual sound preferences are saved and restored when you turn this off.",
			),
			&mut self.notification_options.disable_sounds,
		);
		if !self.notification_sound_status.is_empty() {
			ui.label(
				RichText::new(self.notification_sound_status)
					.size(12.0)
					.color(colors.muted),
			);
		}
		ui.add_space(12.0);
		ui.label(design::medium(ui, "Related Settings", 15.0).color(colors.muted));
		if ui
			.add(
				egui::Button::new("Voice & Video  ›")
					.min_size(egui::vec2(ui.available_width(), 48.0)),
			)
			.clicked()
		{
			self.open_voice_settings();
		}
		self.settings.notifications.heading(ui, Tab::Badges);
		ui.add_enabled_ui(cfg!(target_os = "windows"), |ui| {
			row(
				ui,
				"Enable Unread Message Badge",
				Some(if cfg!(target_os = "windows") {
					"Shows a red badge on the app icon when you have unread messages."
				} else {
					"App icon badges are not available on this platform yet."
				}),
				&mut self.notification_options.unread_badge,
			);
		});
		self.settings.notifications.heading(ui, Tab::Email);
		let mut email = state.notification_settings.email.unwrap_or_default();
		ui.add_enabled_ui(!busy && state.notification_settings.email.is_some(), |ui| {
			for (label, detail, value, make) in [
				(
					"Communication Emails",
					"Receive emails for missed calls, messages, and message digests.",
					&mut email.communication,
					Change::Communication as fn(bool) -> Change,
				),
				(
					"Social Emails",
					"Receive emails for friend requests, new friend suggestions, and events in your server.",
					&mut email.social,
					Change::Social,
				),
				(
					"Announcements and Update Emails",
					"Receive emails about product updates, new features, improvements and bug fixes.",
					&mut email.announcements,
					Change::Announcements,
				),
				(
					"Tip Emails",
					"Receive emails with helpful advice and information on lesser known features.",
					&mut email.tips,
					Change::Tips,
				),
				(
					"Recommendations Emails",
					"Receive emails with recommended servers and suggested events.",
					&mut email.recommendations,
					Change::Recommendations,
				),
			] {
				if row(ui, label, Some(detail), value) {
					change = Some(make(*value));
				}
			}
			ui.add_space(12.0);
			ui.label(design::medium(
				ui,
				"Unsubscribe from all marketing emails",
				16.0,
			));
			ui.label(
				RichText::new(
					"This includes product updates, new features, tips, and recommendations.",
				)
				.size(13.0)
				.color(colors.muted),
			);
			if ui
				.add(egui::Button::new(
					RichText::new("Unsubscribe").color(colors.danger),
				))
				.clicked()
			{
				change = Some(Change::UnsubscribeMarketing);
			}
		});
		self.notification_account_status(ui, state, Section::Email);
		if let Some(change) = change
			&& let Some(command) = state.update_notification_settings(change)
		{
			commands.push(command);
		}
	}
	fn notification_account_status(&mut self, ui: &mut egui::Ui, state: &State, section: Section) {
		let index = if section == Section::Overview { 0 } else { 1 };
		if state.notification_settings.pending == Some(section) {
			ui.weak("Saving / loading preferences...");
		}
		let error = if section == Section::Overview {
			state.notification_settings.overview_error
		} else {
			state.notification_settings.email_error
		};
		if let Some(error) = error {
			ui.colored_label(design::palette(ui).danger, error.label());
			if ui.button("Retry loading preferences").clicked() {
				self.settings.notifications.attempted[index] = false;
			}
		}
	}
}
