//! Native taskbar unread indicator; no message text or account identity leaves the app.
pub const fn supported() -> bool {
	cfg!(target_os = "windows")
}
#[cfg(not(target_os = "windows"))]
pub fn set(_window: &winit::window::Window, _unread: bool) -> Result<(), &'static str> {
	Err("App icon badges are currently available on Windows.")
}
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
pub fn set(window: &winit::window::Window, unread: bool) -> Result<(), &'static str> {
	use windows::{
		Win32::{
			Foundation::HWND,
			System::Com::{
				CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
				CoUninitialize,
			},
			UI::{
				Shell::{ITaskbarList3, TaskbarList},
				WindowsAndMessaging::{CreateIcon, DestroyIcon, HICON},
			},
		},
		core::w,
	};
	use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
	const ERROR: &str = "Taskbar badge unavailable. Check your Windows taskbar settings.";
	let RawWindowHandle::Win32(handle) = window.window_handle().map_err(|_| ERROR)?.as_raw() else {
		return Err(ERROR);
	};
	// SAFETY: all COM operations and the retained winit handle stay on the UI thread.
	let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
	let result = (|| {
		let taskbar: ITaskbarList3 =
			unsafe { CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER) }
				.map_err(|_| ERROR)?;
		unsafe { taskbar.HrInit() }.map_err(|_| ERROR)?;
		let icon = if unread {
			let (mask, pixels) = dot();
			// SAFETY: buffers contain exactly a 16x16 1-bit AND mask and 32-bit BGRA pixels.
			unsafe { CreateIcon(None, 16, 16, 1, 32, mask.as_ptr(), pixels.as_ptr()) }
				.map_err(|_| ERROR)?
		} else {
			HICON::default()
		};
		// SAFETY: SetOverlayIcon copies the icon; its owned handle is destroyed immediately after.
		let result = unsafe {
			taskbar.SetOverlayIcon(
				HWND(handle.hwnd.get() as *mut _),
				icon,
				w!("Unread messages"),
			)
		}
		.map_err(|_| ERROR);
		if unread {
			let _ = unsafe { DestroyIcon(icon) };
		}
		result
	})();
	// Balance only successful apartment initialization; an existing different apartment is untouched.
	if initialized {
		unsafe { CoUninitialize() };
	}
	result
}
#[cfg(any(target_os = "windows", test))]
fn dot() -> ([u8; 32], [u8; 1024]) {
	let mut mask = [255; 32];
	let mut pixels = [0; 1024];
	for y in 0..16 {
		for x in 0..16 {
			if (x as i32 * 2 - 15).pow(2) + (y as i32 * 2 - 15).pow(2) <= 196 {
				mask[y * 2 + x / 8] &= !(1 << (7 - x % 8));
				pixels[(y * 16 + x) * 4..(y * 16 + x + 1) * 4].copy_from_slice(&[74, 70, 237, 255]);
			}
		}
	}
	(mask, pixels)
}
#[cfg(test)]
mod tests {
	#[test]
	fn badge_has_transparent_corners_and_red_center() {
		let (mask, pixels) = super::dot();
		assert_eq!(pixels[..4], [0; 4]);
		assert_eq!(pixels[544..548], [74, 70, 237, 255]);
		assert_ne!(mask[0] & 128, 0);
		assert_eq!(mask[16] & 1, 0);
	}
}
