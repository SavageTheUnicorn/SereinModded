//! Native chrome around the temporary, manually completed invite verification widget.
use crate::{design, dialog};

#[derive(Default)]
pub struct VerificationUi {
	pub request: Option<u64>,
	pub bounds: Option<egui::Rect>,
	pub start_requested: bool,
	pub active: bool,
	pub error: Option<&'static str>,
	generation: u64,
}

impl VerificationUi {
	pub(super) fn show(&mut self, ctx: &egui::Context, state: &mut client_core::State) {
		self.bounds = None;
		let Some((request, _)) = state.invite_challenge() else {
			*self = Self::default();
			return;
		};
		if self.request != Some(request) || self.generation != state.generation {
			*self = Self {
				request: Some(request),
				generation: state.generation,
				..Self::default()
			};
		}
		let title = state
			.invites
			.get(&state.invite_join.code)
			.and_then(|(_, preview)| preview.as_ref())
			.and_then(|preview| preview.as_ref().ok())
			.and_then(|preview| preview.embed.title.as_deref());
		let mut cancel = false;
		let response = dialog::Dialog::new("invite-verification", "Verification required")
			.subtitle("Complete the check to join this server.")
			.width(520.0)
			.show(ctx, |d| {
				d.content(|ui| {
					let colors = design::palette(ui);
					if let Some(title) = title {
						ui.add(egui::Label::new(design::semibold(ui, title, 16.0)).truncate())
							.on_hover_text(title);
						ui.add_space(12.0);
					}
					let height = if self.active {
						(ctx.content_rect().height() - 260.0).clamp(160.0, 480.0)
					} else {
						148.0
					};
					let (rect, _) = ui.allocate_exact_size(
						egui::vec2(ui.available_width(), height),
						egui::Sense::hover(),
					);
					ui.painter().rect_filled(rect, 10, colors.base);
					ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink(16.0)), |ui| {
						ui.vertical_centered(|ui| {
							ui.add_space(if self.active { 28.0 } else { 12.0 });
							ui.label(design::semibold(
								ui,
								if self.active {
									"Loading verification…"
								} else {
									"One quick check"
								},
								18.0,
							));
							ui.add_space(8.0);
							ui.add(
								egui::Label::new(
									egui::RichText::new(if state.demo {
										"Offline preview · no verification service is contacted."
									} else {
										"Discord requires a security check before you can join."
									})
									.size(13.0)
									.color(colors.muted),
								)
								.wrap(),
							);
						});
					});
					if self.active {
						self.bounds = Some(rect);
					}
					if let Some(error) = self.error {
						ui.add_space(12.0);
						dialog::notice(ui, dialog::Level::Error, error);
					}
					ui.add_space(16.0);
				});
				d.footer(|ui| {
					if !self.active
						&& dialog::action(ui, "Verify", dialog::Action::Primary).clicked()
					{
						self.error = None;
						self.active = true;
						self.start_requested = !state.demo;
					}
					cancel |= dialog::action(ui, "Cancel", dialog::Action::Neutral).clicked();
				});
			});
		if cancel || response.close {
			state.cancel_invite_challenge(request);
			*self = Self::default();
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	fn fixture() -> client_core::State {
		let mut state = client_core::State {
			auth: client_core::auth::AuthState::Authenticated,
			gateway_connected: true,
			..Default::default()
		};
		state.invites.insert(
			"synthetic".into(),
			(
				std::time::Instant::now(),
				Some(Ok(model::InvitePreview {
					guild: model::Id(42),
					embed: model::Embed {
						title: Some(
							"A synthetic server with a very long name for narrow windows".into(),
						),
						..Default::default()
					},
				})),
			),
		);
		let Some(client_core::Command::JoinInvite { request, .. }) =
			state.join_invite("synthetic".into())
		else {
			panic!("join");
		};
		state.apply(client_core::Envelope {
			generation: state.generation,
			event: client_core::Event::InviteChallenge {
				request,
				challenge: Box::new(
					client_core::captcha::Challenge::new(
						"synthetic-key".into(),
						None,
						None,
						None,
						false,
					)
					.unwrap(),
				),
			},
		});
		state
	}
	fn frame(
		ctx: &egui::Context,
		view: &mut VerificationUi,
		state: &mut client_core::State,
		size: egui::Vec2,
		events: Vec<egui::Event>,
	) -> Vec<(String, egui::Rect)> {
		fn labels(shape: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
			match shape {
				egui::Shape::Text(text) => out.push((
					text.galley.job.text.clone(),
					text.galley.rect.translate(text.pos.to_vec2()),
				)),
				egui::Shape::Vec(shapes) => {
					for shape in shapes {
						labels(shape, out);
					}
				}
				_ => {}
			}
		}
		let output = ctx.run_ui(
			egui::RawInput {
				screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
				events,
				..Default::default()
			},
			|ui| view.show(ui.ctx(), state),
		);
		let mut texts = Vec::new();
		for shape in &output.shapes {
			labels(&shape.shape, &mut texts);
		}
		output.drop_without_applying_deltas();
		texts
	}
	#[test]
	fn verification_requires_a_click_fits_viewport_and_cancels_with_escape() {
		for (light, size) in [
			(false, egui::vec2(960.0, 760.0)),
			(true, egui::vec2(320.0, 600.0)),
		] {
			for demo in [false, true] {
				let ctx = egui::Context::default();
				design::apply(&ctx);
				ctx.set_visuals(if light {
					egui::Visuals::light()
				} else {
					egui::Visuals::dark()
				});
				let mut state = fixture();
				state.demo = demo;
				let mut view = VerificationUi::default();
				for _ in 0..3 {
					frame(&ctx, &mut view, &mut state, size, vec![]);
				}
				assert!(!view.start_requested && !view.active && view.bounds.is_none());
				let texts = frame(&ctx, &mut view, &mut state, size, vec![]);
				let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
				assert!(
					texts.iter().all(|(_, rect)| viewport.contains_rect(*rect)),
					"{texts:?}"
				);
				let rect = texts.iter().find(|(text, _)| text == "Verify").unwrap().1;
				for pressed in [true, false] {
					frame(
						&ctx,
						&mut view,
						&mut state,
						size,
						vec![
							egui::Event::PointerMoved(rect.center()),
							egui::Event::PointerButton {
								pos: rect.center(),
								button: egui::PointerButton::Primary,
								pressed,
								modifiers: egui::Modifiers::NONE,
							},
						],
					);
				}
				for _ in 0..3 {
					frame(&ctx, &mut view, &mut state, size, vec![]);
				}
				assert!(view.active);
				assert_eq!(view.start_requested, !demo);
				assert!(
					egui::Rect::from_min_size(egui::Pos2::ZERO, size)
						.contains_rect(view.bounds.unwrap())
				);
				view.active = false;
				view.start_requested = false;
				view.error =
					Some("The verification could not load. Check your connection and try again.");
				for _ in 0..3 {
					frame(&ctx, &mut view, &mut state, size, vec![]);
				}
				let texts = frame(&ctx, &mut view, &mut state, size, vec![]);
				assert!(texts.iter().any(|(text, _)| text == "Verify"));
				assert!(
					texts.iter().all(|(_, rect)| viewport.contains_rect(*rect)),
					"{texts:?}"
				);
				frame(
					&ctx,
					&mut view,
					&mut state,
					size,
					vec![egui::Event::Key {
						key: egui::Key::Escape,
						physical_key: None,
						pressed: true,
						repeat: false,
						modifiers: egui::Modifiers::NONE,
					}],
				);
				assert!(view.request.is_none() && view.bounds.is_none() && !view.active);
				assert!(state.invite_challenge().is_none());
			}
		}
	}
}
