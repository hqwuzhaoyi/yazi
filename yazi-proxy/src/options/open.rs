use yazi_config::open::Opener;
use yazi_shared::{event::Cmd, fs::Url};

// --- Open
#[derive(Default)]
pub struct OpenDoOpt {
	pub hovered:     Url,
	pub targets:     Vec<(Url, String)>,
	pub interactive: bool,
}

impl From<Cmd> for OpenDoOpt {
	fn from(mut c: Cmd) -> Self { c.take_data().unwrap_or_default() }
}

// --- Open with
pub struct OpenWithOpt {
	pub targets: Vec<Url>,
	pub opener:  Opener,
}

impl TryFrom<Cmd> for OpenWithOpt {
	type Error = ();

	fn try_from(mut c: Cmd) -> Result<Self, Self::Error> { c.take_data().ok_or(()) }
}
