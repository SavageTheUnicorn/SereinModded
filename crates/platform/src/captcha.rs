//! A temporary, human-operated verification widget. Never receives the account token.
use client_core::captcha::{Challenge, Solution};
use std::sync::Arc;

#[cfg(target_os = "windows")]
use std::{
	borrow::Cow,
	sync::mpsc::{self, Receiver},
	time::{Duration, Instant},
};
#[cfg(target_os = "windows")]
use wry::{WebView, WebViewBuilder, WebViewBuilderExtWindows};

pub struct CaptchaView {
	#[cfg(target_os = "windows")]
	view: WebView,
	#[cfg(target_os = "windows")]
	results: Receiver<Result<Solution, &'static str>>,
	#[cfg(target_os = "windows")]
	opened: Instant,
}

impl CaptchaView {
	#[cfg(target_os = "windows")]
	pub fn open(
		parent: Arc<winit::window::Window>,
		challenge: &Challenge,
		dark: bool,
		wake: impl Fn() + Send + Sync + 'static,
	) -> Result<Self, &'static str> {
		Self::build(parent, challenge, dark, wake, include_str!("captcha.js"))
	}
	#[cfg(target_os = "windows")]
	fn build(
		parent: Arc<winit::window::Window>,
		challenge: &Challenge,
		dark: bool,
		wake: impl Fn() + Send + Sync + 'static,
		script: &str,
	) -> Result<Self, &'static str> {
		let mut random = [0_u8; 32];
		getrandom::fill(&mut random).map_err(|_| "Verification could not start.")?;
		let capability = random
			.iter()
			.map(|byte| format!("{byte:02x}"))
			.collect::<String>()
			+ ":";
		let config = serde_json::json!({"capability": capability, "sitekey": challenge.sitekey(), "rqdata": challenge.rqdata(), "invisible": challenge.invisible(), "dark": dark});
		let script = script.replace("__SEREIN_CAPTCHA_CONFIG__", &config.to_string());
		let html = include_str!("captcha.html")
			.replace("__THEME__", if dark { "dark" } else { "light" })
			.replace("__BACKGROUND__", if dark { "#18191c" } else { "#f7f8fa" })
			.replace("__FOREGROUND__", if dark { "#dbdee1" } else { "#313338" });
		let (send, results) = mpsc::sync_channel(1);
		let view = WebViewBuilder::new()
			.with_visible(false)
			.with_incognito(true).with_devtools(false).with_https_scheme(true)
			.with_custom_protocol("serein-captcha".into(), move |_, request| {
				let valid = request.method() == "GET" && request.uri() == "serein-captcha://verification.invalid/";
				wry::http::Response::builder().status(if valid {200} else {404})
					.header("Content-Type", "text/html; charset=utf-8")
					.header("Cache-Control", "no-store")
					.header("Content-Security-Policy", "default-src 'none'; script-src https://hcaptcha.com https://*.hcaptcha.com; frame-src https://hcaptcha.com https://*.hcaptcha.com; connect-src https://hcaptcha.com https://*.hcaptcha.com; style-src 'unsafe-inline' https://hcaptcha.com https://*.hcaptcha.com; img-src data: https://hcaptcha.com https://*.hcaptcha.com; base-uri 'none'; form-action 'none'")
					.body(Cow::Owned(if valid {html.as_bytes().to_vec()} else {Vec::new()})).expect("static response headers")
			})
			.with_url("serein-captcha://verification.invalid/")
			.with_initialization_script_for_main_only(script, true)
			.with_navigation_handler(|url| own_origin(&url))
			.with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
			.with_download_started_handler(|_, _| false)
			.with_ipc_handler(move |request| {
				// Wry binds WebView2's top-level WebMessageReceived, not its frame events.
				// The main-only bootstrap also checks window.top and keeps the capability private.
				if !own_origin(&request.uri().to_string()) || request.body().len() > 8270 { return; }
				let body = zeroize::Zeroizing::new(request.into_body());
				let Some(payload) = body.strip_prefix(&capability) else {return;};
				let result = if let Some(value) = payload.strip_prefix("verified:") {
					let Some(solution) = Solution::new(value.to_owned()) else {return;};
					Ok(solution)
				} else { match payload {"cancelled:" => Err("Verification cancelled."), "expired:" => Err("Verification expired. Please try again."), "error:" => Err("Verification could not load. Try again or open the invite in Discord."), _ => return} };
				if send.try_send(result).is_ok() {wake();}
			})
			.build_as_child(parent.as_ref()).map_err(|_| "The native verification window could not open.")?;
		Ok(Self {
			view,
			results,
			opened: Instant::now(),
		})
	}
	#[cfg(not(target_os = "windows"))]
	pub fn open(
		_: Arc<winit::window::Window>,
		_: &Challenge,
		_: bool,
		_: impl Fn() + Send + Sync + 'static,
	) -> Result<Self, &'static str> {
		Err("Verification is unavailable on this platform. Open the invite in Discord.")
	}
	pub fn set_bounds(&self, x: i32, y: i32, width: u32, height: u32) {
		#[cfg(target_os = "windows")]
		if self
			.view
			.set_bounds(wry::Rect {
				position: wry::dpi::PhysicalPosition::new(x, y).into(),
				size: wry::dpi::PhysicalSize::new(width, height).into(),
			})
			.is_ok()
		{
			let _ = self.view.set_visible(width > 0 && height > 0);
		}
		#[cfg(not(target_os = "windows"))]
		let _ = (x, y, width, height);
	}
	pub fn poll(&self) -> Option<Result<Solution, &'static str>> {
		#[cfg(target_os = "windows")]
		return self.results.try_recv().ok();
		#[cfg(not(target_os = "windows"))]
		None
	}
	pub fn expired(&self) -> bool {
		#[cfg(target_os = "windows")]
		return self.opened.elapsed() >= Duration::from_secs(300);
		#[cfg(not(target_os = "windows"))]
		true
	}
}

#[cfg(any(target_os = "windows", test))]
fn own_origin(value: &str) -> bool {
	url::Url::parse(value).is_ok_and(|url| {
		url.scheme() == "https"
			&& url.host_str() == Some("serein-captcha.verification.invalid")
			&& url.port_or_known_default() == Some(443)
			&& url.username().is_empty()
			&& url.password().is_none()
			&& url.path() == "/"
			&& url.query().is_none()
	})
}

#[cfg(test)]
mod tests {
	#[test]
	fn ipc_is_restricted_to_the_local_verification_page() {
		assert!(super::own_origin(
			"https://serein-captcha.verification.invalid/"
		));
		for url in [
			"https://discord.com/",
			"https://serein-captcha.verification.invalid.evil.test/",
			"http://serein-captcha.verification.invalid/",
			"https://user@serein-captcha.verification.invalid/",
			"https://serein-captcha.verification.invalid:444/",
			"https://serein-captcha.verification.invalid/other",
		] {
			assert!(!super::own_origin(url));
		}
	}
	#[cfg(target_os = "windows")]
	#[test]
	#[ignore = "opens an offline native WebView2 window; never contacts hCaptcha"]
	fn native_local_page_and_ipc() {
		use super::*;
		use winit::{
			application::ApplicationHandler,
			event::WindowEvent,
			event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
			platform::windows::EventLoopBuilderExtWindows,
			window::{Window, WindowId},
		};
		struct App {
			view: Option<CaptchaView>,
			window: Option<Arc<Window>>,
			deadline: Instant,
			success: bool,
		}
		impl ApplicationHandler for App {
			fn resumed(&mut self, event_loop: &ActiveEventLoop) {
				if self.window.is_some() {
					return;
				}
				let window = Arc::new(
					event_loop
						.create_window(
							Window::default_attributes()
								.with_title("Serein — offline verification smoke")
								.with_inner_size(winit::dpi::PhysicalSize::new(500, 400)),
						)
						.expect("test window"),
				);
				let challenge = Challenge::new("synthetic-sitekey".into(), None, None, None, false)
					.expect("synthetic challenge");
				let script = r#"(() => { const config = __SEREIN_CAPTCHA_CONFIG__; document.addEventListener('DOMContentLoaded', () => { if(window === window.top && location.origin === 'https://serein-captcha.verification.invalid' && document.getElementById('captcha')) { document.getElementById('status').textContent = 'Offline verification surface loaded.'; window.ipc.postMessage(config.capability + 'verified:synthetic-passcode'); } }); })();"#;
				let view = CaptchaView::build(window.clone(), &challenge, true, || {}, script)
					.expect("native webview");
				view.set_bounds(0, 0, 500, 400);
				self.view = Some(view);
				self.window = Some(window);
			}
			fn window_event(
				&mut self,
				event_loop: &ActiveEventLoop,
				_: WindowId,
				event: WindowEvent,
			) {
				if matches!(event, WindowEvent::CloseRequested) {
					event_loop.exit();
				}
			}
			fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
				if self
					.view
					.as_ref()
					.and_then(CaptchaView::poll)
					.is_some_and(|value| value.is_ok())
				{
					self.success = true;
					event_loop.exit();
				}
				if Instant::now() >= self.deadline {
					event_loop.exit();
				}
				event_loop.set_control_flow(ControlFlow::WaitUntil(
					Instant::now() + Duration::from_millis(20),
				));
			}
		}
		let event_loop = EventLoop::builder()
			.with_any_thread(true)
			.build()
			.expect("event loop");
		let mut app = App {
			view: None,
			window: None,
			deadline: Instant::now() + Duration::from_secs(20),
			success: false,
		};
		event_loop.run_app(&mut app).expect("native event loop");
		assert!(
			app.success,
			"local custom-protocol page must load and return its capability-bound result"
		);
	}
}
